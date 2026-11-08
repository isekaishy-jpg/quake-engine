//! The editor application.

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use egui::{Align2, Color32, Context, FontId, Pos2, RichText, Stroke};
use engine_assets::{DirectoryArchive, FileSystem};
use engine_client::{EffectSystem, EffectWorld};
use engine_compile::water_pictures;
use engine_compile::{
    GRAPH_KINDS, GraphInput, MaterialLibrary, ShaderGraph, apply_bake, attach_side_files, compile,
    graph_code, graph_kind, keep_effect_meshes, map_hash, read_effect, read_effects_with,
    read_graph, read_map, write_graph, write_terrain,
};
use engine_core::entity::effect_id;
use engine_core::math::{Vec3, view_axes};
use engine_editor::camera::{Camera, FOV, Keys};
use engine_editor::effect_edit::{self, EMITTER_SETTINGS, FORCE_SETTINGS};
use engine_editor::graph_edit::{add_node, apply, join, remove_node, view_of};
use engine_editor::graph_view::{GraphView, ViewEvent, ViewWire};
use engine_format::{Block, Document, from_text, to_text};
use engine_map::{
    Map, SideFiles, Spline, TerrainBrush, TerrainData, TerrainLayer, TerrainSheet, TerrainStroke,
    Texture, stroke_land,
};
use engine_platform::{
    AppError, Frame, FrameOutcome, InputEvent, MouseButton, PlatformApp, ServiceOutcome, Window,
};
use engine_render::{
    Drawables, GpuParticles, LiveLight, MAX_LIVE_LIGHTS, ParticleDraw, ParticleSim, Renderer,
    RestPoses, Target, TextureFilter, TileStreamer, View, height_scale,
};
use engine_tools::{CELL, Plan, Session, Square, is_plan_map, prefab_names, run};
use serde_json::{Value, json};

use crate::easy::{
    Brush, Easy, LOOKS, PlanClick, PlanView, TOOLS, big_button, draw_plan, plan_keys, word_for,
};
use crate::panels::{ACCENT, apply_contrast, apply_style};
use engine_editor::ui_bridge::{UiInput, UiTextures, ui_frame};

/// How often the service runs.
const SERVICE_STEP: Duration = Duration::from_millis(8);

/// How many log lines stay.
const LOG_LINES: usize = 200;

/// How far the grid reaches from the camera, in cells.
const GRID_REACH: i32 = 24;

/// A grid cell's size in world units.
const GRID_CELL: f32 = 64.0;

/// Placed things land on multiples of this.
const SNAP: f32 = 8.0;

/// A placed thing, for listing and picking.
#[derive(Clone, Debug, PartialEq)]
struct Thing {
    /// Its ID.
    id: u64,
    /// What to call it.
    label: String,
    /// Its corners, if it has a box.
    bounds: Option<(Vec3, Vec3)>,
    /// Its fields, as text.
    fields: Vec<(String, String)>,
    /// What sort of block it is.
    kind: String,
    /// The points of its line, if it has one.
    points: Vec<Vec3>,
    /// Whether its line joins up.
    closed: bool,
}

/// What a click in the view does, besides selecting.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Tool {
    /// Places a prefab.
    Prefab(String),
    /// Places a mesh.
    Mesh(String),
    /// Presses a material on a surface.
    Decal(String),
    /// Presses an image into a surface.
    Stamp(String),
    /// Paints a layer, a point at a time.
    Paint(u8),
    /// Draws a spline, a point at a time.
    Spline,
    /// Applies a material to what's clicked.
    Material(String),
    /// Shapes land with a circle.
    Land(TerrainTool),
}

/// What a brush does to land.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TerrainTool {
    /// Lifts it.
    Raise,
    /// Lowers it.
    Lower,
    /// Brings it to the height it was first touched at.
    Flatten,
    /// Evens it out.
    Smooth,
    /// Cuts a hole.
    Hole,
    /// Fills a hole.
    Fill,
    /// Paints the layer that's chosen.
    Paint,
    /// Makes the water over land deeper.
    Water,
    /// Makes it shallower.
    Dry,
    /// Fills what's under the brush up to where the stroke began.
    Flood,
}

impl TerrainTool {
    /// What's on its button.
    const fn name(self) -> &'static str {
        match self {
            Self::Raise => "raise",
            Self::Lower => "lower",
            Self::Flatten => "flatten",
            Self::Smooth => "smooth",
            Self::Hole => "hole",
            Self::Fill => "fill",
            Self::Paint => "paint",
            Self::Water => "water",
            Self::Dry => "dry",
            Self::Flood => "flood",
        }
    }

    /// The tool called `name`.
    fn called(name: &str) -> Option<Self> {
        LAND_TOOLS.into_iter().find(|tool| tool.name() == name)
    }
}

/// Every land tool.
const LAND_TOOLS: [TerrainTool; 10] = [
    TerrainTool::Raise,
    TerrainTool::Lower,
    TerrainTool::Flatten,
    TerrainTool::Smooth,
    TerrainTool::Hole,
    TerrainTool::Fill,
    TerrainTool::Paint,
    TerrainTool::Water,
    TerrainTool::Dry,
    TerrainTool::Flood,
];

/// How far a brush goes in a step of a stroke, at full strength.
const LAND_STEP: f32 = 4.0;

/// How much of a layer a step of a stroke brings in, at full strength.
const PAINT_STEP: f32 = 0.2;

/// How wide a layer's repeat is, until it's said.
const USUAL_LAYER_SIZE: f32 = 512.0;

/// A sheet of land being shaped, and what it's called.
struct LandSheet {
    /// The name of its file.
    source: String,
    /// The land.
    sheet: TerrainSheet,
}

/// Where a ray first meets land, and which sheet that is.
fn ray_land(sheets: &[LandSheet], start: Vec3, direction: Vec3) -> Option<(usize, Vec3)> {
    let mut along = 0.0;
    while along < 20_000.0 {
        let at = start + direction * along;
        for (number, land) in sheets.iter().enumerate() {
            let height = land.sheet.height_at(at.x, at.y);
            let past = height.is_some_and(|height| {
                if land.sheet.facing_down {
                    at.z >= height
                } else {
                    at.z <= height
                }
            });
            if past {
                let height = height.unwrap_or(at.z);
                return Some((number, Vec3::new(at.x, at.y, height)));
            }
        }
        along += 8.0;
    }
    None
}

impl Tool {
    /// What the view says it's doing.
    fn describe(&self) -> String {
        match self {
            Self::Prefab(name) => format!("placing {name}: click to place"),
            Self::Mesh(name) => format!("placing mesh {name}: click to place"),
            Self::Decal(name) => format!("decal {name}: click a surface"),
            Self::Stamp(name) => format!("stamp {name}: click a surface"),
            Self::Paint(layer) => format!("painting layer {layer}: click along the stroke"),
            Self::Spline => "spline: click along the line, then End line".to_owned(),
            Self::Material(name) => format!("material {name}: click what to change"),
            Self::Land(tool) => format!("land: {}, drag across it", tool.name()),
        }
    }
}

/// Which list the left panel shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    /// Prefabs and meshes.
    Build,
    /// Decals, stamps and paint.
    Surfaces,
    /// Sky, fog, water, lights and splines.
    World,
    /// Materials.
    Materials,
    /// The map's own script.
    Script,
    /// A shader graph.
    Graph,
    /// An effect.
    Effects,
}

/// How far in front of the camera an effect's preview goes.
const PREVIEW_AWAY: f32 = 300.0;

/// How far below its preview the floor it can hit is.
const PREVIEW_FLOOR: f32 = 64.0;

/// How long after a change a preview takes it up, so that typing doesn't read files each time.
const PREVIEW_WAIT: f32 = 0.2;

/// An effect being worked on, and what's showing of it.
struct EffectEditing {
    /// What it's called, which says where it's kept.
    name: String,
    /// What it says.
    document: Document,
    /// What it's said, to go back to.
    undo: Vec<Document>,
    /// The box that's chosen.
    chosen: Option<usize>,
    /// What's been typed for a setting and not yet left.
    typing: Option<(String, String)>,
    /// The canvas.
    view: GraphView,
    /// Where it was last drawn.
    rect: Option<egui::Rect>,
    /// What's running.
    system: EffectSystem,
    /// Where the preview is, while there's one.
    place: Option<Vec3>,
    /// Whether what's running is behind what it says.
    behind: bool,
    /// How long since the last change.
    waited: f32,
    /// What's wrong with what it says.
    problem: Option<String>,
}

impl Default for EffectEditing {
    fn default() -> Self {
        Self {
            name: String::new(),
            document: effect_edit::blank(),
            undo: Vec::new(),
            chosen: None,
            typing: None,
            view: GraphView::default(),
            rect: None,
            system: EffectSystem::default(),
            place: None,
            behind: false,
            waited: 0.0,
            problem: None,
        }
    }
}

/// A shader graph being worked on.
#[derive(Default)]
struct GraphEditing {
    /// What it's called, which says where it's kept.
    name: String,
    /// The graph.
    graph: ShaderGraph,
    /// What it's been, to go back to.
    undo: Vec<ShaderGraph>,
    /// The node that's chosen.
    chosen: Option<usize>,
    /// The canvas.
    view: GraphView,
    /// Where it was last drawn.
    rect: Option<egui::Rect>,
}

/// The editor.
pub(crate) struct Editor {
    /// The map being edited.
    session: Session,
    /// The content directory.
    content: PathBuf,
    /// The window lease, which must outlive the renderer.
    window: Option<Window>,
    /// The renderer, once there's a window.
    renderer: Option<Renderer>,
    /// Keeps detailed light near the view, on a map whose light streams.
    tile_streamer: Option<TileStreamer>,
    /// The window's size.
    size: (u32, u32),
    /// The UI.
    context: Context,
    /// Input for the UI.
    input: UiInput,
    /// The UI's textures.
    textures: UiTextures,
    /// The camera.
    camera: Camera,
    /// Which movement keys are down.
    keys: Keys,
    /// Whether the right button is looking around.
    looking: bool,
    /// The last compile of the map, for drawing.
    map: Option<Arc<Map>>,
    /// Whether the map needs compiling again.
    stale: bool,
    /// What's in the map.
    things: Vec<Thing>,
    /// What's selected.
    selected: Option<u64>,
    /// What a click does.
    tool: Option<Tool>,
    /// The stroke or spline being drawn.
    drawing: Option<u64>,
    /// What the left panel shows.
    tab: Tab,
    /// The meshes and materials on offer.
    offered: (Vec<String>, Vec<String>),
    /// How big the next decal or stamp is, and how deep a stamp goes.
    press: (f64, f64),
    /// The map's land, as it's being shaped.
    land: Vec<LandSheet>,
    /// How much of a land brush fades, and how strong it is.
    land_brush: (f32, f32),
    /// The graph that's open.
    graph: GraphEditing,
    /// The effect that's open.
    effect: EffectEditing,
    /// How meshes that are particles stand.
    effect_meshes: RestPoses,
    /// The name and shader of the next field.
    next_field: (String, String),
    /// The sheet whose layers are shown, which is the one last touched.
    land_chosen: usize,
    /// The layer that's painted with.
    land_layer: usize,
    /// The material of the next layer.
    new_layer: String,
    /// What a stroke of land changed, newest last, to go back to.
    land_undo: Vec<(usize, TerrainData)>,
    /// The height a stroke of flattening began at.
    land_level: Option<f32>,
    /// The map's script, as it's being edited.
    script: String,
    /// The material being edited, and its text.
    material: (String, String),
    /// Where the view is in the window.
    view_rect: egui::Rect,
    /// Prefabs on offer.
    prefabs: Vec<String>,
    /// Parameters for the next prefab, as JSON.
    params: String,
    /// What's happened.
    log: Vec<String>,
    /// A new field's name and value, in the properties panel.
    new_field: (String, String),
    /// Lines from standard input.
    lines: Option<Receiver<String>>,
    /// A screenshot to take at the end of this frame.
    capture: Option<PathBuf>,
    /// Whether to stop.
    quit: bool,
    /// Whether the pointer is over the 3D view.
    over_view: bool,
    /// Whether a line command has said where the pointer is.
    pointer_told: bool,
    /// The easy way of working.
    easy: Easy,
}

