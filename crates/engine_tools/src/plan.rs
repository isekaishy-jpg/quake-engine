//! A level as a floor plan (proposal 0119).
//!
//! The simplest way to make a level: paint squares on graph paper, and put things
//! on them. The plan is kept in the map, and what it makes is ordinary brushes and
//! entities, so everything else carries on working, and an advanced author can
//! take it further by hand.

use std::collections::{BTreeMap, BTreeSet};

use engine_format::{Block, Value};
use serde_json::{Value as Json, json};

use crate::json::numbers;
use crate::session::Session;

/// How wide a square of the plan is, in world units.
pub const CELL: f64 = 128.0;

/// How high rooms are.
const ROOM_HEIGHT: f64 = 192.0;

/// How thick walls, floors and ceilings are.
const THICK: f64 = 32.0;

/// How far below the floor a pool's bed is.
const POOL_DEPTH: f64 = 96.0;

/// What marks what a plan made, so it can be made again.
const MADE: &str = "from_plan";

/// The look a new plan has.
const FIRST_LOOK: &str = "uc01_blight_sewers";

/// What a square is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Square {
    /// Floor to walk on.
    Floor,
    /// A pool.
    Water,
}

/// Which things can go on a square, with what each becomes in the level.
const THINGS: [(&str, &str); 7] = [
    ("start", "info_player_start"),
    ("light", "light"),
    ("door", "func_door"),
    ("health", "item_health"),
    ("ammo", "item_shells"),
    ("enemy", "monster_army"),
    ("fire", "flame"),
];

/// A floor plan.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// What's painted, by column and row.
    pub squares: BTreeMap<(i32, i32), Square>,
    /// What's on them.
    pub things: BTreeMap<(i32, i32), String>,
    /// The pictures it's made of, as the start of their materials' names.
    pub look: String,
}

impl Plan {
    /// The plan in `session`'s map.
    #[must_use]
    pub fn of(session: &Session) -> Self {
        let mut plan = Self {
            look: FIRST_LOOK.to_owned(),
            ..Self::default()
        };
        let Some(block) = session.document.blocks_of("plan").next() else {
            return plan;
        };
        if let Some(look) = block.get("look").and_then(Value::text) {
            look.clone_into(&mut plan.look);
        }
        let list = |key: &str| block.get(key).and_then(Value::numbers).unwrap_or_default();
        #[expect(clippy::cast_possible_truncation, reason = "plans are small")]
        for [x, y] in list("floor").as_chunks::<2>().0 {
            plan.squares.insert((*x as i32, *y as i32), Square::Floor);
        }
        #[expect(clippy::cast_possible_truncation, reason = "plans are small")]
        for [x, y] in list("water").as_chunks::<2>().0 {
            plan.squares.insert((*x as i32, *y as i32), Square::Water);
        }
        for (name, _) in THINGS {
            #[expect(clippy::cast_possible_truncation, reason = "plans are small")]
            for [x, y] in list(name).as_chunks::<2>().0 {
                plan.things.insert((*x as i32, *y as i32), name.to_owned());
            }
        }
        plan
    }

    /// The plan as a block.
    fn block(&self) -> Block {
        let mut block = Block::new("plan");
        block.set("look", Value::Text(self.look.clone()));
        let squares = |wanted: Square| -> Vec<f64> {
            self.squares
                .iter()
                .filter(|(_, square)| **square == wanted)
                .flat_map(|((x, y), _)| [f64::from(*x), f64::from(*y)])
                .collect()
        };
        block.set("floor", numbers(&squares(Square::Floor)));
        block.set("water", numbers(&squares(Square::Water)));
        for (name, _) in THINGS {
            let places: Vec<f64> = self
                .things
                .iter()
                .filter(|(_, thing)| thing.as_str() == name)
                .flat_map(|((x, y), _)| [f64::from(*x), f64::from(*y)])
                .collect();
            block.set(name, numbers(&places));
        }
        block
    }

