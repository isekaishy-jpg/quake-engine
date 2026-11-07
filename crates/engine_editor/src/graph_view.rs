//! A canvas of boxes, ports and curves, which knows nothing of what its nodes mean, so that shaders
//! and particles can both use it (proposal 0145).

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2, pos2, vec2};

/// How wide a box is.
const WIDTH: f32 = 150.0;
/// How tall its title is.
const TITLE: f32 = 24.0;
/// How tall a row of it is.
const ROW: f32 = 20.0;
/// How far from a port a pointer still counts as on it.
const REACH: f32 = 9.0;

/// A box.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewNode {
    /// What's on its title.
    pub title: String,
    /// Where it is on the canvas.
    pub at: [f32; 2],
    /// What goes in, one to a row.
    pub inputs: Vec<String>,
    /// Whether anything comes out of it.
    pub output: bool,
}

/// A curve from a box's output to another's input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewWire {
    /// The box it leaves.
    pub from: usize,
    /// The box it goes to.
    pub to: usize,
    /// Which input.
    pub input: usize,
}

/// What a person did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ViewEvent {
    /// Picked a box.
    Chosen(usize),
    /// Moved one to here.
    Moved(usize, [f32; 2]),
    /// Drew a curve.
    Joined(ViewWire),
    /// Took one off this box's input.
    Cut(usize, usize),
}

/// Where a box's input is on the canvas.
#[must_use]
pub fn input_port(node: &ViewNode, input: usize) -> [f32; 2] {
    #[expect(clippy::cast_precision_loss, reason = "a box has few inputs")]
    let row = input as f32;
    [node.at[0], node.at[1] + TITLE + ROW * (row + 0.5)]
}

/// Where its output is.
#[must_use]
pub fn output_port(node: &ViewNode) -> [f32; 2] {
    [node.at[0] + WIDTH, node.at[1] + TITLE * 0.5]
}

/// The input at `place`, as a box and which of its inputs.
#[must_use]
pub fn input_at(nodes: &[ViewNode], place: [f32; 2]) -> Option<(usize, usize)> {
    let near = |port: [f32; 2]| (port[0] - place[0]).hypot(port[1] - place[1]) < REACH;
    nodes.iter().enumerate().find_map(|(number, node)| {
        (0..node.inputs.len())
            .find(|input| near(input_port(node, *input)))
            .map(|input| (number, input))
    })
}

/// The output at `place`.
#[must_use]
pub fn output_at(nodes: &[ViewNode], place: [f32; 2]) -> Option<usize> {
    let near = |port: [f32; 2]| (port[0] - place[0]).hypot(port[1] - place[1]) < REACH;
    nodes
        .iter()
        .position(|node| node.output && near(output_port(node)))
}

/// How tall a box is.
fn height_of(node: &ViewNode) -> f32 {
    #[expect(clippy::cast_precision_loss, reason = "a box has few inputs")]
    let rows = node.inputs.len() as f32;
    TITLE + ROW * rows + 4.0
}

/// The box at `place`, the top one first.
#[must_use]
pub fn node_at(nodes: &[ViewNode], place: [f32; 2]) -> Option<usize> {
    nodes.iter().rposition(|node| {
        let (x, y) = (place[0] - node.at[0], place[1] - node.at[1]);
        (0.0..WIDTH).contains(&x) && (0.0..height_of(node)).contains(&y)
    })
}

/// What's being dragged.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    /// A box, and where in it it was picked up.
    Box(usize, [f32; 2]),
    /// A new curve, from this box.
    Wire(usize),
    /// The canvas.
    Canvas,
}

/// A view of boxes, which keeps where it's looking.
#[derive(Clone, Debug, Default)]
pub struct GraphView {
    /// How far the canvas has moved.
    pan: Vec2,
    /// What's being dragged.
    drag: Option<Drag>,
}

/// A place on the canvas, on the screen.
fn to_screen(rect: Rect, pan: Vec2, place: [f32; 2]) -> Pos2 {
    rect.min + pan + vec2(place[0], place[1])
}

/// A curve between two places.
fn curve(from: Pos2, to: Pos2) -> Vec<Pos2> {
    let bend = ((to.x - from.x).abs() * 0.5).max(30.0);
    let (a, b) = (from + vec2(bend, 0.0), to - vec2(bend, 0.0));
    (0_u8..=20)
        .map(|step| {
            let t = f32::from(step) / 20.0;
            let u = 1.0 - t;
            let weigh = |p: Pos2, w: f32| p.to_vec2() * w;
            let sum = weigh(from, u * u * u)
                + weigh(a, 3.0 * u * u * t)
                + weigh(b, 3.0 * u * t * t)
                + weigh(to, t * t * t);
            sum.to_pos2()
        })
        .collect()
}