/// Reads standard input on its own thread.
fn read_lines() -> Receiver<String> {
    let (sender, receiver) = channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    receiver
}

/// A value's text.
fn field_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items.iter().map(field_text).collect::<Vec<_>>().join(" "),
        other => other.to_string(),
    }
}

/// A point from three numbers.
#[expect(clippy::cast_possible_truncation, reason = "the view needs only f32")]
fn vec3(numbers: &[f64]) -> Vec3 {
    Vec3::new(numbers[0] as f32, numbers[1] as f32, numbers[2] as f32)
}

/// A thing from its JSON.
fn thing_from(json: &Value) -> Option<Thing> {
    let id = json.get("id")?.as_u64()?;
    let kind = json.get("kind").and_then(Value::as_str).unwrap_or("?");
    let label = json
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| json.get("type").and_then(Value::as_str))
        .or_else(|| json.get("prefab").and_then(Value::as_str))
        .unwrap_or(kind);
    let numbers = |key: &str| -> Option<Vec<f64>> {
        json.get(key)?
            .as_array()?
            .iter()
            .map(Value::as_f64)
            .collect()
    };
    let cuboid_of = |json: &Value| -> Option<(Vec3, Vec3)> {
        let parts: Vec<f64> = json
            .get("cuboid")?
            .as_array()?
            .iter()
            .map(Value::as_f64)
            .collect::<Option<_>>()?;
        (parts.len() == 6).then(|| (vec3(&parts[..3]), vec3(&parts[3..])))
    };
    let bounds = cuboid_of(json)
        .or_else(|| json.get("children")?.as_array()?.iter().find_map(cuboid_of))
        .or_else(|| {
            let origin = numbers("origin").filter(|parts| parts.len() == 3)?;
            let at = vec3(&origin);
            let reach = Vec3::new(16.0, 16.0, 16.0);
            Some((at - reach, at + reach))
        });
    let fields = json
        .as_object()?
        .iter()
        .filter(|(key, _)| !matches!(key.as_str(), "id" | "kind" | "children"))
        .map(|(key, value)| (key.clone(), field_text(value)))
        .collect();
    let points = numbers("points")
        .map(|parts| {
            parts
                .as_chunks::<3>()
                .0
                .iter()
                .map(|point| vec3(point))
                .collect()
        })
        .unwrap_or_default();
    Some(Thing {
        id,
        label: format!("{id}  {label}"),
        bounds,
        fields,
        kind: kind.to_owned(),
        points,
        closed: json.get("closed").and_then(Value::as_bool) == Some(true),
    })
}

/// Where a ray from `start` along `direction` first enters a box, as a
/// distance, using the slab method.
fn ray_box(start: Vec3, direction: Vec3, bounds: (Vec3, Vec3)) -> Option<f32> {
    let (low, high) = bounds;
    let parts = [
        (start.x, direction.x, low.x, high.x),
        (start.y, direction.y, low.y, high.y),
        (start.z, direction.z, low.z, high.z),
    ];
    let (mut near, mut far) = (0.0_f32, f32::INFINITY);
    for (from, along, min, max) in parts {
        if along.abs() < 1e-6 {
            if from < min.min(max) || from > min.max(max) {
                return None;
            }
            continue;
        }
        let (a, b) = ((min - from) / along, (max - from) / along);
        near = near.max(a.min(b));
        far = far.min(a.max(b));
        if near > far {
            return None;
        }
    }
    Some(near)
}

/// Which way the side of `bounds` nearest `point` faces.
fn box_normal(point: Vec3, bounds: (Vec3, Vec3)) -> [f64; 3] {
    let (low, high) = bounds;
    let sides = [
        ((point.x - low.x).abs(), [-1.0, 0.0, 0.0]),
        ((point.x - high.x).abs(), [1.0, 0.0, 0.0]),
        ((point.y - low.y).abs(), [0.0, -1.0, 0.0]),
        ((point.y - high.y).abs(), [0.0, 1.0, 0.0]),
        ((point.z - low.z).abs(), [0.0, 0.0, -1.0]),
        ((point.z - high.z).abs(), [0.0, 0.0, 1.0]),
    ];
    sides
        .iter()
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map_or([0.0, 0.0, 1.0], |side| side.1)
}

/// `value` on the snapping grid.
fn snapped(value: f32) -> f64 {
    f64::from((value / SNAP).round() * SNAP)
}

