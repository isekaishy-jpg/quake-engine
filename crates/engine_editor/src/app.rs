//! The editor application.

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use egui::{Align2, Color32, Context, FontId, Pos2, RichText, Stroke};
use engine_assets::{DirectoryArchive, FileSystem};
use engine_compile::{MaterialLibrary, apply_bake, compile, read_map, source_hash};
use engine_core::math::Vec3;
use engine_format::from_text;
use engine_map::{Map, Texture};
use engine_platform::{
    AppError, Frame, FrameOutcome, InputEvent, MouseButton, PlatformApp, ServiceOutcome, Window,
};
use engine_render::{Drawables, Renderer, Target, TextureFilter, View};
use engine_tools::{Session, prefab_names, run};
use serde_json::{Value, json};

use crate::camera::{Camera, FOV, Keys};
use crate::panels::{ACCENT, apply_style};
use crate::ui_bridge::{UiInput, UiTextures, ui_frame};

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
    /// The prefab that a click places.
    tool: Option<String>,
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
    Some(Thing {
        id,
        label: format!("{id}  {label}"),
        bounds,
        fields,
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
            prefabs: prefab_names(content),
            params: "{}".to_owned(),
            log: Vec::new(),
            new_field: (String::new(), String::new()),
            lines: Some(read_lines()),
            capture: None,
            quit: false,
            over_view: false,
        };
        editor.refresh_things();
        editor.frame_start_position();
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
        self.stale = true;
        result
    }

    /// Rereads what's in the map.
    fn refresh_things(&mut self) {
        let Ok(described) = run(&mut self.session, "describe", &json!({})) else {
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
        if let Err(error) = library.place_meshes(&mut working) {
            self.log.push(format!("compile: {error}"));
            return;
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
        if let Some(path) = saved_map_path(&self.session)
            && let Ok(bytes) = std::fs::read(path.with_extension("ebake"))
            && let Ok(saved) = std::fs::read_to_string(&path)
        {
            let _applied = apply_bake(&mut map, &bytes, source_hash(&saved));
        }
        let map = Arc::new(map);
        if let Err(error) = renderer.load_map(Arc::clone(&map)) {
            self.log.push(format!("render: {error}"));
        }
        self.map = Some(map);
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
                self.tool = args
                    .get("prefab")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                if let Some(params) = args.get("params") {
                    self.params = params.to_string();
                }
                emit(&json!({ "tool": self.tool }));
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

    /// A click in the 3D view: places the tool's prefab, or selects.
    fn click(&mut self, pixel: (f32, f32)) {
        #[expect(clippy::cast_precision_loss, reason = "window sizes are small")]
        let size = (self.size.0 as f32, self.size.1 as f32);
        let direction = self.camera.ray(pixel, size);
        let start = self.camera.origin;
        let hit = self
            .things
            .iter()
            .filter_map(|thing| {
                let distance = ray_box(start, direction, thing.bounds?)?;
                Some((distance, thing.id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some(prefab) = self.tool.clone() {
            let distance = hit.map_or(512.0, |(distance, _)| distance);
            let at = start + direction * distance;
            let params: Value = serde_json::from_str(&self.params).unwrap_or_else(|_| json!({}));
            let args = json!({
                "prefab": prefab,
                "at": [snapped(at.x), snapped(at.y), snapped(at.z)],
                "params": params,
            });
            let _result = self.operate("place_prefab", &args);
        } else {
            self.selected = hit.map(|(_, id)| id);
        }
    }

    /// Draws the grid and selection over the view.
    fn overlay(&self, painter: &egui::Painter) {
        #[expect(clippy::cast_precision_loss, reason = "window sizes are small")]
        let size = (self.size.0 as f32, self.size.1 as f32);
        let line = |from: Vec3, to: Vec3, stroke: Stroke| {
            if let (Some(a), Some(b)) = (
                self.camera.project(from, size),
                self.camera.project(to, size),
            ) {
                painter.line_segment([Pos2::new(a.0, a.1), Pos2::new(b.0, b.1)], stroke);
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

    /// The toolbar.
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("ENGINE EDITOR").color(ACCENT));
            ui.separator();
            let name = saved_map_path(&self.session)
                .map_or_else(|| "unsaved".to_owned(), |path| path.display().to_string());
            ui.label(name);
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
                run_op = Some("playtest");
            }
            if let Some(name) = run_op {
                let _result = self.operate(name, &json!({}));
            }
        });
    }

    /// The left panel: prefabs and what's in the map.
    fn left_panel(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("PREFABS").color(ACCENT));
        egui::ScrollArea::vertical()
            .id_salt("prefabs")
            .max_height(280.0)
            .show(ui, |ui| {
                let select = ui.selectable_label(self.tool.is_none(), "select");
                if select.clicked() {
                    self.tool = None;
                }
                for name in self.prefabs.clone() {
                    let chosen = self.tool.as_deref() == Some(name.as_str());
                    if ui.selectable_label(chosen, name.as_str()).clicked() {
                        self.tool = Some(name);
                    }
                }
            });
        ui.label("parameters");
        ui.text_edit_singleline(&mut self.params);
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
                        self.tool = None;
                    }
                }
            });
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
        let central = egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root, |ui| {
                let rect = ui.max_rect();
                let response = ui.allocate_rect(rect, egui::Sense::click());
                self.overlay(ui.painter());
                if let Some(tool) = &self.tool {
                    ui.painter().text(
                        rect.left_top() + egui::vec2(10.0, 8.0),
                        Align2::LEFT_TOP,
                        format!("placing {tool}: click to place"),
                        FontId::monospace(13.0),
                        ACCENT,
                    );
                }
                response
            });
        self.over_view = central.inner.hovered();
        if central.inner.clicked() {
            let pixel = self.input.pointer;
            self.click(pixel);
        }
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
        let raw = self.input.take(self.size, frame.now.as_secs_f64());
        let context = self.context.clone();
        let output = context.run_ui(raw, |ui| self.build_ui(ui));
        let primitives = context.tessellate(output.shapes, output.pixels_per_point);
        if self.stale {
            self.rebuild();
        }
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
        };
        if let Err(error) = renderer.draw(&view, Drawables::default()) {
            self.log.push(format!("draw: {error}"));
        }
        if let Some(path) = self.capture.take()
            && let Some(renderer) = &mut self.renderer
        {
            let shot = renderer.capture(&view, Drawables::default(), self.size.0, self.size.1);
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
