//! Changing an effect, and what the node view shows of it (proposal 0146). What's changed is the
//! file as it reads, so that nothing in it is lost.

use engine_format::{Block, Document, Value, from_text, to_text};

use crate::graph_view::{ViewEvent, ViewNode, ViewWire};

/// What an emitter can say, in the order the panel shows it, with what each takes.
pub const EMITTER_SETTINGS: &[(&str, &str)] = &[
    ("kind", "sprite, ribbon, beam, light or decal"),
    ("blend", "over or add"),
    ("rate", "how many a second"),
    ("burst", "how many at once"),
    ("delay", "seconds before it starts"),
    ("duration", "seconds it runs, or 0 for always"),
    ("shape", "point, ball, box, ring, disc, cone or line"),
    ("radius", "for a ball, ring, disc or cone"),
    ("height", "for a cone"),
    ("half_size", "x y z, for a box or line"),
    ("outward", "true to head away from the middle"),
    ("life", "seconds, or a low and a high"),
    ("speed", "a number, or a low and a high"),
    ("direction", "x y z"),
    ("spread", "degrees"),
    ("gravity", "a share of the world's"),
    ("drag", "how much it slows"),
    ("turbulence", "how hard it's pushed about"),
    ("size", "a number, or when and how big"),
    ("color", "r g b a, or a when before each four"),
    ("reach", "of a light or beam"),
    ("gpu", "true, or sim for compute"),
    ("motion", "a shader_motion, with gpu = sim"),
    ("field", "a field it reads, with gpu = sim"),
    ("image", "a picture"),
    ("flipbook", "its columns and rows"),
    ("flip_rate", "frames a second"),
    ("shader", "a shader_particle"),
    ("rotation", "degrees, a low and a high"),
    ("spin", "degrees a second, a low and a high"),
    ("stretch", "how far it's drawn out"),
    ("soft", "how far it fades into what's behind"),
    ("lit", "true to take the level's light"),
    ("collide", "none, bounce, stick or die"),
    ("bounce", "how much speed it keeps"),
    ("far", "where it thins out and stops"),
];

/// What a force can say.
pub const FORCE_SETTINGS: &[(&str, &str)] = &[
    ("kind", "attract, vortex or wind"),
    ("at", "x y z, from the effect's middle"),
    ("strength", "how hard"),
    ("reach", "how far an attract goes"),
    ("axis", "x y z, of a vortex"),
    ("inward", "how hard a vortex draws in"),
    ("blowing", "x y z, of a wind"),
    ("gusts", "how much a wind comes and goes"),
];

/// The box that stands for the timeline, which is always first.
pub const TIMELINE: usize = 0;

/// The rows of an emitter's box.
const INPUTS: [&str; 3] = ["when one ends", "when one hits", "on the timeline"];

/// What's said of a block's starting one, by row.
const STARTS: [&str; 2] = ["on_death", "on_hit"];

/// An effect with one emitter in it.
#[must_use]
pub fn blank() -> Document {
    let mut document = Document::new("effect");
    let _name = add_emitter(&mut document, [240.0, 40.0]);
    document
}

/// Each emitter's place among the document's blocks.
#[must_use]
pub fn emitter_blocks(document: &Document) -> Vec<usize> {
    let blocks = document.blocks.iter().enumerate();
    blocks
        .filter(|(_, block)| block.kind == "emitter")
        .map(|(at, _)| at)
        .collect()
}

/// The emitter of box `number` of the view.
#[must_use]
pub fn emitter_of(document: &Document, number: usize) -> Option<&Block> {
    let at = *emitter_blocks(document).get(number.checked_sub(1)?)?;
    document.blocks.get(at)
}

/// The same, to change.
pub fn emitter_mut(document: &mut Document, number: usize) -> Option<&mut Block> {
    let at = *emitter_blocks(document).get(number.checked_sub(1)?)?;
    document.blocks.get_mut(at)
}

/// What `block` is called.
fn name_of(block: &Block) -> &str {
    block.name.as_deref().unwrap_or("")
}

/// Adds an emitter at `at`, and says what it's called.
pub fn add_emitter(document: &mut Document, at: [f32; 2]) -> String {
    let taken = |name: &str| document.blocks_of("emitter").any(|b| name_of(b) == name);
    let name = (1..)
        .map(|number| format!("emitter{number}"))
        .find(|name| !taken(name))
        .unwrap_or_default();
    let mut block = Block::named("emitter", &name);
    set_field(&mut block, "at", place(at));
    set_field(&mut block, "rate", Value::Number(40.0));
    set_field(&mut block, "life", Value::Number(1.0));
    set_field(&mut block, "speed", Value::Number(60.0));
    document.blocks.push(block);
    name
}

