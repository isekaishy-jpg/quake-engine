//! Carrying `egui` to and from the engine: input one way, triangles the other.

use std::collections::HashMap;

use egui::epaint::{ImageDelta, Primitive, Vertex};
use egui::{
    ClippedPrimitive, Color32, Context, Event, ImageData, Key, Modifiers, PointerButton, Pos2,
    RawInput, Rect, TextureId, TexturesDelta, Vec2,
};
use engine_core::canvas::Image;
use engine_platform::{InputEvent, MouseButton};
use engine_render::{Renderer, UiBatch, UiFrame};

/// Input gathered between frames.
#[derive(Debug, Default)]
pub(crate) struct UiInput {
    /// Events waiting for `egui`.
    events: Vec<Event>,
    /// The pointer, in pixels.
    pub(crate) pointer: (f32, f32),
    /// Which modifiers are down.
    modifiers: Modifiers,
}

/// A key's `egui` identity.
fn egui_key(name: &str) -> Option<Key> {
    Some(match name {
        "ENTER" => Key::Enter,
        "ESCAPE" => Key::Escape,
        "BACKSPACE" => Key::Backspace,
        "TAB" => Key::Tab,
        "DELETE" => Key::Delete,
        "LEFT" => Key::ArrowLeft,
        "RIGHT" => Key::ArrowRight,
        "UP" => Key::ArrowUp,
        "DOWN" => Key::ArrowDown,
        "HOME" => Key::Home,
        "END" => Key::End,
        "A" => Key::A,
        "C" => Key::C,
        "V" => Key::V,
        "X" => Key::X,
        "Y" => Key::Y,
        "Z" => Key::Z,
        _ => return None,
    })
}

/// The text a key types, on a US layout.
fn key_text(name: &str, shift: bool) -> Option<String> {
    let mut letters = name.chars();
    let (first, rest) = (letters.next()?, letters.next());
    if rest.is_some() {
        return (name == "SPACE").then(|| " ".to_owned());
    }
    if first.is_ascii_alphabetic() {
        let lower = first.to_ascii_lowercase();
        return Some(if shift { first } else { lower }.to_string());
    }
    let shifted = match first {
        '1' => '!',
        '2' => '@',
        '3' => '#',
        '4' => '$',
        '5' => '%',
        '6' => '^',
        '7' => '&',
        '8' => '*',
        '9' => '(',
        '0' => ')',
        '-' => '_',
        '=' => '+',
        '[' => '{',
        ']' => '}',
        ';' => ':',
        '\'' => '"',
        ',' => '<',
        '.' => '>',
        '/' => '?',
        '\\' => '|',
        '`' => '~',
        other => other,
    };
    Some(if shift { shifted } else { first }.to_string())
}

impl UiInput {
    /// Takes in an engine input event.
    pub(crate) fn event(&mut self, event: &InputEvent) {
        match event {
            InputEvent::Pointer { x, y } => {
                #[expect(clippy::cast_possible_truncation, reason = "pixels fit f32")]
                let at = (*x as f32, *y as f32);
                self.pointer = at;
                self.events.push(Event::PointerMoved(Pos2::new(at.0, at.1)));
            }
            InputEvent::Button { button, pressed } => {
                let button = match button {
                    MouseButton::Left => PointerButton::Primary,
                    MouseButton::Right => PointerButton::Secondary,
                    MouseButton::Middle => PointerButton::Middle,
                };
                self.events.push(Event::PointerButton {
                    pos: Pos2::new(self.pointer.0, self.pointer.1),
                    button,
                    pressed: *pressed,
                    modifiers: self.modifiers,
                });
            }
            InputEvent::Wheel { lines } => {
                #[expect(clippy::cast_possible_truncation, reason = "a wheel fits f32")]
                let lines = *lines as f32;
                self.events.push(Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: Vec2::new(0.0, lines),
                    phase: egui::TouchPhase::Move,
                    modifiers: self.modifiers,
                    source: egui::MouseWheelSource::Wheel,
                });
            }
            InputEvent::Key {
                name,
                pressed,
                repeat,
            } => {
                match *name {
                    "LEFT SHIFT" | "RIGHT SHIFT" => self.modifiers.shift = *pressed,
                    "LEFT CTRL" | "RIGHT CTRL" => {
                        self.modifiers.ctrl = *pressed;
                        self.modifiers.command = *pressed;
                    }
                    "LEFT ALT" | "RIGHT ALT" => self.modifiers.alt = *pressed,
                    _ => {}
                }
                if let Some(key) = egui_key(name) {
                    self.events.push(Event::Key {
                        key,
                        physical_key: None,
                        pressed: *pressed,
                        repeat: *repeat,
                        modifiers: self.modifiers,
                    });
                }
                if *pressed
                    && !self.modifiers.ctrl
                    && let Some(text) = key_text(name, self.modifiers.shift)
                {
                    self.events.push(Event::Text(text));
                }
            }
            _ => {}
        }
    }

    /// What `egui` gets this frame.
    pub(crate) fn take(&mut self, size: (u32, u32), time: f64) -> RawInput {
        #[expect(clippy::cast_precision_loss, reason = "window sizes are small")]
        let screen = Vec2::new(size.0 as f32, size.1 as f32);
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen)),
            time: Some(time),
            events: std::mem::take(&mut self.events),
            ..RawInput::default()
        }
    }
}