impl Editor {
    /// Sets up, opening `map` if it's given.
    pub(crate) fn new(content: &Path, map: Option<&str>, size: (u32, u32)) -> Result<Self, String> {
        let session = match map {
            Some(path) if Path::new(path).exists() => Session::open(Path::new(path), content)?,
            Some(path) => {
                let mut session = Session::new(content);
                session.save(Some(Path::new(path)))?;
                session
            }
            None => Session::new(content),
        };
        let context = Context::default();
        apply_style(&context);
        let mut editor = Self {
            session,
            content: content.to_owned(),
            window: None,
            renderer: None,
            tile_streamer: None,
            size,
            context,
            input: UiInput::default(),
            textures: UiTextures::default(),
            camera: Camera {
                origin: Vec3::new(0.0, 0.0, 128.0),
                yaw: 0.0,
                pitch: 10.0,
            },
            keys: Keys::default(),
            looking: false,
            map: None,
            stale: true,
            things: Vec::new(),
            selected: None,
            tool: None,
            drawing: None,
            tab: Tab::Build,
            offered: (Vec::new(), Vec::new()),
            press: (64.0, 8.0),
            land: Vec::new(),
            land_brush: (0.6, 1.0),
            graph: GraphEditing::default(),
            effect: EffectEditing::default(),
            effect_meshes: RestPoses::default(),
            next_field: (String::new(), String::new()),
            land_chosen: 0,
            land_layer: 0,
            new_layer: String::new(),
            land_undo: Vec::new(),
            land_level: None,
            script: String::new(),
            material: (String::new(), String::new()),
            view_rect: egui::Rect::NOTHING,
            prefabs: prefab_names(content),
            params: "{}".to_owned(),
            log: Vec::new(),
            new_field: (String::new(), String::new()),
            lines: Some(read_lines()),
            capture: None,
            quit: false,
            over_view: false,
            pointer_told: false,
            easy: Easy::default(),
        };
        editor.easy.on = is_plan_map(&editor.session);
        editor.refresh_plan();
        editor.refresh_things();
        editor.frame_start_position();
        if let Ok(listed) = run(&mut editor.session, "list", &json!({})) {
            let names = |key: &str| -> Vec<String> {
                listed
                    .get(key)
                    .and_then(Value::as_array)
                    .map(|names| {
                        names
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default()
            };
            editor.offered = (names("meshes"), names("materials"));
        }
        Ok(editor)
    }

    /// Puts the camera at the map's start, if it has one.
    fn frame_start_position(&mut self) {
        let start = self
            .things
            .iter()
            .find(|thing| thing.label.contains("info_player_start"))
            .and_then(|thing| thing.bounds);
        if let Some((low, high)) = start {
            self.camera.origin = (low + high) * 0.5 + Vec3::new(0.0, 0.0, 32.0);
        } else if let Some((x, y)) = self.easy.plan.floor.first() {
            #[expect(clippy::cast_possible_truncation, reason = "a square's place fits f32")]
            let middle = |at: i32| (f64::from(at) + 0.5) as f32 * CELL as f32;
            self.camera.origin = Vec3::new(middle(*x), middle(*y), 64.0);
        }
    }

    /// Adds a line to the log.
    fn note(&mut self, line: String) {
        self.log.push(line);
        if self.log.len() > LOG_LINES {
            self.log.remove(0);
        }
    }

    /// Runs an operation, as any front end would.
    fn operate(&mut self, name: &str, args: &Value) -> Result<Value, String> {
        let result = run(&mut self.session, name, args);
        match &result {
            Ok(answer) => {
                let short: String = answer.to_string().chars().take(100).collect();
                self.note(format!("{name}: {short}"));
            }
            Err(error) => self.note(format!("{name} failed: {error}")),
        }
        self.refresh_things();
        self.refresh_plan();
        self.stale = true;
        result
    }

    /// Rereads what's in the map.
    fn refresh_things(&mut self) {
        let Ok(described) = run(&mut self.session, "describe", &json!({ "limit": 0 })) else {
            return;
        };
        self.things = described
            .get("things")
            .and_then(Value::as_array)
            .map(|things| things.iter().filter_map(thing_from).collect())
            .unwrap_or_default();
    }

    /// Compiles the map for drawing, with its bake if that's up to date.
    fn rebuild(&mut self) {
        self.stale = false;
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        let text = self.session.text();
        let Ok(document) = from_text(&text) else {
            return;
        };
        let mut working = match read_map(&document) {
            Ok(working) => working,
            Err(error) => {
                self.log.push(format!("map: {error}"));
                return;
            }
        };
        let mut filesystem = FileSystem::new();
        filesystem.mount(Box::new(DirectoryArchive::new(&self.content)));
        let mut library = MaterialLibrary::new(&filesystem);
        let used = match library.prepare(&mut working) {
            Ok(used) => used,
            Err(error) => {
                self.log.push(format!("compile: {error}"));
                return;
            }
        };
        let water_pictures = water_pictures(&mut library, &working);
        if let Err(problem) = keep_effect_meshes(&filesystem, &mut library, &mut working) {
            self.log.push(format!("effects: {problem}"));
        }
        let mut textures = |material: &str| {
            library
                .load(material)
                .unwrap_or_else(|_| Texture::default())
        };
        let mut map = match compile(&working, &mut textures) {
            Ok(map) => map,
            Err(error) => {
                self.log.push(format!("compile: {error}"));
                return;
            }
        };
        match water_pictures {
            Ok(pictures) => map.water_pictures = pictures,
            Err(problem) => self.log.push(format!("water: {problem}")),
        }
        if let Some(path) = saved_map_path(&self.session)
            && let Ok(bytes) = std::fs::read(path.with_extension("ebake"))
        {
            let _applied = apply_bake(&mut map, &bytes, map_hash(&document, &used));
            attach_beside(&mut map, &path);
        }
        self.land = working
            .terrain
            .iter()
            .zip(&map.terrain)
            .map(|(source, sheet)| LandSheet {
                source: source.source.clone(),
                sheet: sheet.clone(),
            })
            .collect();
        let map = Arc::new(map);
        self.tile_streamer = TileStreamer::new(&map);
        if let Err(error) = renderer.load_map(Arc::clone(&map)) {
            self.log.push(format!("render: {error}"));
        }
        self.map = Some(map);
        // A new load has none of what the preview shows.
        self.effect.behind = self.effect.place.is_some();
    }

    /// Takes in a JSON line from standard input.
    fn line(&mut self, line: &str) {
        let Ok(request) = serde_json::from_str::<Value>(line) else {
            emit(&json!({ "error": "not JSON" }));
            return;
        };
        let name = request.get("op").and_then(Value::as_str).unwrap_or("");
        let args = request.get("args").cloned().unwrap_or_else(|| json!({}));
        match name {
            "easy" => {
                if let Some(on) = args.get("on").and_then(Value::as_bool) {
                    self.easy.on = on;
                }
                if let Some(seeing) = args.get("seeing").and_then(Value::as_bool) {
                    self.easy.seeing = seeing;
                    self.frame_start_position();
                }
                emit(&json!({ "ok": true }));
            }
            "capture" => {
                let path = args
                    .get("path")
                    .and_then(Value::as_str)
                    .unwrap_or("editor.png");
                self.capture = Some(PathBuf::from(path));
                emit(&json!({ "path": path }));
            }
            "camera" => {
                if let Some(parts) = args.get("origin").and_then(Value::as_array) {
                    let parts: Option<Vec<f64>> = parts.iter().map(Value::as_f64).collect();
                    if let Some(parts) = parts.filter(|parts| parts.len() == 3) {
                        self.camera.origin = vec3(&parts);
                    }
                }
                #[expect(clippy::cast_possible_truncation, reason = "angles fit f32")]
                if let Some(yaw) = args.get("yaw").and_then(Value::as_f64) {
                    self.camera.yaw = yaw as f32;
                }
                #[expect(clippy::cast_possible_truncation, reason = "angles fit f32")]
                if let Some(pitch) = args.get("pitch").and_then(Value::as_f64) {
                    self.camera.pitch = pitch as f32;
                }
                emit(&json!({ "ok": true }));
            }
            "tool" => {
                let name = args.get("name").and_then(Value::as_str).map(str::to_owned);
                let prefab = args
                    .get("prefab")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let layer = args.get("layer").and_then(Value::as_u64).unwrap_or(0);
                if let Some(size) = args.get("size").and_then(Value::as_f64) {
                    self.press.0 = size;
                }
                if args.get("layer").is_some() {
                    self.land_layer = usize::try_from(layer).unwrap_or(0);
                }
                self.set_tool(match args.get("kind").and_then(Value::as_str) {
                    Some("mesh") => name.map(Tool::Mesh),
                    Some("decal") => name.map(Tool::Decal),
                    Some("stamp") => name.map(Tool::Stamp),
                    Some("material") => name.map(Tool::Material),
                    Some("paint") => Some(Tool::Paint(u8::try_from(layer).unwrap_or(0))),
                    Some("spline") => Some(Tool::Spline),
                    Some("land") => name
                        .and_then(|name| TerrainTool::called(&name))
                        .map(Tool::Land),
                    _ => prefab.or(name).map(Tool::Prefab),
                });
                if let Some(params) = args.get("params") {
                    self.params = params.to_string();
                }
                emit(&json!({ "tool": self.tool.as_ref().map(Tool::describe) }));
            }
            "tab" => {
                self.tab = match args.get("name").and_then(Value::as_str) {
                    Some("graph") => Tab::Graph,
                    Some("effects") => Tab::Effects,
                    Some("surfaces") => Tab::Surfaces,
                    _ => Tab::Build,
                };
                emit(&json!({ "ok": true }));
            }
            "mouse" => {
                let number = |key: &str| args.get(key).and_then(Value::as_f64).unwrap_or(0.0);
                self.input.event(&InputEvent::Pointer {
                    x: number("x"),
                    y: number("y"),
                });
                if let Some(pressed) = args.get("down").and_then(Value::as_bool) {
                    self.input.event(&InputEvent::Button {
                        button: MouseButton::Left,
                        pressed,
                    });
                }
                emit(&json!({ "ok": true }));
            }
            "effect_open" => {
                let name = args.get("name").and_then(Value::as_str).unwrap_or("");
                self.effect.name = name.to_owned();
                self.open_effect();
                emit(
                    &json!({ "emitters": effect_edit::emitter_blocks(&self.effect.document).len() }),
                );
            }
            "effect_name" => {
                let name = args.get("name").and_then(Value::as_str).unwrap_or("");
                self.effect.name = name.to_owned();
                emit(&json!({ "ok": true }));
            }
            "effect_add" => {
                self.add_emitter();
                emit(
                    &json!({ "emitters": effect_edit::emitter_blocks(&self.effect.document).len() }),
                );
            }
            "effect_set" => {
                let text = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("");
                let number = args.get("emitter").and_then(Value::as_u64).unwrap_or(1);
                let number = usize::try_from(number).unwrap_or(1);
                let told = self.effect_setting(number, text("key"), text("value"));
                emit(&json!({ "problem": told.err() }));
            }
            "effect_join" => {
                let number = |key: &str| {
                    let found = args.get(key).and_then(Value::as_u64).unwrap_or(0);
                    usize::try_from(found).unwrap_or(0)
                };
                let wire = ViewWire {
                    from: number("from"),
                    to: number("to"),
                    input: number("input"),
                };
                let done = self.effect_changes(ViewEvent::Joined(wire));
                emit(&json!({ "joined": done }));
            }
            "effect_play" => {
                self.play_effect();
                emit(&json!({ "ok": true }));
            }
            "effect_stop" => {
                self.stop_effect();
                emit(&json!({ "ok": true }));
            }
            "effect_save" => {
                self.save_effect();
                emit(&json!({ "problem": self.effect.problem }));
            }
            "effect_state" => {
                let effect = &self.effect;
                emit(&json!({
                    "going": effect.system.going(),
                    "particles": effect.system.count(),
                    "computed": effect.system.sims().len(),
                    "problem": effect.problem,
                }));
            }
            "graph_open" => {
                let name = args.get("name").and_then(Value::as_str).unwrap_or("");
                self.graph.name = name.to_owned();
                self.open_graph();
                emit(&json!({ "nodes": self.graph.graph.nodes.len() }));
            }
            "graph_name" => {
                let name = args.get("name").and_then(Value::as_str).unwrap_or("");
                self.graph.name = name.to_owned();
                emit(&json!({ "ok": true }));
            }
            "graph_add" => {
                let kind = args.get("kind").and_then(Value::as_str).unwrap_or("");
                self.add_graph_node(kind);
                emit(&json!({ "nodes": self.graph.graph.nodes.len() }));
            }
            "graph_join" => {
                let text = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("");
                let before = self.graph.graph.clone();
                let done = join(
                    &mut self.graph.graph,
                    text("from"),
                    text("to"),
                    text("input"),
                );
                if done {
                    self.graph.undo.push(before);
                }
                emit(&json!({ "joined": done }));
            }
            "graph_save" => {
                self.save_graph();
                let told = graph_code(&self.graph.graph);
                emit(&json!({ "code": told.as_ref().ok(), "problems": told.as_ref().err() }));
            }
            "land_layer" => {
                let material = args.get("material").and_then(Value::as_str).unwrap_or("");
                self.add_land_layer(material);
                emit(&json!({ "layers": self.land_layers().len() }));
            }
            "pointer" => {
                #[expect(clippy::cast_possible_truncation, reason = "pixels fit f32")]
                let pixel = (
                    args.get("x").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                    args.get("y").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                );
                self.input.pointer = pixel;
                self.pointer_told = true;
                emit(&json!({ "ok": true }));
            }
            "click" => {
                #[expect(clippy::cast_possible_truncation, reason = "pixels fit f32")]
                let pixel = (
                    args.get("x").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                    args.get("y").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                );
                self.click(pixel);
                emit(&json!({ "selected": self.selected, "things": self.things.len() }));
            }
            "select" => {
                self.selected = args.get("id").and_then(Value::as_u64);
                emit(&json!({ "selected": self.selected }));
            }
            "quit" => {
                self.quit = true;
                emit(&json!({ "ok": true }));
            }
            _ => match self.operate(name, &args) {
                Ok(answer) => emit(&answer),
                Err(error) => emit(&json!({ "error": error })),
            },
        }
    }

    /// The view's size.
    fn view_size(&self) -> (f32, f32) {
        let size = self.view_rect.size();
        if size.x > 1.0 && size.y > 1.0 {
            (size.x, size.y)
        } else {
            #[expect(clippy::cast_precision_loss, reason = "window sizes are small")]
            (self.size.0 as f32, self.size.1 as f32)
        }
    }

    /// Chooses what a click does, ending any line being drawn.
    fn set_tool(&mut self, tool: Option<Tool>) {
        self.drawing = None;
        self.tool = tool;
    }

    /// Where `pixel` is looking, from the camera.
    fn pointer_ray(&self, pixel: (f32, f32)) -> (Vec3, Vec3) {
        let corner = self.view_rect.min;
        let inside = if self.view_rect.is_positive() {
            (pixel.0 - corner.x, pixel.1 - corner.y)
        } else {
            pixel
        };
        (
            self.camera.origin,
            self.camera.ray(inside, self.view_size()),
        )
    }

    /// The brush's size, as a radius.
    #[expect(clippy::cast_possible_truncation, reason = "land is in f32")]
    fn brush_radius(&self) -> f32 {
        self.press.0 as f32 * 0.5
    }

    /// One step of a land brush under `pixel`. What it changes goes straight to the screen, and the
    /// map's built again when the stroke ends.
    fn land_step(&mut self, tool: TerrainTool, pixel: (f32, f32)) {
        let (start, direction) = self.pointer_ray(pixel);
        let Some((number, place)) = ray_land(&self.land, start, direction) else {
            return;
        };
        let level = *self.land_level.get_or_insert(place.z);
        let (soft, strength) = self.land_brush;
        let radius = self.brush_radius();
        let Some(land) = self.land.get_mut(number) else {
            return;
        };
        let origin = land.sheet.origin;
        let brush = TerrainBrush {
            middle: [place.x - origin.x, place.y - origin.y],
            radius,
            soft,
        };
        let stroke = match tool {
            TerrainTool::Raise => TerrainStroke::Raise(LAND_STEP * strength),
            TerrainTool::Lower => TerrainStroke::Raise(-LAND_STEP * strength),
            TerrainTool::Flatten => TerrainStroke::Flatten(level - origin.z),
            TerrainTool::Smooth => TerrainStroke::Smooth(strength.min(1.0) * 0.5),
            TerrainTool::Hole => TerrainStroke::Hole(true),
            TerrainTool::Fill => TerrainStroke::Hole(false),
            TerrainTool::Paint => TerrainStroke::Paint(self.land_layer, strength * PAINT_STEP),
            TerrainTool::Water => TerrainStroke::Water(LAND_STEP * strength),
            TerrainTool::Dry => TerrainStroke::Water(-LAND_STEP * strength),
            TerrainTool::Flood => TerrainStroke::Flood(level - origin.z),
        };
        stroke_land(&mut land.sheet.data, &brush, stroke);
        if let Some(renderer) = &mut self.renderer
            && let Err(error) = renderer.update_land(number, &land.sheet)
        {
            self.log.push(format!("render: {error}"));
        }
    }

    /// Notes what land is before a stroke, so that it can be gone back to.
    fn land_stroke_begins(&mut self, pixel: (f32, f32)) {
        let (start, direction) = self.pointer_ray(pixel);
        self.land_level = None;
        if let Some((number, _)) = ray_land(&self.land, start, direction)
            && let Some(land) = self.land.get(number)
        {
            self.land_undo.push((number, land.sheet.data.clone()));
            self.land_chosen = number;
        }
    }

    /// Keeps what a stroke did, and builds the map again.
    fn land_stroke_ends(&mut self) {
        let folder = self.content.join("terrain");
        for land in &self.land {
            let file = folder.join(format!("{}.eterrain", land.source));
            if let Err(error) = std::fs::write(&file, write_terrain(&land.sheet.data)) {
                self.log.push(format!("{}: {error}", file.display()));
            }
        }
        self.stale = true;
    }

    /// What the chosen sheet's painted with.
    fn land_layers(&self) -> Vec<String> {
        self.land
            .get(self.land_chosen)
            .map_or_else(Vec::new, |land| {
                let layers = land.sheet.data.layers.iter();
                layers.map(|layer| layer.material.clone()).collect()
            })
    }

    /// Paints the land with `material` too, and builds the map again so that its pictures load.
    fn add_land_layer(&mut self, material: &str) {
        let Some(land) = self.land.get_mut(self.land_chosen) else {
            self.note("there's no land to paint".to_owned());
            return;
        };
        let data = &mut land.sheet.data;
        let next = data.layers.len();
        let layer = TerrainLayer {
            material: material.to_owned(),
            size: USUAL_LAYER_SIZE,
            hex: false,
        };
        if material.is_empty() || !data.set_layer(next, layer) {
            self.note("that can't be a layer: name a material, up to eight of them".to_owned());
            return;
        }
        self.land_layer = next;
        self.land_stroke_ends();
    }

    /// A shader graph: the kinds to add, a node's values, what's wrong, and the code it comes to.
    fn graph_tab(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("GRAPH").color(ACCENT));
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.graph.name);
        });
        ui.horizontal(|ui| {
            if ui.button("Open").clicked() {
                self.open_graph();
            }
            if ui.button("Save").clicked() {
                self.save_graph();
            }
            if ui.button("Undo").clicked()
                && let Some(last) = self.graph.undo.pop()
            {
                self.graph.graph = last;
            }
        });
        egui::ScrollArea::vertical()
            .id_salt("kinds")
            .max_height(220.0)
            .show(ui, |ui| {
                for group in ["there", "values", "sums", "code"] {
                    ui.label(RichText::new(group).color(ACCENT));
                    ui.horizontal_wrapped(|ui| {
                        for kind in GRAPH_KINDS.iter().filter(|kind| kind.group == group) {
                            if ui.button(kind.name).clicked() {
                                self.add_graph_node(kind.name);
                            }
                        }
                    });
                }
            });
        self.chosen_node(ui);
        ui.label(RichText::new("WHAT IT MAKES").color(ACCENT));
        match graph_code(&self.graph.graph) {
            Ok(code) => {
                egui::ScrollArea::vertical()
                    .id_salt("graph code")
                    .max_height(160.0)
                    .show(ui, |ui| {
                        ui.label(RichText::new(code).monospace().size(10.0));
                    });
            }
            Err(problems) => {
                for problem in problems {
                    ui.colored_label(ACCENT, problem);
                }
            }
        }
    }

    /// What can be done with an effect, and what its chosen box says.
    fn effects_tab(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("EFFECT").color(ACCENT));
        ui.text_edit_singleline(&mut self.effect.name);
        ui.horizontal(|ui| {
            if ui.button("New").clicked() {
                self.new_effect();
            }
            if ui.button("Open").clicked() {
                self.open_effect();
            }
            if ui.button("Save").clicked() {
                self.save_effect();
            }
            if ui.button("Undo").clicked()
                && let Some(last) = self.effect.undo.pop()
            {
                self.effect.document = last;
                self.effect_is_behind();
            }
        });
        ui.horizontal(|ui| {
            let playing = self.effect.place.is_some();
            if ui
                .button(if playing { "Move here" } else { "Play" })
                .clicked()
            {
                self.play_effect();
            }
            if ui.add_enabled(playing, egui::Button::new("Stop")).clicked() {
                self.stop_effect();
            }
            if ui.button("Add emitter").clicked() {
                self.add_emitter();
            }
        });
        if let Some(problem) = &self.effect.problem {
            ui.colored_label(ACCENT, problem);
        }
        self.effect_timeline(ui);
        egui::ScrollArea::vertical()
            .id_salt("emitter")
            .max_height(420.0)
            .show(ui, |ui| self.chosen_emitter(ui));
    }

    /// When the timeline starts what it starts.
    fn effect_timeline(&mut self, ui: &mut egui::Ui) {
        let times = effect_edit::timeline_of(&self.effect.document);
        if times.is_empty() {
            return;
        }
        ui.label(RichText::new("TIMELINE").color(ACCENT));
        for (at, mut time, start) in times {
            ui.horizontal(|ui| {
                let drag = egui::DragValue::new(&mut time)
                    .speed(0.01)
                    .range(0.0..=60.0);
                if ui.add(drag).changed() {
                    effect_edit::set_timeline(&mut self.effect.document, at, time);
                    self.effect_is_behind();
                }
                ui.label(start);
            });
        }
    }

    /// The settings of the emitter that's chosen.
    fn chosen_emitter(&mut self, ui: &mut egui::Ui) {
        let Some(number) = self.effect.chosen.filter(|number| *number > 0) else {
            ui.label("Choose an emitter below");
            return;
        };
        let Some(block) = effect_edit::emitter_of(&self.effect.document, number) else {
            return;
        };
        let forces = block.children_of("force").count();
        let mut name = block.name.clone().unwrap_or_default();
        ui.label(RichText::new("EMITTER").color(ACCENT));
        if ui.text_edit_singleline(&mut name).lost_focus() {
            let before = self.effect.document.clone();
            match effect_edit::rename(&mut self.effect.document, number, &name) {
                Ok(()) => {
                    self.effect.undo.push(before);
                    self.effect_is_behind();
                }
                Err(problem) => self.effect.problem = Some(problem),
            }
        }
        for (key, hint) in EMITTER_SETTINGS {
            self.effect_row(ui, number, None, key, hint);
        }
        for force in 0..forces {
            ui.label(RichText::new("FORCE").color(ACCENT));
            for (key, hint) in FORCE_SETTINGS {
                self.effect_row(ui, number, Some(force), key, hint);
            }
            if ui.button("Remove force").clicked()
                && let Some(block) = effect_edit::emitter_mut(&mut self.effect.document, number)
            {
                let mut seen = 0;
                block.children.retain(|child| {
                    let this = child.kind == "force" && {
                        seen += 1;
                        seen - 1 == force
                    };
                    !this
                });
                self.effect_is_behind();
            }
        }
        ui.horizontal(|ui| {
            if ui.button("Add force").clicked()
                && let Some(block) = effect_edit::emitter_mut(&mut self.effect.document, number)
            {
                let mut force = Block::new("force");
                force.set("kind", engine_format::Value::Text("wind".to_owned()));
                block.children.push(force);
                self.effect_is_behind();
            }
            if ui.button("Remove").clicked() {
                self.effect.undo.push(self.effect.document.clone());
                effect_edit::remove_emitter(&mut self.effect.document, number);
                self.effect.chosen = None;
                self.effect_is_behind();
            }
        });
    }

    /// A row for `key`, of the emitter of box `number`, or one of its forces.
    fn effect_row(
        &mut self,
        ui: &mut egui::Ui,
        number: usize,
        force: Option<usize>,
        key: &str,
        hint: &str,
    ) {
        let id = format!("{number}/{force:?}/{key}");
        let block = effect_edit::emitter_of(&self.effect.document, number);
        let block = block.and_then(|block| match force {
            Some(force) => block.children_of("force").nth(force),
            None => Some(block),
        });
        let said = block.map_or_else(String::new, |block| effect_edit::setting(block, key));
        let mut text = match &self.effect.typing {
            Some((typing, text)) if *typing == id => text.clone(),
            _ => said,
        };
        let changed = ui
            .horizontal(|ui| {
                ui.add_sized([74.0, 18.0], egui::Label::new(key).truncate())
                    .on_hover_text(hint);
                let field = egui::TextEdit::singleline(&mut text).hint_text(hint);
                ui.add(field)
            })
            .inner;
        if changed.gained_focus() {
            self.effect.undo.push(self.effect.document.clone());
        }
        if changed.changed() {
            self.effect.typing = Some((id, text.clone()));
            let found = effect_edit::emitter_mut(&mut self.effect.document, number);
            let found = found.and_then(|block| match force {
                Some(force) => block
                    .children
                    .iter_mut()
                    .filter(|b| b.kind == "force")
                    .nth(force),
                None => Some(block),
            });
            if let Some(block) = found {
                match effect_edit::set_setting(block, key, &text) {
                    Ok(()) => self.effect_is_behind(),
                    Err(problem) => self.effect.problem = Some(problem),
                }
            }
        }
        if changed.lost_focus() {
            self.effect.typing = None;
        }
    }

    /// Says what's typed for `key`, as a setting of the emitter of box `number`.
    fn effect_setting(&mut self, number: usize, key: &str, text: &str) -> Result<(), String> {
        let before = self.effect.document.clone();
        let Some(block) = effect_edit::emitter_mut(&mut self.effect.document, number) else {
            return Err(format!("there's no emitter {number}"));
        };
        effect_edit::set_setting(block, key, text)?;
        self.effect.undo.push(before);
        self.effect_is_behind();
        Ok(())
    }

    /// The effect's boxes.
    fn effect_canvas(&mut self, ui: &mut egui::Ui) {
        let rect = ui.max_rect();
        self.effect.rect = Some(rect);
        let (nodes, wires) = effect_edit::view_of(&self.effect.document);
        let chosen = self.effect.chosen;
        let events = self
            .effect
            .view
            .show(ui, rect, (&nodes, &wires), chosen, ACCENT);
        for event in events {
            let _changed = self.effect_changes(event);
        }
    }

    /// Does what was done in the node view of an effect, and says if that changed it.
    fn effect_changes(&mut self, event: ViewEvent) -> bool {
        if let ViewEvent::Chosen(number) = event {
            self.effect.chosen = Some(number);
            self.effect.typing = None;
            return false;
        }
        let before = self.effect.document.clone();
        let changed = effect_edit::apply(&mut self.effect.document, event);
        if changed {
            self.effect.undo.push(before);
            self.effect_is_behind();
        }
        changed
    }

    /// Adds an emitter, in the middle of what's showing.
    fn add_emitter(&mut self) {
        let before = self.effect.document.clone();
        let middle = self
            .effect
            .rect
            .map_or([240.0, 100.0], |rect| self.effect.view.middle(rect));
        let count = effect_edit::emitter_blocks(&before).len();
        // In rows, as a box is wider than the steps of a graph's.
        #[expect(clippy::cast_precision_loss, reason = "an effect has few emitters")]
        let (across, down) = ((count % 3) as f32 * 190.0, (count / 3 % 3) as f32 * 110.0);
        let at = [middle[0] - 190.0 + across, middle[1] - 110.0 + down];
        let _name = effect_edit::add_emitter(&mut self.effect.document, at);
        self.effect.undo.push(before);
        self.effect.chosen = Some(count + 1);
        self.effect_is_behind();
    }

    /// Says what's running needs to catch up.
    fn effect_is_behind(&mut self) {
        self.effect.behind = true;
        self.effect.waited = 0.0;
        self.effect.problem = None;
    }

    /// Where the file of the effect that's open is.
    fn effect_file(&self) -> PathBuf {
        let name = format!("{}.eeffect", self.effect.name.trim());
        self.content.join("effects").join(name)
    }

    /// Starts on a new effect.
    fn new_effect(&mut self) {
        self.effect.undo.push(self.effect.document.clone());
        self.effect.document = effect_edit::blank();
        self.effect.chosen = Some(1);
        self.effect_is_behind();
    }

    /// Reads the effect of the name that's given.
    fn open_effect(&mut self) {
        let file = self.effect_file();
        let read = std::fs::read_to_string(&file)
            .map_err(|error| error.to_string())
            .and_then(|text| from_text(&text).map_err(|error| error.to_string()));
        match read {
            Ok(document) => {
                self.effect.document = document;
                self.effect.undo.clear();
                self.effect.chosen = None;
                self.effect_is_behind();
            }
            Err(problem) => self.note(format!("{}: {problem}", file.display())),
        }
    }

    /// Keeps the effect, if it reads. What's wrong stays with it, and isn't written.
    fn save_effect(&mut self) {
        if self.effect.name.trim().is_empty() {
            self.effect.problem = Some("an effect needs a name".to_owned());
            return;
        }
        if let Err(problem) = read_effect(self.effect.name.trim(), &self.effect.document) {
            self.effect.problem = Some(problem.to_string());
            return;
        }
        let file = self.effect_file();
        if let Some(folder) = file.parent() {
            let _made = std::fs::create_dir_all(folder);
        }
        match std::fs::write(&file, to_text(&self.effect.document)) {
            Ok(()) => self.note(format!("wrote {}", file.display())),
            Err(error) => self.note(format!("{}: {error}", file.display())),
        }
    }

    /// Shows the effect, in front of the camera.
    fn play_effect(&mut self) {
        let [forward, _, _] = view_axes(self.camera.yaw, self.camera.pitch);
        self.effect.place = Some(self.camera.origin + forward * PREVIEW_AWAY);
        self.effect.system = EffectSystem::default();
        self.effect.behind = true;
        self.effect.waited = PREVIEW_WAIT;
    }

    /// Stops showing it.
    fn stop_effect(&mut self) {
        self.effect.place = None;
        self.effect.system = EffectSystem::default();
        self.effect.behind = false;
    }

    /// The name what's showing goes by.
    fn preview_name(&self) -> String {
        let name = self.effect.name.trim();
        if name.is_empty() {
            "preview".to_owned()
        } else {
            name.to_owned()
        }
    }

    /// Gives what's showing what the effect now says. What's wrong leaves it as it was.
    fn read_preview(&mut self) {
        self.effect.behind = false;
        let name = self.preview_name();
        let edited = match read_effect(&name, &self.effect.document) {
            Ok(edited) => edited,
            Err(problem) => {
                self.effect.problem = Some(problem.to_string());
                return;
            }
        };
        let mut filesystem = FileSystem::new();
        filesystem.mount(Box::new(DirectoryArchive::new(&self.content)));
        let level = self.map.clone().unwrap_or_default();
        let read = match read_effects_with(&filesystem, &level, edited) {
            Ok(read) => read,
            Err(problem) => {
                self.effect.problem = Some(problem.to_string());
                return;
            }
        };
        self.effect.system.change_effects(&read.effects);
        if let Some(name) = read.missing_meshes.first() {
            self.effect.problem = Some(format!("save it to see {name}, as the map's built again"));
            self.stale = true;
        }
        if let Some(renderer) = &mut self.renderer {
            let held = Map {
                effects: read.effects,
                effect_pictures: read.pictures,
                effect_shaders: read.shaders,
                effect_motions: read.motions,
                fields: level.fields.clone(),
                ..Map::default()
            };
            if let Err(problem) = renderer.reload_effects(&held) {
                self.effect.problem = Some(problem.to_string());
            }
            let told = renderer.take_shader_problems();
            if !told.is_empty() {
                self.effect.problem = Some(told.join("\n"));
            }
        }
    }

    /// Moves what's showing on by `seconds`. When it's over, it goes again.
    fn run_preview(&mut self, seconds: f32) {
        let Some(place) = self.effect.place else {
            return;
        };
        self.effect.waited += seconds;
        if self.effect.behind && self.effect.waited >= PREVIEW_WAIT {
            self.read_preview();
        }
        let id = effect_id(&self.preview_name());
        if self.effect.system.going() == 0 {
            self.effect.system.start(id, place, None, false);
        }
        let floor = place.z - PREVIEW_FLOOR;
        let hit = |from: Vec3, to: Vec3| {
            (from.z >= floor && to.z < floor).then(|| {
                let share = (from.z - floor) / (from.z - to.z);
                (from + (to - from) * share, Vec3::new(0.0, 0.0, 1.0))
            })
        };
        let world = EffectWorld {
            place: &|_| None,
            hit: &hit,
        };
        self.effect.system.set_eye(self.camera.origin);
        self.effect.system.update(seconds, &world);
    }

    /// The values of the node that's chosen.
    fn chosen_node(&mut self, ui: &mut egui::Ui) {
        let Some(at) = self
            .graph
            .chosen
            .filter(|at| *at < self.graph.graph.nodes.len())
        else {
            return;
        };
        let id = self.graph.graph.nodes[at].id.clone();
        ui.label(RichText::new(&id).color(ACCENT));
        let kind = graph_kind(&self.graph.graph.nodes[at].kind);
        let node = &mut self.graph.graph.nodes[at];
        for setting in kind.map_or(&[][..], |kind| kind.settings) {
            ui.label(*setting);
            let value = node.settings.entry((*setting).to_owned()).or_default();
            ui.text_edit_singleline(value);
        }
        for (name, default) in kind.map_or(&[][..], |kind| kind.inputs) {
            if matches!(node.inputs.get(*name), Some(GraphInput::Node(_))) {
                continue;
            }
            let mut value = match node.inputs.get(*name) {
                Some(GraphInput::Number(number)) => *number,
                _ => *default,
            };
            ui.horizontal(|ui| {
                ui.label(*name);
                if ui
                    .add(egui::DragValue::new(&mut value).speed(0.01))
                    .changed()
                {
                    node.inputs
                        .insert((*name).to_owned(), GraphInput::Number(value));
                }
            });
        }
        if ui.button("Remove").clicked() {
            self.graph.undo.push(self.graph.graph.clone());
            remove_node(&mut self.graph.graph, &id);
            self.graph.chosen = None;
        }
    }

    /// The boxes and curves of the graph.
    fn graph_canvas(&mut self, ui: &mut egui::Ui) {
        let rect = ui.max_rect();
        self.graph.rect = Some(rect);
        let (nodes, wires) = view_of(&self.graph.graph);
        let chosen = self.graph.chosen;
        let events = self
            .graph
            .view
            .show(ui, rect, (&nodes, &wires), chosen, ACCENT);
        for event in events {
            self.graph_event(event);
        }
    }

    /// Does what was done in the node view.
    fn graph_event(&mut self, event: ViewEvent) {
        if let ViewEvent::Chosen(number) = event {
            self.graph.chosen = Some(number);
            return;
        }
        let before = self.graph.graph.clone();
        if apply(&mut self.graph.graph, event) {
            self.graph.undo.push(before);
        }
    }

    /// Adds a node, in the middle of what's showing.
    fn add_graph_node(&mut self, kind: &str) {
        let before = self.graph.graph.clone();
        let middle = self
            .graph
            .rect
            .map_or([100.0, 100.0], |rect| self.graph.view.middle(rect));
        // Each a little from the last, so that one doesn't hide another.
        #[expect(clippy::cast_precision_loss, reason = "a graph has few nodes")]
        let step = (self.graph.graph.nodes.len() % 8) as f32 * 26.0;
        let at = [middle[0] - 200.0 + step, middle[1] - 100.0 + step];
        if add_node(&mut self.graph.graph, kind, at).is_some() {
            self.graph.undo.push(before);
            self.graph.chosen = Some(self.graph.graph.nodes.len() - 1);
        }
    }

    /// Where the file of the graph that's open is.
    fn graph_file(&self) -> PathBuf {
        let name = format!("{}.egraph", self.graph.name.trim());
        self.content.join("shaders").join(name)
    }

    /// Reads the graph of the name that's given.
    fn open_graph(&mut self) {
        let file = self.graph_file();
        let read = std::fs::read_to_string(&file)
            .map_err(|error| error.to_string())
            .and_then(|text| read_graph(&text));
        match read {
            Ok(graph) => {
                self.graph.graph = graph;
                self.graph.undo.clear();
                self.graph.chosen = None;
            }
            Err(problem) => self.note(format!("{}: {problem}", file.display())),
        }
    }

    /// Keeps the graph, and builds the map again, as what's in the level is its preview.
    fn save_graph(&mut self) {
        if self.graph.name.trim().is_empty() {
            self.note("a graph needs a name".to_owned());
            return;
        }
        let file = self.graph_file();
        if let Some(folder) = file.parent() {
            let _made = std::fs::create_dir_all(folder);
        }
        match std::fs::write(&file, write_graph(&self.graph.graph)) {
            Ok(()) => {
                self.note(format!("wrote {}", file.display()));
                self.stale = true;
            }
            Err(error) => self.note(format!("{}: {error}", file.display())),
        }
    }

    /// Goes back on the last stroke of land.
    fn undo_land(&mut self) {
        let Some((number, data)) = self.land_undo.pop() else {
            return;
        };
        if let Some(land) = self.land.get_mut(number) {
            land.sheet.data = data;
        }
        self.land_stroke_ends();
    }

    /// A click in the 3D view: uses the tool, or selects.
    fn click(&mut self, pixel: (f32, f32)) {
        if let Some(Tool::Land(tool)) = self.tool {
            self.land_stroke_begins(pixel);
            self.land_step(tool, pixel);
            self.land_stroke_ends();
            return;
        }
        let corner = self.view_rect.min;
        let inside = if self.view_rect.is_positive() {
            (pixel.0 - corner.x, pixel.1 - corner.y)
        } else {
            pixel
        };
        let direction = self.camera.ray(inside, self.view_size());
        let start = self.camera.origin;
        let hit = self
            .things
            .iter()
            .filter_map(|thing| {
                let bounds = thing.bounds?;
                Some((ray_box(start, direction, bounds)?, thing.id, bounds))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let Some(tool) = self.tool.clone() else {
            self.selected = hit.map(|(_, id, _)| id);
            return;
        };
        let distance = hit.map_or_else(
            || {
                // Nothing there: the ground, or a short way off.
                if direction.z < -0.01 {
                    -start.z / direction.z
                } else {
                    512.0
                }
            },
            |(distance, ..)| distance,
        );
        let at = start + direction * distance;
        let place = [snapped(at.x), snapped(at.y), snapped(at.z)];
        let normal = hit.map_or([0.0, 0.0, 1.0], |(.., bounds)| box_normal(at, bounds));
        let (size, depth) = self.press;
        let _result = match tool {
            Tool::Prefab(prefab) => {
                let params: Value =
                    serde_json::from_str(&self.params).unwrap_or_else(|_| json!({}));
                let args = json!({ "prefab": prefab, "at": place, "params": params });
                self.operate("place_prefab", &args)
            }
            Tool::Mesh(source) => {
                self.operate("place_mesh", &json!({ "source": source, "origin": place }))
            }
            Tool::Decal(material) => self.operate(
                "add_decal",
                &json!({ "material": material, "origin": place, "normal": normal, "size": size }),
            ),
            Tool::Stamp(image) => self.operate(
                "add_stamp",
                &json!({
                    "image": image,
                    "origin": place,
                    "normal": normal,
                    "size": size,
                    "depth": depth,
                }),
            ),
            Tool::Paint(layer) => {
                let args = json!({ "points": place, "layer": layer, "radius": size * 0.5 });
                self.extend_line("paint", args, place)
            }
            Tool::Spline => {
                let name = format!("spline_{}", self.things.len());
                let args = json!({ "name": name, "points": place });
                self.extend_line("add_spline", args, place)
            }
            Tool::Material(material) => match hit {
                Some((_, id, _)) => {
                    self.operate("set_material", &json!({ "id": id, "material": material }))
                }
                None => Ok(json!({})),
            },
            Tool::Land(_) => Ok(json!({})),
        };
    }

    /// Adds a point to the line being drawn, or starts one with `operation`.
    fn extend_line(
        &mut self,
        operation: &str,
        start: Value,
        point: [f64; 3],
    ) -> Result<Value, String> {
        if let Some(id) = self.drawing {
            return self.operate("append_point", &json!({ "id": id, "point": point }));
        }
        let answer = self.operate(operation, &start)?;
        self.drawing = answer.get("id").and_then(Value::as_u64);
        Ok(answer)
    }

    /// Whether the tool in hand works by a circle.
    const fn tool_has_a_circle(&self) -> bool {
        matches!(
            self.tool,
            Some(Tool::Land(_) | Tool::Paint(_) | Tool::Decal(_) | Tool::Stamp(_))
        )
    }

    /// The circle of the tool in hand, where the pointer is: lying on land, following its shape, or
    /// flat on the surface of what's under it.
    fn brush_circle(&self) -> Vec<Vec3> {
        const POINTS: u16 = 64;
        let (start, direction) = self.pointer_ray(self.input.pointer);
        let radius = self.brush_radius();
        let round = (0..=POINTS).map(|at| {
            let share = f32::from(at) / f32::from(POINTS);
            (share * std::f32::consts::TAU).sin_cos()
        });
        let hit = self
            .things
            .iter()
            .filter_map(|thing| {
                let bounds = thing.bounds?;
                Some((ray_box(start, direction, bounds)?, bounds))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let land = ray_land(&self.land, start, direction);
        // Whichever's nearer, and a block in front of the land is what's being pointed at.
        let on_land = land.filter(|(_, place)| {
            hit.is_none_or(|(distance, _)| (*place - start).length() < distance)
        });
        if let Some((number, place)) = on_land {
            let sheet = &self.land[number].sheet;
            return round
                .map(|(sine, cosine)| {
                    let (x, y) = (place.x + cosine * radius, place.y + sine * radius);
                    Vec3::new(x, y, sheet.surface_at(x, y).0 + 1.0)
                })
                .collect();
        }
        let Some((distance, bounds)) = hit else {
            return Vec::new();
        };
        let place = start + direction * distance;
        let [x, y, z] = box_normal(place, bounds).map(|part| {
            #[expect(clippy::cast_possible_truncation, reason = "a direction")]
            let part = part as f32;
            part
        });
        let normal = Vec3::new(x, y, z);
        let side = if z.abs() > 0.9 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 0.0, 1.0)
        };
        let across = normal.cross(side);
        let up = normal.cross(across);
        let lifted = place + normal;
        round
            .map(|(sine, cosine)| lifted + across * (cosine * radius) + up * (sine * radius))
            .collect()
    }

    /// Draws the grid, lines and selection over the view.
    fn overlay(&self, painter: &egui::Painter) {
        let size = self.view_size();
        let corner = self.view_rect.min.to_vec2();
        let spot = |point: Vec3| {
            self.camera
                .project(point, size)
                .map(|(x, y)| Pos2::new(x, y) + corner)
        };
        let line = |from: Vec3, to: Vec3, stroke: Stroke| {
            if let (Some(a), Some(b)) = (spot(from), spot(to)) {
                painter.line_segment([a, b], stroke);
            }
        };
        let grid = Stroke::new(1.0, Color32::from_white_alpha(14));
        let centre_x = (self.camera.origin.x / GRID_CELL).round() * GRID_CELL;
        let centre_y = (self.camera.origin.y / GRID_CELL).round() * GRID_CELL;
        #[expect(clippy::cast_precision_loss, reason = "a small count")]
        let reach = GRID_REACH as f32 * GRID_CELL;
        for step in -GRID_REACH..=GRID_REACH {
            #[expect(clippy::cast_precision_loss, reason = "a small count")]
            let offset = step as f32 * GRID_CELL;
            line(
                Vec3::new(centre_x + offset, centre_y - reach, 0.0),
                Vec3::new(centre_x + offset, centre_y + reach, 0.0),
                grid,
            );
            line(
                Vec3::new(centre_x - reach, centre_y + offset, 0.0),
                Vec3::new(centre_x + reach, centre_y + offset, 0.0),
                grid,
            );
        }
        for thing in self.things.iter().filter(|thing| !thing.points.is_empty()) {
            let chosen = Some(thing.id) == self.selected || Some(thing.id) == self.drawing;
            self.draw_line(painter, thing, chosen, &spot);
        }
        let chosen = self
            .things
            .iter()
            .find(|thing| Some(thing.id) == self.selected)
            .and_then(|thing| thing.bounds);
        if let Some((low, high)) = chosen {
            let stroke = Stroke::new(1.5, ACCENT);
            let corner = |x: bool, y: bool, z: bool| {
                Vec3::new(
                    if x { high.x } else { low.x },
                    if y { high.y } else { low.y },
                    if z { high.z } else { low.z },
                )
            };
            for (a, b) in [
                ((false, false), (true, false)),
                ((true, false), (true, true)),
                ((true, true), (false, true)),
                ((false, true), (false, false)),
            ] {
                line(corner(a.0, a.1, false), corner(b.0, b.1, false), stroke);
                line(corner(a.0, a.1, true), corner(b.0, b.1, true), stroke);
                line(corner(a.0, a.1, false), corner(a.0, a.1, true), stroke);
            }
        }
    }

    /// Draws a spline or stroke as a floating line, with bubbles at its points.
    fn draw_line(
        &self,
        painter: &egui::Painter,
        thing: &Thing,
        chosen: bool,
        spot: &dyn Fn(Vec3) -> Option<Pos2>,
    ) {
        let color = if chosen {
            ACCENT
        } else {
            Color32::from_rgb(0x6f, 0xb7, 0xd6)
        };
        let shape = Spline {
            name: String::new(),
            points: thing.points.clone(),
            closed: thing.closed,
        };
        let steps = thing.points.len() * 12;
        let mut last = None;
        for step in 0..=steps {
            #[expect(clippy::cast_precision_loss, reason = "short lines")]
            let share = step as f32 / steps.max(1) as f32;
            let now = if thing.kind == "spline" {
                shape.point_at(share).and_then(spot)
            } else {
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    clippy::cast_precision_loss,
                    reason = "within a short line"
                )]
                let at = (share * (thing.points.len() - 1) as f32) as usize;
                spot(thing.points[at])
            };
            if let (Some(from), Some(to)) = (last, now) {
                painter.line_segment([from, to], Stroke::new(2.0, color));
            }
            last = now;
        }
        for point in &thing.points {
            if let Some(at) = spot(*point) {
                painter.circle_filled(at, 5.0, color);
                painter.circle_stroke(at, 7.0, Stroke::new(1.0, color));
            }
        }
    }

    /// The toolbar.
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("ENGINE EDITOR").color(ACCENT));
            ui.separator();
            let name = saved_map_path(&self.session)
                .map_or_else(|| "unsaved".to_owned(), |path| path.display().to_string());
            ui.label(name);
            ui.separator();
            if ui.button("Easy").clicked() {
                self.easy.on = true;
                self.refresh_plan();
            }
            ui.separator();
            let mut run_op = None;
            if ui.button("Save").clicked() {
                run_op = Some("save");
            }
            if ui.button("Undo").clicked() {
                run_op = Some("undo");
            }
            if ui.button("Redo").clicked() {
                run_op = Some("redo");
            }
            if ui.button("Validate").clicked() {
                run_op = Some("validate");
            }
            if ui.button("Compile").clicked() {
                let _saved = self.operate("save", &json!({}));
                run_op = Some("compile");
            }
            if ui.button("Play").clicked() {
                let _saved = self.operate("save", &json!({}));
                run_op = Some("play");
            }
            if let Some(name) = run_op {
                let _result = self.operate(name, &json!({}));
            }
        });
    }

    /// Reads the floor plan back from the map.
    fn refresh_plan(&mut self) {
        let plan = Plan::of(&self.session);
        let pick = |wanted: Square| -> Vec<(i32, i32)> {
            plan.squares
                .iter()
                .filter(|(_, square)| **square == wanted)
                .map(|(place, _)| *place)
                .collect()
        };
        self.easy.plan = PlanView {
            floor: pick(Square::Floor),
            water: pick(Square::Water),
            things: plan
                .things
                .iter()
                .map(|(place, name)| (*place, word_for(name)))
                .collect(),
            missing: plan
                .missing()
                .iter()
                .map(|line| (*line).to_owned())
                .collect(),
            look: plan.look.clone(),
        };
    }

    /// Does the chosen tool's work on `square`, and says in plain words what
    /// happened, or why it couldn't.
    fn use_brush(&mut self, square: (i32, i32)) {
        self.easy.cursor = square;
        let cells = json!([[square.0, square.1]]);
        let (name, args) = match self.easy.brush {
            Brush::Room => ("plan_paint", json!({ "cells": cells, "kind": "floor" })),
            Brush::Water => ("plan_paint", json!({ "cells": cells, "kind": "water" })),
            Brush::Erase => ("plan_paint", json!({ "cells": cells, "kind": "none" })),
            Brush::Thing(thing) => ("plan_thing", json!({ "cells": cells, "thing": thing })),
        };
        let _result = self.operate(name, &args);
        self.refresh_plan();
    }

    /// Saves, giving a new level a name if it hasn't one.
    fn save_easily(&mut self) {
        if self.session.saved_path().is_none() {
            let maps = self.content.join("maps");
            let free = (1..10_000)
                .map(|number| maps.join(format!("my_level_{number}.emap")))
                .find(|path| !path.exists());
            if let Some(path) = free {
                let _saved = self.operate("save", &json!({ "path": path.display().to_string() }));
            }
            return;
        }
        let _saved = self.operate("save", &json!({}));
    }

    /// Saves, lights the level, and plays it.
    fn play_easily(&mut self) {
        self.save_easily();
        if self.operate("compile", &json!({})).is_ok() {
            let _played = self.operate("play", &json!({}));
        }
    }

    /// The top of the easy view.
    fn easy_top(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if big_button(ui, "Undo", false, 70.0) {
                let _undone = self.operate("undo", &json!({}));
                self.refresh_plan();
            }
            if big_button(ui, "Redo", false, 70.0) {
                let _redone = self.operate("redo", &json!({}));
                self.refresh_plan();
            }
            if big_button(ui, "Save", false, 70.0) {
                self.save_easily();
            }
            let ready = self.easy.plan.missing.is_empty();
            if ui
                .add_enabled_ui(ready, |ui| big_button(ui, "Play", false, 90.0))
                .inner
            {
                self.play_easily();
            }
            ui.separator();
            let seeing = self.easy.seeing;
            if big_button(
                ui,
                if seeing { "Back to plan" } else { "See it" },
                false,
                110.0,
            ) {
                self.easy.seeing = !seeing;
                self.frame_start_position();
            }
            ui.separator();
            if big_button(ui, "Bigger", false, 70.0) {
                self.easy.size = (self.easy.size + 0.25).min(3.0);
            }
            if big_button(ui, "Smaller", false, 70.0) {
                self.easy.size = (self.easy.size - 0.25).max(1.0);
            }
            if big_button(ui, "Plain colours", self.easy.contrast, 120.0) {
                self.easy.contrast = !self.easy.contrast;
                apply_contrast(&self.context, self.easy.contrast);
            }
            ui.separator();
            if big_button(ui, "Advanced", false, 90.0) {
                self.easy.on = false;
                self.context.set_zoom_factor(1.0);
                self.frame_start_position();
            }
        });
    }

    /// The tools, one to a line.
    fn easy_tools(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Choose a tool").size(16.0));
        for (number, (brush, words, ..)) in (1..).zip(&TOOLS) {
            let label = format!("{} {words}", number % 10);
            if big_button(ui, &label, self.easy.brush == *brush, 170.0) {
                self.easy.brush = *brush;
            }
        }
        ui.add_space(10.0);
        ui.label(RichText::new("What it's made of").size(16.0));
        for (words, look) in LOOKS {
            if big_button(ui, words, self.easy.plan.look == look, 170.0) {
                let _changed = self.operate("plan_look", &json!({ "look": look }));
                self.refresh_plan();
            }
        }
    }

    /// What to do next.
    fn easy_next(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("What's next").size(16.0));
        if self.easy.plan.missing.is_empty() {
            ui.label(RichText::new("Your level is ready. Press Play!").size(15.0));
        }
        for line in &self.easy.plan.missing {
            ui.label(RichText::new(format!("- {line}")).size(15.0));
        }
        ui.add_space(10.0);
        let hint = TOOLS.iter().find(|tool| tool.0 == self.easy.brush);
        if let Some((.., hint)) = hint {
            ui.label(RichText::new(*hint).size(15.0));
        }
        ui.add_space(10.0);
        ui.label(RichText::new("With the keyboard").size(15.0));
        for line in [
            "Arrows: move",
            "Space: use the tool",
            "Tab: go through the buttons",
            "Esc: back to the plan",
            "1 to 0: choose a tool",
            "U: undo",
            "P: play",
        ] {
            ui.label(line);
        }
        ui.add_space(10.0);
        let line = self.log.last().cloned().unwrap_or_default();
        if let Some((_, trouble)) = line.split_once(" failed: ") {
            ui.label(RichText::new(trouble).size(15.0).color(ACCENT));
        }
    }

    /// Buttons that look round the level, for what doesn't suit a mouse.
    fn easy_walk(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for (words, ahead, turn) in [
                ("Turn left", 0.0, 20.0),
                ("Forward", 96.0, 0.0),
                ("Back", -96.0, 0.0),
                ("Turn right", 0.0, -20.0),
            ] {
                if big_button(ui, words, false, 90.0) {
                    self.camera.step(ahead, turn);
                }
            }
        });
    }

    /// The easy view.
    fn easy_ui(&mut self, root: &mut egui::Ui) {
        egui::Panel::top("easy_top").show(root, |ui| self.easy_top(ui));
        egui::Panel::left("easy_tools")
            .default_size(190.0)
            .show(root, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.easy_tools(ui));
            });
        egui::Panel::right("easy_next")
            .default_size(230.0)
            .show(root, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.easy_next(ui));
            });
        if self.easy.seeing {
            egui::Panel::bottom("easy_walk").show(root, |ui| self.easy_walk(ui));
        }
        let seeing = self.easy.seeing;
        let central = egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root, |ui| {
                let rect = ui.max_rect();
                self.view_rect = rect;
                let keys = plan_keys(ui, &mut self.easy);
                let clicked = if seeing {
                    let _response = ui.allocate_rect(rect, egui::Sense::hover());
                    None
                } else {
                    draw_plan(ui, rect, &self.easy)
                };
                (keys, clicked)
            });
        self.over_view = seeing;
        let (keys, clicked) = central.inner;
        if let Some(PlanClick::Use(square)) = clicked {
            self.use_brush(square);
        }
        if keys.use_tool && !seeing {
            self.use_brush(self.easy.cursor);
        }
        if keys.undo {
            let _undone = self.operate("undo", &json!({}));
            self.refresh_plan();
        }
        if keys.play && self.easy.plan.missing.is_empty() {
            self.play_easily();
        }
    }

    /// The left panel: what can be added, and what's in the map.
    fn left_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for (tab, name) in [
                (Tab::Build, "Build"),
                (Tab::Surfaces, "Surfaces"),
                (Tab::World, "World"),
                (Tab::Materials, "Materials"),
                (Tab::Script, "Script"),
                (Tab::Graph, "Graph"),
                (Tab::Effects, "Effects"),
            ] {
                if ui.selectable_label(self.tab == tab, name).clicked() {
                    self.tab = tab;
                }
            }
        });
        ui.separator();
        if ui.selectable_label(self.tool.is_none(), "select").clicked() {
            self.set_tool(None);
        }
        if self.drawing.is_some() && ui.button("End line").clicked() {
            self.drawing = None;
        }
        match self.tab {
            Tab::Build => self.build_tab(ui),
            Tab::Surfaces => self.surfaces_tab(ui),
            Tab::World => self.world_tab(ui),
            Tab::Materials => self.materials_tab(ui),
            Tab::Script => self.script_tab(ui),
            Tab::Graph => self.graph_tab(ui),
            Tab::Effects => self.effects_tab(ui),
        }
        ui.separator();
        ui.label(RichText::new("MAP").color(ACCENT));
        egui::ScrollArea::vertical()
            .id_salt("things")
            .show(ui, |ui| {
                for index in 0..self.things.len() {
                    let (id, label) = (self.things[index].id, self.things[index].label.clone());
                    if ui
                        .selectable_label(self.selected == Some(id), label)
                        .clicked()
                    {
                        self.selected = Some(id);
                        self.set_tool(None);
                    }
                }
            });
    }

    /// A list of names, each choosing a tool.
    fn tool_list(
        &mut self,
        ui: &mut egui::Ui,
        id: &str,
        names: &[String],
        make: fn(String) -> Tool,
    ) {
        egui::ScrollArea::vertical()
            .id_salt(id.to_owned())
            .max_height(200.0)
            .show(ui, |ui| {
                for name in names {
                    let tool = make(name.clone());
                    let chosen = self.tool.as_ref() == Some(&tool);
                    if ui.selectable_label(chosen, name.as_str()).clicked() {
                        self.set_tool(Some(tool));
                    }
                }
            });
    }

    /// Prefabs and meshes.
    fn build_tab(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("PREFABS").color(ACCENT));
        let prefabs = self.prefabs.clone();
        self.tool_list(ui, "prefabs", &prefabs, Tool::Prefab);
        ui.label("parameters");
        ui.text_edit_singleline(&mut self.params);
        ui.label(RichText::new("MESHES").color(ACCENT));
        let meshes = self.offered.0.clone();
        self.tool_list(ui, "meshes", &meshes, Tool::Mesh);
    }

    /// Decals, stamps and paint.
    fn surfaces_tab(&mut self, ui: &mut egui::Ui) {
        ui.label("size");
        ui.add(egui::DragValue::new(&mut self.press.0).range(8.0..=1024.0));
        ui.label("stamp depth");
        ui.add(egui::DragValue::new(&mut self.press.1).range(1.0..=64.0));
        ui.label(RichText::new("LAND").color(ACCENT));
        ui.horizontal_wrapped(|ui| {
            for tool in LAND_TOOLS {
                let chosen = self.tool == Some(Tool::Land(tool));
                if ui.selectable_label(chosen, tool.name()).clicked() {
                    self.set_tool(Some(Tool::Land(tool)));
                }
            }
        });
        ui.label("soft edge");
        ui.add(egui::Slider::new(&mut self.land_brush.0, 0.0..=1.0));
        ui.label("strength");
        ui.add(egui::Slider::new(&mut self.land_brush.1, 0.1..=4.0));
        if ui.button("undo land").clicked() {
            self.undo_land();
        }
        if self.land.len() > 1 {
            ui.label("sheet");
            let names: Vec<String> = self.land.iter().map(|land| land.source.clone()).collect();
            for (number, name) in names.into_iter().enumerate() {
                if ui
                    .selectable_label(self.land_chosen == number, name)
                    .clicked()
                {
                    self.land_chosen = number;
                }
            }
        }
        ui.label("layers");
        for (number, material) in self.land_layers().into_iter().enumerate() {
            let chosen = self.land_layer == number;
            if ui.selectable_label(chosen, material).clicked() {
                self.land_layer = number;
            }
        }
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.new_layer);
            if ui.button("add").clicked() {
                let material = std::mem::take(&mut self.new_layer);
                self.add_land_layer(&material);
            }
        });
        ui.label(RichText::new("PAINT LAYER").color(ACCENT));
        ui.horizontal(|ui| {
            for layer in 0..3 {
                let tool = Tool::Paint(layer);
                let chosen = self.tool.as_ref() == Some(&tool);
                if ui.selectable_label(chosen, layer.to_string()).clicked() {
                    self.set_tool(Some(tool));
                }
            }
        });
        let materials = self.offered.1.clone();
        ui.label(RichText::new("DECALS").color(ACCENT));
        self.tool_list(ui, "decals", &materials, Tool::Decal);
        ui.label(RichText::new("STAMPS").color(ACCENT));
        self.tool_list(ui, "stamps", &materials, Tool::Stamp);
    }

    /// Sky, fog, water, lights and splines.
    fn world_tab(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("ADD").color(ACCENT));
        ui.horizontal_wrapped(|ui| {
            for kind in ["sky", "fog", "water"] {
                if ui.button(kind).clicked() {
                    let _added = self.operate("set_block", &json!({ "kind": kind }));
                }
            }
            if ui.button("spline").clicked() {
                self.set_tool(Some(Tool::Spline));
            }
        });
        ui.horizontal_wrapped(|ui| {
            let at = self.camera.origin;
            let place = [snapped(at.x), snapped(at.y), snapped(at.z)];
            for (name, kind) in [
                ("fixed light", "light"),
                ("live light", "live_light"),
                ("flame", "flame"),
            ] {
                if ui.button(name).clicked() {
                    let args = json!({ "type": kind, "origin": place });
                    let _added = self.operate("add_entity", &args);
                }
            }
        });
        ui.label(RichText::new("FIELD").color(ACCENT));
        ui.label("name");
        ui.text_edit_singleline(&mut self.next_field.0);
        ui.label("shader");
        ui.text_edit_singleline(&mut self.next_field.1);
        if ui.button("add field").clicked() {
            let (name, shader) = self.next_field.clone();
            let added = self.operate("add_field", &json!({ "name": name, "shader": shader }));
            if let Err(problem) = added {
                self.note(problem);
            }
        }
    }

    /// Materials: applying them, and editing their text.
    fn materials_tab(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("MATERIALS").color(ACCENT));
        let names = self.offered.1.clone();
        egui::ScrollArea::vertical()
            .id_salt("materials")
            .max_height(180.0)
            .show(ui, |ui| {
                for name in names {
                    let tool = Tool::Material(name.clone());
                    let chosen = self.tool.as_ref() == Some(&tool);
                    if ui.selectable_label(chosen, name.as_str()).clicked() {
                        let read = self.operate("read_material", &json!({ "name": name }));
                        let text = read.ok().and_then(|answer| {
                            answer
                                .get("text")
                                .and_then(Value::as_str)
                                .map(str::to_owned)
                        });
                        self.material = (name, text.unwrap_or_default());
                        self.set_tool(Some(tool));
                    }
                }
            });
        ui.text_edit_multiline(&mut self.material.1);
        if ui.button("Save material").clicked() {
            let (name, text) = self.material.clone();
            let _saved = self.operate("write_material", &json!({ "name": name, "text": text }));
        }
    }

    /// The map's own script.
    fn script_tab(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("MAP SCRIPT").color(ACCENT));
        ui.label("entity types are functions here");
        if ui.button("Load").clicked()
            && let Ok(answer) = self.operate("read_script", &json!({}))
        {
            self.script = answer
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
        }
        egui::ScrollArea::vertical()
            .id_salt("script")
            .max_height(320.0)
            .show(ui, |ui| {
                ui.add(egui::TextEdit::multiline(&mut self.script).code_editor());
            });
        if ui.button("Save script").clicked() {
            let text = self.script.clone();
            let _saved = self.operate("write_script", &json!({ "text": text }));
        }
    }

    /// The right panel: the selected thing's fields.
    fn right_panel(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("PROPERTIES").color(ACCENT));
        let Some(thing) = self
            .things
            .iter()
            .find(|thing| Some(thing.id) == self.selected)
            .cloned()
        else {
            ui.label("nothing selected");
            return;
        };
        ui.label(thing.label.as_str());
        ui.separator();
        let mut change = None;
        egui::Grid::new("fields").num_columns(2).show(ui, |ui| {
            for (key, value) in &thing.fields {
                ui.label(key);
                let mut text = value.clone();
                let response = ui.text_edit_singleline(&mut text);
                if response.lost_focus() && text != *value {
                    change = Some((key.clone(), text));
                }
                ui.end_row();
            }
        });
        if let Some((key, text)) = change {
            self.set_text_field(thing.id, &key, &text);
        }
        ui.separator();
        ui.label("add a field");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.new_field.0);
            ui.text_edit_singleline(&mut self.new_field.1);
        });
        if ui.button("Add").clicked() && !self.new_field.0.is_empty() {
            let (key, text) = std::mem::take(&mut self.new_field);
            self.set_text_field(thing.id, &key, &text);
        }
        ui.separator();
        if ui.button("Delete").clicked() {
            let _result = self.operate("delete", &json!({ "id": thing.id }));
            self.selected = None;
        }
    }

    /// Sets a field from what was typed: numbers stay numbers.
    fn set_text_field(&mut self, id: u64, key: &str, text: &str) {
        let numbers: Option<Vec<f64>> = text
            .split_whitespace()
            .map(|part| part.parse().ok())
            .collect();
        let value = match numbers.as_deref() {
            Some([single]) => json!(single),
            Some(many) if !many.is_empty() => json!(many),
            _ => json!(text),
        };
        let _result = self.operate(
            "set_field",
            &json!({ "id": id, "key": key, "value": value }),
        );
    }

    /// Builds the UI for a frame.
    fn build_ui(&mut self, root: &mut egui::Ui) {
        self.context
            .set_zoom_factor(if self.easy.on { self.easy.size } else { 1.0 });
        if self.easy.on {
            self.easy_ui(root);
            return;
        }
        egui::Panel::top("toolbar").show(root, |ui| self.toolbar(ui));
        egui::Panel::bottom("status").show(root, |ui| {
            let line = self.log.last().cloned().unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label(format!("{} things", self.things.len()));
                ui.separator();
                let at = self.camera.origin;
                ui.label(format!("{:.0} {:.0} {:.0}", at.x, at.y, at.z));
                ui.separator();
                ui.label(line);
            });
        });
        egui::Panel::left("left")
            .default_size(230.0)
            .show(root, |ui| self.left_panel(ui));
        egui::Panel::right("right")
            .default_size(300.0)
            .show(root, |ui| self.right_panel(ui));
        if self.tab == Tab::Graph {
            egui::Panel::bottom("graph")
                .default_size(360.0)
                .show(root, |ui| self.graph_canvas(ui));
        }
        if self.tab == Tab::Effects {
            egui::Panel::bottom("effect")
                .default_size(300.0)
                .show(root, |ui| self.effect_canvas(ui));
        }
        let central = egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root, |ui| {
                let rect = ui.max_rect();
                self.view_rect = rect;
                let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
                self.overlay(ui.painter());
                if self.tool_has_a_circle() && (self.over_view || self.pointer_told) {
                    let circle = self.brush_circle();
                    let (size, corner) = (self.view_size(), self.view_rect.min.to_vec2());
                    let line: Vec<Pos2> = circle
                        .iter()
                        .filter_map(|point| self.camera.project(*point, size))
                        .map(|(x, y)| Pos2::new(x, y) + corner)
                        .collect();
                    ui.painter()
                        .add(egui::Shape::line(line, Stroke::new(1.5, ACCENT)));
                }
                if let Some(tool) = &self.tool {
                    ui.painter().text(
                        rect.left_top() + egui::vec2(10.0, 8.0),
                        Align2::LEFT_TOP,
                        tool.describe(),
                        FontId::monospace(13.0),
                        ACCENT,
                    );
                }
                response
            });
        self.over_view = central.inner.hovered();
        let pixel = self.input.pointer;
        if central.inner.clicked() {
            self.click(pixel);
        }
        if let Some(Tool::Land(tool)) = self.tool {
            if central.inner.drag_started() {
                self.land_stroke_begins(pixel);
            }
            if central.inner.dragged() {
                self.land_step(tool, pixel);
            }
            if central.inner.drag_stopped() {
                self.land_stroke_ends();
            }
        }
    }
}