    /// What's still missing, in plain words, for a level that can be played.
    #[must_use]
    pub fn missing(&self) -> Vec<&'static str> {
        let has = |name: &str| self.things.values().any(|thing| thing == name);
        let mut out = Vec::new();
        if self.squares.is_empty() {
            out.push("Paint a room");
        }
        if !has("start") {
            out.push("Choose where you start");
        }
        if !has("light") {
            out.push("Add a light");
        }
        out
    }
}

/// The name of a material of `look`.
fn material(look: &str, part: &str) -> String {
    format!("{look}_{part}")
}

/// A brush a plan made.
fn brush(material: &str, cuboid: [f64; 6]) -> Block {
    let mut block = Block::new("brush");
    block.set("material", Value::Text(material.to_owned()));
    block.set("cuboid", numbers(&cuboid));
    block.set(MADE, Value::Flag(true));
    block
}

/// Runs of squares side by side in a row that pass `wanted`, as a row and first
/// and last columns. Fewer, longer brushes make for lighter levels.
fn runs(squares: &BTreeSet<(i32, i32)>) -> Vec<(i32, i32, i32)> {
    let mut by_row: BTreeMap<i32, Vec<i32>> = BTreeMap::new();
    for (x, y) in squares {
        by_row.entry(*y).or_default().push(*x);
    }
    let mut out = Vec::new();
    for (y, mut columns) in by_row {
        columns.sort_unstable();
        let mut start = columns[0];
        let mut last = columns[0];
        for x in columns.into_iter().skip(1) {
            if x != last + 1 {
                out.push((y, start, last));
                start = x;
            }
            last = x;
        }
        out.push((y, start, last));
    }
    out
}

/// The edges of a square, in the world.
fn edges(x: i32, y: i32) -> [f64; 4] {
    let (x, y) = (f64::from(x), f64::from(y));
    [x * CELL, y * CELL, (x + 1.0) * CELL, (y + 1.0) * CELL]
}

/// The walls round what's painted. A wall only reaches past its square at an outer
/// corner, where the next wall meets it. Anywhere else it would stick into an opening
/// and narrow it.
fn walls(plan: &Plan, wall: &str) -> Vec<Block> {
    let open = |x: i32, y: i32| plan.squares.contains_key(&(x, y));
    let reach = |x: i32, y: i32| if open(x, y) { 0.0 } else { THICK };
    let (low, high) = (-POOL_DEPTH - THICK, ROOM_HEIGHT + THICK);
    let mut out = Vec::new();
    for (x, y) in plan.squares.keys().copied() {
        let [left, bottom, right, top] = edges(x, y);
        let (south, north) = (bottom - reach(x, y - 1), top + reach(x, y + 1));
        let (west, east) = (left - reach(x - 1, y), right + reach(x + 1, y));
        if !open(x - 1, y) {
            out.push(brush(wall, [left - THICK, south, low, left, north, high]));
        }
        if !open(x + 1, y) {
            out.push(brush(wall, [right, south, low, right + THICK, north, high]));
        }
        if !open(x, y - 1) {
            out.push(brush(wall, [west, bottom - THICK, low, east, bottom, high]));
        }
        if !open(x, y + 1) {
            out.push(brush(wall, [west, top, low, east, top + THICK, high]));
        }
    }
    out
}

/// The door on a square: it fills the way through, across whichever way is open.
fn door(plan: &Plan, x: i32, y: i32, picture: &str) -> Block {
    let [left, bottom, right, top] = edges(x, y);
    let (middle_x, middle_y) = ((left + right) * 0.5, (bottom + top) * 0.5);
    let open = |x: i32, y: i32| plan.squares.contains_key(&(x, y));
    let across = open(x - 1, y) || open(x + 1, y);
    let cuboid = if across {
        [
            middle_x - 8.0,
            bottom,
            0.0,
            middle_x + 8.0,
            top,
            ROOM_HEIGHT,
        ]
    } else {
        [
            left,
            middle_y - 8.0,
            0.0,
            right,
            middle_y + 8.0,
            ROOM_HEIGHT,
        ]
    };
    let mut block = Block::new("entity");
    block.set("type", Value::Text("func_door".to_owned()));
    block.set("angle", Value::Number(-1.0));
    block.set("wait", Value::Number(3.0));
    block.set("speed", Value::Number(200.0));
    block.set("sounds", Value::Number(1.0));
    block.set(MADE, Value::Flag(true));
    block.children.push(brush(picture, cuboid));
    block
}