impl GraphView {
    /// Where on the canvas the middle of what's showing is, for what's added next.
    #[must_use]
    pub fn middle(&self, rect: Rect) -> [f32; 2] {
        let place = rect.center() - rect.min - self.pan;
        [place.x, place.y]
    }

    /// Draws `nodes` and `wires` in `rect`, with `chosen` picked out, and says what was done.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        (nodes, wires): (&[ViewNode], &[ViewWire]),
        chosen: Option<usize>,
        accent: Color32,
    ) -> Vec<ViewEvent> {
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
        let pan = self.pan;
        let place_of = |pointer: Pos2| {
            let place = pointer - rect.min - pan;
            [place.x, place.y]
        };
        let mut events = Vec::new();
        let pointer = response.interact_pointer_pos();
        // A drag's only known once the pointer has moved, and what it took hold of is where it began.
        let began = ui.input(|input| input.pointer.press_origin());
        if response.drag_started()
            && let Some(pointer) = began.or(pointer)
        {
            let place = place_of(pointer);
            self.drag = Some(if let Some(from) = output_at(nodes, place) {
                Drag::Wire(from)
            } else if let Some(number) = node_at(nodes, place) {
                let at = nodes[number].at;
                events.push(ViewEvent::Chosen(number));
                Drag::Box(number, [place[0] - at[0], place[1] - at[1]])
            } else {
                Drag::Canvas
            });
        }
        if response.dragged() {
            match (self.drag, pointer) {
                (Some(Drag::Box(number, grip)), Some(pointer)) => {
                    let place = place_of(pointer);
                    let at = [place[0] - grip[0], place[1] - grip[1]];
                    events.push(ViewEvent::Moved(number, at));
                }
                (Some(Drag::Canvas), _) => self.pan += response.drag_delta(),
                _ => {}
            }
        }
        if response.drag_stopped() {
            if let (Some(Drag::Wire(from)), Some(pointer)) =
                (self.drag, ui.ctx().pointer_latest_pos())
                && let Some((to, input)) = input_at(nodes, place_of(pointer))
                && to != from
            {
                events.push(ViewEvent::Joined(ViewWire { from, to, input }));
            }
            self.drag = None;
        }
        if response.clicked()
            && let Some(pointer) = pointer
        {
            let place = place_of(pointer);
            if let Some((to, input)) = input_at(nodes, place) {
                events.push(ViewEvent::Cut(to, input));
            } else if let Some(number) = node_at(nodes, place) {
                events.push(ViewEvent::Chosen(number));
            }
        }
        let wire_stroke = Stroke::new(1.5, ui.visuals().text_color());
        for wire in wires {
            let (Some(from), Some(to)) = (nodes.get(wire.from), nodes.get(wire.to)) else {
                continue;
            };
            let line = curve(
                to_screen(rect, pan, output_port(from)),
                to_screen(rect, pan, input_port(to, wire.input)),
            );
            painter.add(Shape::line(line, wire_stroke));
        }
        if let (Some(Drag::Wire(from)), Some(pointer)) = (self.drag, ui.ctx().pointer_latest_pos())
            && let Some(node) = nodes.get(from)
        {
            let line = curve(to_screen(rect, pan, output_port(node)), pointer);
            painter.add(Shape::line(line, Stroke::new(1.5, accent)));
        }
        for (number, node) in nodes.iter().enumerate() {
            let top = to_screen(rect, pan, node.at);
            let body = Rect::from_min_size(top, vec2(WIDTH, height_of(node)));
            painter.rect_filled(body, 3.0, ui.visuals().panel_fill);
            let edge = if chosen == Some(number) {
                accent
            } else {
                ui.visuals().weak_text_color()
            };
            painter.rect_stroke(body, 3.0, Stroke::new(1.0, edge), egui::StrokeKind::Inside);
            painter.text(
                top + vec2(8.0, TITLE * 0.5),
                Align2::LEFT_CENTER,
                &node.title,
                FontId::proportional(13.0),
                ui.visuals().strong_text_color(),
            );
            for (input, name) in node.inputs.iter().enumerate() {
                let port = to_screen(rect, pan, input_port(node, input));
                painter.circle_filled(port, 4.0, accent);
                painter.text(
                    port + vec2(9.0, 0.0),
                    Align2::LEFT_CENTER,
                    name,
                    FontId::proportional(12.0),
                    ui.visuals().text_color(),
                );
            }
            if node.output {
                painter.circle_filled(to_screen(rect, pan, output_port(node)), 4.0, accent);
            }
        }
        events
    }
}

/// A place on the canvas as a point, for what's outside.
#[must_use]
pub fn point(place: [f32; 2]) -> Pos2 {
    pos2(place[0], place[1])
}