/// A place on the canvas, as a value.
fn place(at: [f32; 2]) -> Value {
    Value::List(vec![
        Value::Number(f64::from(at[0])),
        Value::Number(f64::from(at[1])),
    ])
}

/// Says `value` for `key`, in place of what was said.
fn set_field(block: &mut Block, key: &str, value: Value) {
    match block.fields.iter_mut().find(|(name, _)| name == key) {
        Some(field) => field.1 = value,
        None => block.set(key, value),
    }
}

/// Takes `key` away.
fn clear_field(block: &mut Block, key: &str) {
    block.fields.retain(|(name, _)| name != key);
}

/// Takes the emitter of box `number` away, and whatever starts it.
pub fn remove_emitter(document: &mut Document, number: usize) {
    let Some(name) = emitter_of(document, number).map(|block| name_of(block).to_owned()) else {
        return;
    };
    document
        .blocks
        .retain(|block| !(block.kind == "emitter" && name_of(block) == name));
    forget_starts(document, &name, 0);
    forget_starts(document, &name, 1);
    forget_starts(document, &name, 2);
}

/// Takes away what starts `name` by row `input`.
fn forget_starts(document: &mut Document, name: &str, input: usize) {
    if input == 2 {
        document.blocks.retain(|block| {
            block.kind != "timeline" || block.get("start").and_then(Value::text) != Some(name)
        });
        return;
    }
    for block in &mut document.blocks {
        if block.get(STARTS[input]).and_then(Value::text) == Some(name) {
            clear_field(block, STARTS[input]);
        }
    }
}

/// What `key` says in `block`, as it would be written, or nothing.
#[must_use]
pub fn setting(block: &Block, key: &str) -> String {
    block.get(key).map_or_else(String::new, value_text)
}

/// `value`, as a person would type it.
fn value_text(value: &Value) -> String {
    let mut block = Block::new("v");
    block.set("k", value.clone());
    let mut document = Document::new("x");
    document.blocks.push(block);
    let text = to_text(&document);
    let start = text.find("k = ").map_or(0, |at| at + 4);
    let line = text[start..].lines().next().unwrap_or("");
    line.trim().to_owned()
}

/// What's typed, as a value, or what's wrong with it.
fn typed_value(text: &str) -> Result<Value, String> {
    let wrapped = format!("format 1 x\nv {{ k = {text} }}\n");
    let document = from_text(&wrapped).map_err(|error| error.to_string())?;
    let block = document.blocks.first();
    block
        .and_then(|block| block.get("k").cloned())
        .ok_or_else(|| format!("{text} isn't a value"))
}

/// Says what's typed for `key` in `block`. With nothing typed, it says nothing of it.
///
/// # Errors
///
/// Says what's wrong with what's typed, and changes nothing.
pub fn set_setting(block: &mut Block, key: &str, text: &str) -> Result<(), String> {
    if text.trim().is_empty() {
        clear_field(block, key);
        return Ok(());
    }
    set_field(block, key, typed_value(text)?);
    Ok(())
}

/// Calls the emitter of box `number` `name`, and whatever starts it follows.
///
/// # Errors
///
/// Says why not, if there's another of that name.
pub fn rename(document: &mut Document, number: usize, name: &str) -> Result<(), String> {
    let Some(was) = emitter_of(document, number).map(|block| name_of(block).to_owned()) else {
        return Ok(());
    };
    if was == name {
        return Ok(());
    }
    if name.trim().is_empty() || name.contains(char::is_whitespace) {
        return Err("a name is one word".to_owned());
    }
    if document.blocks_of("emitter").any(|b| name_of(b) == name) {
        return Err(format!("there's already a {name}"));
    }
    let new = Value::Text(name.to_owned());
    for block in &mut document.blocks {
        for key in ["on_death", "on_hit", "start"] {
            if block.get(key).and_then(Value::text) == Some(&was) {
                set_field(block, key, new.clone());
            }
        }
        if block.kind == "emitter" && name_of(block) == was {
            block.name = Some(name.to_owned());
        }
    }
    Ok(())
}

/// Where box `number` goes, when nothing says.
fn usual_place(number: usize) -> [f32; 2] {
    #[expect(clippy::cast_precision_loss, reason = "an effect has few emitters")]
    let step = number as f32;
    [40.0 + step * 200.0, 40.0 + (step % 2.0) * 60.0]
}

/// Where `block` says it goes.
fn place_of(block: &Block, number: usize) -> [f32; 2] {
    let numbers = block.get("at").and_then(Value::numbers);
    #[expect(clippy::cast_possible_truncation, reason = "a place on a canvas")]
    numbers.map_or(usual_place(number), |at| match at[..] {
        [x, y, ..] => [x as f32, y as f32],
        _ => usual_place(number),
    })
}