/// Says where `map` can read what's beside its bake, from where it's saved.
fn attach_beside(map: &mut Map, path: &Path) {
    let name = path
        .file_stem()
        .map(|name| name.to_string_lossy().into_owned());
    let content = path.parent().and_then(Path::parent).map(Path::to_owned);
    if let (Some(name), Some(content)) = (name, content) {
        let files: SideFiles = Arc::new(move |file| std::fs::read(content.join(file)).ok());
        attach_side_files(map, &name, &files);
    }
}

/// The saved path of the session.
fn saved_map_path(session: &Session) -> Option<PathBuf> {
    session.saved_path()
}

/// Writes an answer to standard output.
#[expect(clippy::print_stdout, reason = "standard output is the live channel")]
fn emit(value: &Value) {
    println!("{value}");
}

/// Writes a capture as a PNG.
fn write_png(path: &Path, capture: &engine_render::Capture) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|error| error.to_string())?;
    let mut encoder =
        png::Encoder::new(std::io::BufWriter::new(file), capture.width, capture.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer
        .write_image_data(&capture.rgba)
        .map_err(|error| error.to_string())
}

impl PlatformApp for Editor {
    fn started(&mut self, window: Window) -> Result<(), AppError> {
        let target = Target::Window(window.native()?);
        self.window = Some(window);
        let mut renderer = Renderer::new(target, self.size)?;
        renderer.set_filter(TextureFilter {
            smooth: true,
            trilinear: true,
            anisotropy: 8.0,
        })?;
        self.renderer = Some(renderer);
        Ok(())
    }