/// The thing `name` on the square at `x` and `y`.
fn thing(plan: &Plan, name: &str, x: i32, y: i32) -> Option<Block> {
    if name == "door" {
        return Some(door(plan, x, y, &material(&plan.look, "door")));
    }
    let kind = THINGS.iter().find(|(known, _)| *known == name)?.1;
    let [left, bottom, right, top] = edges(x, y);
    let (middle_x, middle_y) = ((left + right) * 0.5, (bottom + top) * 0.5);
    let height = match name {
        "light" => ROOM_HEIGHT - 40.0,
        "fire" => 8.0,
        _ => 32.0,
    };
    let mut block = Block::new("entity");
    block.set("type", Value::Text(kind.to_owned()));
    block.set("origin", numbers(&[middle_x, middle_y, height]));
    block.set(MADE, Value::Flag(true));
    if name == "light" {
        block.set("intensity", Value::Number(300.0));
        block.set("size", Value::Number(12.0));
    }
    Some(block)
}

/// Everything `plan` makes.
fn made(plan: &Plan) -> Vec<Block> {
    let floor = material(&plan.look, "floor_flags");
    let wall = material(&plan.look, "wall_blocks");
    let ceiling = material(&plan.look, "ceiling_coffer");
    let liquid = material(&plan.look, "liquid");
    let mut world = Block::named("entity", "world");
    world.set("type", Value::Text("worldspawn".to_owned()));
    world.set("ambient", Value::Number(24.0));
    world.set("light_spacing", Value::Number(32.0));
    world.set(MADE, Value::Flag(true));
    let mut out = vec![world];
    let all: BTreeSet<(i32, i32)> = plan.squares.keys().copied().collect();
    let dry: BTreeSet<(i32, i32)> = plan
        .squares
        .iter()
        .filter(|(_, square)| **square == Square::Floor)
        .map(|(place, _)| *place)
        .collect();
    let wet: BTreeSet<(i32, i32)> = all.difference(&dry).copied().collect();
    for (y, first, last) in runs(&dry) {
        let [left, bottom, _, _] = edges(first, y);
        let [_, _, right, top] = edges(last, y);
        out.push(brush(
            &floor,
            [left, bottom, -POOL_DEPTH - THICK, right, top, 0.0],
        ));
    }
    for (y, first, last) in runs(&wet) {
        let [left, bottom, _, _] = edges(first, y);
        let [_, _, right, top] = edges(last, y);
        let bed = -POOL_DEPTH;
        out.push(brush(&wall, [left, bottom, bed - THICK, right, top, bed]));
        let mut water = brush(&liquid, [left, bottom, bed, right, top, -8.0]);
        water.set("contents", Value::Text("water".to_owned()));
        out.push(water);
    }
    for (y, first, last) in runs(&all) {
        let [left, bottom, _, _] = edges(first, y);
        let [_, _, right, top] = edges(last, y);
        out.push(brush(
            &ceiling,
            [left, bottom, ROOM_HEIGHT, right, top, ROOM_HEIGHT + THICK],
        ));
    }
    out.extend(walls(plan, &wall));
    for ((x, y), name) in &plan.things {
        if plan.squares.contains_key(&(*x, *y)) {
            out.extend(thing(plan, name, *x, *y));
        }
    }
    out
}

/// Puts `plan` in the map, in place of what the last one made.
fn apply(session: &mut Session, plan: &Plan) {
    session.checkpoint();
    session
        .document
        .blocks
        .retain(|block| block.kind != "plan" && block.get(MADE).is_none());
    let mut block = plan.block();
    let _id = session.give_id(&mut block);
    session.document.blocks.push(block);
    for mut block in made(plan) {
        let _id = session.give_id(&mut block);
        session.document.blocks.push(block);
    }
    session.dirty = true;
}