/// The boxes and curves of `document`: first the timeline, and then each emitter.
#[must_use]
pub fn view_of(document: &Document) -> (Vec<ViewNode>, Vec<ViewWire>) {
    let emitters: Vec<&Block> = document.blocks_of("emitter").collect();
    let held = document.blocks_of("view").next();
    let mut nodes = vec![ViewNode {
        title: "timeline".to_owned(),
        at: held.map_or(usual_place(0), |block| place_of(block, 0)),
        inputs: Vec::new(),
        output: true,
    }];
    for (number, block) in emitters.iter().enumerate() {
        nodes.push(ViewNode {
            title: name_of(block).to_owned(),
            at: place_of(block, number + 1),
            inputs: INPUTS.iter().map(|&input| input.to_owned()).collect(),
            output: true,
        });
    }
    let box_of = |name: &str| {
        let at = emitters.iter().position(|block| name_of(block) == name);
        at.map(|at| at + 1)
    };
    let mut wires = Vec::new();
    for (number, block) in emitters.iter().enumerate() {
        for (input, key) in STARTS.iter().enumerate() {
            let starts = block.get(key).and_then(Value::text).and_then(box_of);
            if let Some(to) = starts {
                wires.push(ViewWire {
                    from: number + 1,
                    to,
                    input,
                });
            }
        }
    }
    for block in document.blocks_of("timeline") {
        if let Some(to) = block.get("start").and_then(Value::text).and_then(box_of) {
            wires.push(ViewWire {
                from: TIMELINE,
                to,
                input: 2,
            });
        }
    }
    (nodes, wires)
}

/// Whether what `name` starts comes back round to `what`.
fn starts_in_the_end(document: &Document, name: &str, what: &str, seen: &mut Vec<String>) -> bool {
    if name == what {
        return true;
    }
    if seen.iter().any(|known| known == name) {
        return false;
    }
    seen.push(name.to_owned());
    let found = document.blocks_of("emitter").find(|b| name_of(b) == name);
    STARTS.iter().any(|key| {
        let next = found.and_then(|block| block.get(key)).and_then(Value::text);
        next.is_some_and(|next| starts_in_the_end(document, next, what, seen))
    })
}

/// Has box `from` start box `to` by row `input`. What would go round for ever isn't joined, and a
/// row only takes what it says it does.
pub fn join(document: &mut Document, from: usize, to: usize, input: usize) -> bool {
    let Some(name) = emitter_of(document, to).map(|block| name_of(block).to_owned()) else {
        return false;
    };
    if from == TIMELINE {
        if input != 2 {
            return false;
        }
        let mut block = Block::new("timeline");
        block.set("at", Value::Number(0.0));
        block.set("start", Value::Text(name));
        document.blocks.push(block);
        return true;
    }
    let Some(key) = STARTS.get(input) else {
        return false;
    };
    let Some(source) = emitter_of(document, from).map(|block| name_of(block).to_owned()) else {
        return false;
    };
    if starts_in_the_end(document, &name, &source, &mut Vec::new()) {
        return false;
    }
    if let Some(block) = emitter_mut(document, from) {
        set_field(block, key, Value::Text(name));
    }
    true
}

/// Does what was done in the node view to `document`, and says whether that changed it.
pub fn apply(document: &mut Document, event: ViewEvent) -> bool {
    match event {
        ViewEvent::Chosen(_) => false,
        ViewEvent::Moved(TIMELINE, at) => {
            let found = document.blocks.iter_mut().find(|b| b.kind == "view");
            match found {
                Some(block) => set_field(block, "at", place(at)),
                None => {
                    let mut block = Block::new("view");
                    block.set("at", place(at));
                    document.blocks.push(block);
                }
            }
            true
        }
        ViewEvent::Moved(number, at) => emitter_mut(document, number).is_some_and(|block| {
            set_field(block, "at", place(at));
            true
        }),
        ViewEvent::Joined(wire) => join(document, wire.from, wire.to, wire.input),
        ViewEvent::Cut(number, input) => {
            let Some(name) = emitter_of(document, number).map(|b| name_of(b).to_owned()) else {
                return false;
            };
            forget_starts(document, &name, input);
            true
        }
    }
}

/// Each timeline's place among the document's blocks, with its time and what it starts.
#[must_use]
pub fn timeline_of(document: &Document) -> Vec<(usize, f64, String)> {
    let blocks = document.blocks.iter().enumerate();
    blocks
        .filter(|(_, block)| block.kind == "timeline")
        .map(|(at, block)| {
            let time = block.get("at").and_then(Value::number).unwrap_or(0.0);
            let start = block.get("start").and_then(Value::text).unwrap_or("");
            (at, time, start.to_owned())
        })
        .collect()
}

/// Sets when the timeline at block `at` goes.
pub fn set_timeline(document: &mut Document, at: usize, time: f64) {
    if let Some(block) = document.blocks.get_mut(at) {
        set_field(block, "at", Value::Number(time));
    }
}