/// Textures `egui` has given us, as pictures on the renderer.
#[derive(Debug, Default)]
pub(crate) struct UiTextures {
    /// CPU copies, so partial updates can be applied.
    images: HashMap<TextureId, (usize, usize, Vec<u8>)>,
    /// Picture numbers.
    pictures: HashMap<TextureId, u32>,
}

impl UiTextures {
    /// Applies what changed.
    pub(crate) fn update(&mut self, delta: &TexturesDelta, renderer: &mut Renderer) {
        for (id, changes) in &delta.set {
            for change in changes {
                self.apply(*id, change);
            }
            if let Some((width, height, full)) = self.images.get(id) {
                let picture = Image {
                    width: u32::try_from(*width).unwrap_or(0),
                    height: u32::try_from(*height).unwrap_or(0),
                    rgba: full.clone(),
                };
                if let Ok(number) = renderer.add_picture(&picture) {
                    self.pictures.insert(*id, number);
                }
            }
        }
    }

    /// Applies one change to the CPU copy.
    fn apply(&mut self, id: TextureId, change: &ImageDelta) {
        let ImageData::Color(image) = &change.image;
        let (width, height) = (image.size[0], image.size[1]);
        let pixels: Vec<u8> = image
            .pixels
            .iter()
            .flat_map(|color| color.to_srgba_unmultiplied())
            .collect();
        let Some(at) = change.pos else {
            self.images.insert(id, (width, height, pixels));
            return;
        };
        let Some((full_width, _, full)) = self.images.get_mut(&id) else {
            return;
        };
        for y in 0..height {
            let target = ((at[1] + y) * *full_width + at[0]) * 4;
            let source = y * width * 4;
            if let (Some(to), Some(from)) = (
                full.get_mut(target..target + width * 4),
                pixels.get(source..source + width * 4),
            ) {
                to.copy_from_slice(from);
            }
        }
    }

    /// The renderer's picture for `id`.
    fn picture(&self, id: TextureId) -> Option<u32> {
        self.pictures.get(&id).copied()
    }
}

/// A vertex's colour, unmultiplied, as the canvas blends.
fn color_parts(color: Color32) -> [f32; 4] {
    let [red, green, blue, alpha] = color.to_srgba_unmultiplied();
    [red, green, blue, alpha].map(|part| f32::from(part) / 255.0)
}

/// Clip space from pixels, with y up as the world shader has it.
fn clip_vertex(vertex: &Vertex, size: (f32, f32)) -> [f32; 8] {
    let [red, green, blue, alpha] = color_parts(vertex.color);
    [
        vertex.pos.x / size.0 * 2.0 - 1.0,
        1.0 - vertex.pos.y / size.1 * 2.0,
        vertex.uv.x,
        vertex.uv.y,
        red,
        green,
        blue,
        alpha,
    ]
}

/// Turns what `egui` drew into triangles for the renderer.
pub(crate) fn ui_frame(
    context: &Context,
    primitives: &[ClippedPrimitive],
    textures: &UiTextures,
    size: (u32, u32),
) -> UiFrame {
    let _pixels = context;
    let mut frame = UiFrame::default();
    #[expect(clippy::cast_precision_loss, reason = "window sizes are small")]
    let float_size = (size.0 as f32, size.1 as f32);
    for ClippedPrimitive {
        clip_rect,
        primitive,
    } in primitives
    {
        let Primitive::Mesh(mesh) = primitive else {
            continue;
        };
        let Some(picture) = textures.picture(mesh.texture_id) else {
            continue;
        };
        let first = u32::try_from(frame.vertices.len() / 8).unwrap_or(0);
        for index in &mesh.indices {
            if let Some(vertex) = mesh.vertices.get(*index as usize) {
                frame.vertices.extend(clip_vertex(vertex, float_size));
            }
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped to the window"
        )]
        let clip = [
            clip_rect.min.x.clamp(0.0, float_size.0) as u32,
            clip_rect.min.y.clamp(0.0, float_size.1) as u32,
            clip_rect.max.x.clamp(0.0, float_size.0) as u32,
            clip_rect.max.y.clamp(0.0, float_size.1) as u32,
        ];
        frame.batches.push(UiBatch {
            picture,
            first,
            count: u32::try_from(mesh.indices.len()).unwrap_or(0),
            clip,
        });
    }
    frame
}