/// The squares named in `args`.
fn squares_of(args: &Json) -> Result<Vec<(i32, i32)>, String> {
    let list = args
        .get("cells")
        .and_then(Json::as_array)
        .ok_or("cells needs a list of column and row pairs")?;
    let pair = |item: &Json| -> Option<(i32, i32)> {
        let item = item.as_array()?;
        let number = |at: usize| i32::try_from(item.get(at)?.as_i64()?).ok();
        Some((number(0)?, number(1)?))
    };
    list.iter()
        .map(|item| pair(item).ok_or_else(|| "a cell is a column and a row".to_owned()))
        .collect()
}

/// What a plan is, for whoever asked.
fn answer(plan: &Plan) -> Json {
    let list = |wanted: Square| -> Vec<[i32; 2]> {
        plan.squares
            .iter()
            .filter(|(_, square)| **square == wanted)
            .map(|((x, y), _)| [*x, *y])
            .collect()
    };
    let things: Vec<Json> = plan
        .things
        .iter()
        .map(|((x, y), name)| json!({ "cell": [x, y], "thing": name }))
        .collect();
    json!({
        "floor": list(Square::Floor),
        "water": list(Square::Water),
        "things": things,
        "look": plan.look,
        "missing": plan.missing(),
    })
}

/// `plan_read`: what the plan is.
pub(crate) fn plan_read(session: &Session) -> Json {
    answer(&Plan::of(session))
}

/// `plan_paint`: paints squares, as `floor` or `water`, or `none` to clear them.
pub(crate) fn plan_paint(session: &mut Session, args: &Json) -> Result<Json, String> {
    let cells = squares_of(args)?;
    let kind = args.get("kind").and_then(Json::as_str).unwrap_or("floor");
    let mut plan = Plan::of(session);
    for cell in cells {
        match kind {
            "floor" => {
                plan.squares.insert(cell, Square::Floor);
            }
            "water" => {
                plan.squares.insert(cell, Square::Water);
            }
            "none" => {
                plan.squares.remove(&cell);
                plan.things.remove(&cell);
            }
            other => return Err(format!("there's no {other} to paint")),
        }
    }
    apply(session, &plan);
    Ok(answer(&plan))
}

/// `plan_thing`: puts a thing on squares, or takes it off with `none`. There's
/// only one start, so it moves.
pub(crate) fn plan_thing(session: &mut Session, args: &Json) -> Result<Json, String> {
    let cells = squares_of(args)?;
    let name = args
        .get("thing")
        .and_then(Json::as_str)
        .ok_or("thing is required")?;
    if name != "none" && THINGS.iter().all(|(known, _)| *known != name) {
        return Err(format!("there's no {name} to put down"));
    }
    let mut plan = Plan::of(session);
    if name == "start" {
        plan.things.retain(|_, thing| thing != "start");
    }
    for cell in cells {
        if !plan.squares.contains_key(&cell) {
            return Err("that square has no floor yet".to_owned());
        }
        if name == "none" {
            plan.things.remove(&cell);
        } else {
            plan.things.insert(cell, name.to_owned());
        }
    }
    apply(session, &plan);
    Ok(answer(&plan))
}

/// `plan_look`: changes what the level's made of, by the start of its materials'
/// names.
pub(crate) fn plan_look(session: &mut Session, args: &Json) -> Result<Json, String> {
    let look = args
        .get("look")
        .and_then(Json::as_str)
        .ok_or("look is required")?;
    let mut plan = Plan::of(session);
    look.clone_into(&mut plan.look);
    apply(session, &plan);
    Ok(answer(&plan))
}

/// A map with no blocks of its own but what a plan made, which is the sort that
/// opens on the plan.
#[must_use]
pub fn is_plan_map(session: &Session) -> bool {
    let blocks = &session.document.blocks;
    blocks
        .iter()
        .all(|block| block.kind == "plan" || block.get(MADE).is_some())
}