    fn service(&mut self, _now: Duration) -> ServiceOutcome {
        if self.quit {
            return ServiceOutcome::Stop;
        }
        if let Some(lines) = self.lines.take() {
            while let Ok(line) = lines.try_recv() {
                self.line(&line);
            }
            self.lines = Some(lines);
        }
        ServiceOutcome::After(SERVICE_STEP)
    }

    fn input(&mut self, event: &InputEvent, _time: Duration) {
        match event {
            InputEvent::Button {
                button: MouseButton::Right,
                pressed,
            } => {
                self.looking = *pressed && (self.over_view || self.looking);
            }
            InputEvent::Motion { x, y } if self.looking => self.camera.look(*x, *y),
            InputEvent::Resized { width, height } => {
                self.size = (*width, *height);
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(*width, *height);
                }
            }
            InputEvent::Key { name, pressed, .. } if self.looking || !*pressed => match *name {
                "W" => self.keys.forward = *pressed,
                "S" => self.keys.back = *pressed,
                "A" => self.keys.left = *pressed,
                "D" => self.keys.right = *pressed,
                "E" => self.keys.up = *pressed,
                "Q" => self.keys.down = *pressed,
                "LEFT SHIFT" => self.keys.fast = *pressed,
                _ => {}
            },
            _ => {}
        }
        if !self.looking {
            self.input.event(event);
        }
    }

    fn frame(&mut self, frame: &Frame) -> FrameOutcome {
        if self.quit {
            return FrameOutcome::Stop;
        }
        if frame.reduced {
            return FrameOutcome::Continue;
        }
        #[expect(clippy::cast_possible_truncation, reason = "a frame's time fits f32")]
        let seconds = frame.delta.as_secs_f64().min(0.1) as f32;
        self.camera.fly(self.keys, seconds);
        let context = self.context.clone();
        let scale = context.pixels_per_point();
        let raw = self.input.take(self.size, frame.now.as_secs_f64(), scale);
        let output = context.run_ui(raw, |ui| self.build_ui(ui));
        let primitives = context.tessellate(output.shapes, output.pixels_per_point);
        if self.stale {
            self.rebuild();
        }
        self.run_preview(seconds);
        let Some(renderer) = &mut self.renderer else {
            return FrameOutcome::Continue;
        };
        self.textures.update(&output.textures_delta, renderer);
        let ui = ui_frame(&context, &primitives, &self.textures, self.size);
        renderer.set_ui(ui);
        #[expect(clippy::cast_possible_truncation, reason = "map time fits f32")]
        let time = frame.now.as_secs_f64() as f32;
        let view = View {
            origin: self.camera.origin,
            yaw: self.camera.yaw,
            pitch: self.camera.pitch,
            roll: 0.0,
            fov: FOV,
            time,
            styles: [1.0; 64],
            gamma: 1.0,
            light_shift: [0.0; 3],
        };
        let mut moving: Vec<LiveLight> = self.map.as_ref().map_or_else(Vec::new, |map| {
            (0_u32..)
                .zip(&map.live_lights)
                .map(|(id, light)| LiveLight {
                    id,
                    origin: light.position(time),
                    radius: light.radius,
                    color: light.color,
                    spot: light.spot,
                    spot_cosine: light.spot_cosine,
                    shadows: light.shadows,
                    bounce: light.bounce,
                })
                .take(MAX_LIVE_LIGHTS)
                .collect()
        });
        let system = &self.effect.system;
        let (draws, lights) = system.draws(&|_| [1.0; 3]);
        let mut particles: Vec<ParticleDraw> = draws.iter().map(ParticleDraw::from).collect();
        let eye = self.camera.origin;
        let away = |particle: &ParticleDraw| (particle.origin - eye).length();
        particles.sort_by(|a, b| away(b).total_cmp(&away(a)));
        let worked_out = system.gpu_emitters();
        let gpu_particles: Vec<GpuParticles> = worked_out.iter().map(GpuParticles::from).collect();
        let computed = system.sims();
        let computed = computed.iter();
        let particle_sims: Vec<ParticleSim> = computed
            .map(|(sim, emitter)| ParticleSim::of(sim, emitter, [1.0; 3]))
            .collect();
        for (id, light) in (1_000_000_u32..).zip(&lights) {
            if moving.len() < MAX_LIVE_LIGHTS {
                moving.push(LiveLight {
                    id,
                    origin: light.origin,
                    radius: light.reach,
                    color: light.color,
                    ..LiveLight::default()
                });
            }
        }
        let (mut palette, mut meshes) = (Vec::new(), Vec::new());
        if let Some(map) = &self.map {
            self.effect_meshes.draw(
                map,
                &system.mesh_draws(),
                (eye, height_scale(FOV), &|_| 1.0),
                (&mut palette, &mut meshes),
            );
        }
        let drawables = Drawables {
            live_lights: &moving,
            moving: &meshes,
            palette: &palette,
            particles: &particles,
            gpu_particles: &gpu_particles,
            particle_sims: &particle_sims,
            ..Drawables::default()
        };
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the view is inside the window"
        )]
        let area = self.view_rect.is_positive().then(|| {
            let rect = self.view_rect * context.pixels_per_point();
            [
                rect.min.x as u32,
                rect.min.y as u32,
                rect.width() as u32,
                rect.height() as u32,
            ]
        });
        renderer.set_view_area(area);
        if let Some(streamer) = &mut self.tile_streamer
            && let Err(error) = streamer.update(view.origin, renderer)
        {
            self.log.push(format!("tiles: {error}"));
        }
        if let Err(error) = renderer.draw(&view, drawables) {
            self.log.push(format!("draw: {error}"));
        }
        if let Some(path) = self.capture.take()
            && let Some(renderer) = &mut self.renderer
        {
            let shot = renderer.capture(&view, drawables, self.size.0, self.size.1);
            match shot
                .map_err(|error| error.to_string())
                .and_then(|shot| write_png(&path, &shot))
            {
                Ok(()) => self.note(format!("wrote {}", path.display())),
                Err(error) => self.note(format!("capture failed: {error}")),
            }
        }
        FrameOutcome::Continue
    }

    fn mouse_capture(&self) -> bool {
        self.looking
    }

    fn notice(&mut self, message: String) {
        self.note(message);
    }

    fn stopping(&mut self) {
        self.renderer = None;
        self.window = None;
    }
}
