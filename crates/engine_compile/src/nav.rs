//! Working out where things can walk (proposal 0090).

use std::collections::HashSet;

use engine_core::math::Vec3;
use engine_map::{
    ClipChild, Contents, Map, NAV_WAYS, NavGrid, NavLeap, NavLeaps, NavPlace, NavWay,
};

use crate::light::in_parallel;

/// How far apart places are.
const SPACING: f32 = 16.0;

/// How high a step something can take, as `STEPSIZE` is.
const STEP: f32 = 18.0;

/// How far from a wall a place has to be, past where a walker fits.
const CLEARANCE: f32 = 2.0;

/// How finely a column is looked down.
const SCAN: f32 = 8.0;

/// How far above a walker's middle water has to reach for it to have to swim.
const HEAD: f32 = 16.0;

/// How far below the top of water a swimmer's middle is.
const AFLOAT: f32 = 4.0;

/// The furthest something can drop. A fall hurts from about 264 units.
const MOST_DROP: f32 = 224.0;

/// How far up a jump can land, of the 45 or so it reaches.
const MOST_CLIMB: f32 = 40.0;

/// How high a jump's arc goes above where it starts or lands, whichever's higher.
const ARC: f32 = 44.0;

/// The longest jump, in cells. A jump at a run reaches about 200 units, and this leaves
/// room to get it wrong.
const LONGEST_JUMP: i32 = 10;

/// How far off an edge something can drop or swim, in cells.
const LEAP_REACH: i32 = 2;

/// How high a ledge something can climb out of water onto.
const WATER_LIP: f32 = 48.0;

/// What a place's link holds when there's none that way.
const NO_LINK: u32 = u32::MAX;

/// One leap of a kind is kept for each square of this many cells. Every place along an
/// edge could drop, and a path only needs to find one nearby.
const LEAP_SPREAD: i32 = 4;

/// The boxes that clipping hulls are grown by, as the hulls have them.
const HULL_SIZES: [(Vec3, Vec3); 2] = [
    (Vec3::new(-16.0, -16.0, -24.0), Vec3::new(16.0, 16.0, 32.0)),
    (Vec3::new(-32.0, -32.0, -24.0), Vec3::new(32.0, 32.0, 64.0)),
];

/// What's needed to ask about a clipping hull.
struct Hull<'a> {
    /// The map.
    map: &'a Map,
    /// Its root.
    root: ClipChild,
    /// The box it's grown by, for land, which isn't in hulls.
    size: (Vec3, Vec3),
}

impl Hull<'_> {
    /// Whether a player's centre can't be at `point`.
    fn solid(&self, point: Vec3) -> bool {
        self.map.tree.clip_solid(self.root, point)
            || self
                .map
                .terrain
                .iter()
                .any(|sheet| sheet.holds_box(point, self.size))
    }

    /// Where a player stands, going down from `open` to `solid`, to within a
    /// fraction of a unit.
    fn floor_between(&self, open: f32, solid: f32, x: f32, y: f32) -> f32 {
        let (mut high, mut low) = (open, solid);
        for _ in 0..6 {
            let middle = (high + low) * 0.5;
            if self.solid(Vec3::new(x, y, middle)) {
                low = middle;
            } else {
                high = middle;
            }
        }
        high
    }

    /// The floors of the column at `x` and `y`, top first.
    fn floors(&self, x: f32, y: f32, top: f32, bottom: f32) -> Vec<f32> {
        let mut out = Vec::new();
        let mut z = top;
        let mut was_open = !self.solid(Vec3::new(x, y, z));
        while z > bottom {
            let next = z - SCAN;
            let open = !self.solid(Vec3::new(x, y, next));
            if was_open && !open {
                out.push(self.floor_between(z, next, x, y));
            }
            was_open = open;
            z = next;
        }
        out
    }

    /// Whether a walker at `point` would be clear of walls, with a little room,
    /// so a path doesn't run along the very edge of where it fits. It looks from
    /// a step's height, as the next stair up isn't a wall.
    fn has_room(&self, point: Vec3) -> bool {
        let lift = Vec3::new(0.0, 0.0, STEP + 1.0);
        [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)]
            .iter()
            .all(|(x, y)| !self.solid(point + lift + Vec3::new(*x, *y, 0.0) * CLEARANCE))
    }

    /// Whether a line from `a` to `b` is clear.
    fn clear_line(&self, a: Vec3, b: Vec3) -> bool {
        let length = (b - a).length();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a short count"
        )]
        let count = (length / SCAN).ceil().max(1.0) as u32;
        (0..=count).all(|step| {
            #[expect(clippy::cast_precision_loss, reason = "a short count")]
            let share = step as f32 / count as f32;
            !self.solid(a + (b - a) * share)
        })
    }

    /// Whether something can go off the edge at `from`, and down to `to`.
    fn falls_clear(&self, from: Vec3, to: Vec3) -> bool {
        let above = Vec3::new(to.x, to.y, from.z);
        self.clear_line(from, above) && self.clear_line(above, to)
    }

    /// Whether an arc from `from` over to `to` is clear.
    fn arcs_clear(&self, from: Vec3, to: Vec3) -> bool {
        let top = from.z.max(to.z) + ARC;
        let middle = Vec3::new((from.x + to.x) * 0.5, (from.y + to.y) * 0.5, top);
        let rise = Vec3::new(from.x, from.y, from.z + ARC.min(top - from.z));
        let fall = Vec3::new(to.x, to.y, to.z + ARC.min(top - to.z));
        self.clear_line(from, rise)
            && self.clear_line(rise, middle)
            && self.clear_line(middle, fall)
            && self.clear_line(fall, to)
    }

    /// Whether something can step between two places next to each other.
    fn steps_between(&self, a: Vec3, b: Vec3) -> bool {
        if (a.z - b.z).abs() > STEP {
            return false;
        }
        // Checked at a step's height, as a walker lifts over a stair.
        let lift = Vec3::new(0.0, 0.0, STEP.min(a.z.max(b.z) - a.z.min(b.z) + 1.0));
        !self.solid((a + b) * 0.5 + lift)
    }
}

/// What can't be stood in, however solid it isn't.
fn harmful(map: &Map, point: Vec3) -> bool {
    let feet = point + Vec3::new(0.0, 0.0, -20.0);
    let contents = map
        .tree
        .leaf(map.tree.leaf_at(feet))
        .map(|leaf| leaf.contents);
    matches!(contents, Some(Contents::Lava | Contents::Slime))
}

/// What fills `point`.
fn contents_at(map: &Map, point: Vec3) -> Option<Contents> {
    map.tree
        .leaf(map.tree.leaf_at(point))
        .map(|leaf| leaf.contents)
}

/// Whether water at `point` reaches over a walker's head.
fn too_deep(map: &Map, point: Vec3) -> bool {
    contents_at(map, point + Vec3::new(0.0, 0.0, HEAD)) == Some(Contents::Water)
}

/// Where something floats in the water over `point`, if it's too deep there to stand.
fn afloat_over(map: &Map, point: Vec3) -> Option<Vec3> {
    if !too_deep(map, point) {
        return None;
    }
    let mut at = point + Vec3::new(0.0, 0.0, HEAD);
    while contents_at(map, at) == Some(Contents::Water) {
        at = at + Vec3::new(0.0, 0.0, SCAN);
    }
    // Back down to the top of the water, to within a unit.
    let (mut wet, mut dry) = (at.z - SCAN, at.z);
    while dry - wet > 1.0 {
        let middle = (wet + dry) * 0.5;
        if contents_at(map, Vec3::new(at.x, at.y, middle)) == Some(Contents::Water) {
            wet = middle;
        } else {
            dry = middle;
        }
    }
    Some(Vec3::new(at.x, at.y, wet - AFLOAT))
}

/// Whether `point` is afloat, with nothing under it to stand on.
fn is_afloat(map: &Map, hull: &Hull<'_>, point: Vec3) -> bool {
    contents_at(map, point) == Some(Contents::Water)
        && !hull.solid(point + Vec3::new(0.0, 0.0, -SCAN))
}

/// The walkable grid of `map`'s world, for what fits clipping hull `size`:
/// 0 for a player, 1 for a large monster.
#[must_use]
pub(crate) fn build_nav(map: &Map, size: usize) -> NavGrid {
    let Some(world) = map.models.first() else {
        return NavGrid::default();
    };
    let root = world.hulls[size];
    let hull = Hull {
        map,
        root,
        size: HULL_SIZES[size],
    };
    let (low, high) = (world.mins, world.maxs);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "maps are small"
    )]
    let size = [
        ((high.x - low.x) / SPACING).ceil() as u32 + 1,
        ((high.y - low.y) / SPACING).ceil() as u32 + 1,
    ];
    let mut grid = NavGrid {
        origin: low,
        spacing: SPACING,
        size,
        columns: Vec::with_capacity((size[0] * size[1] + 1) as usize),
        places: Vec::new(),
        leaps: NavLeaps::default(),
    };
    let rows: Vec<u32> = (0..size[1]).collect();
    let found = in_parallel(&rows, |y| {
        let mut row = Vec::with_capacity(size[0] as usize);
        for x in 0..size[0] {
            #[expect(clippy::cast_precision_loss, reason = "grids are small")]
            let (px, py) = (low.x + x as f32 * SPACING, low.y + *y as f32 * SPACING);
            let floors = hull.floors(px, py, high.z + 64.0, low.z - 64.0);
            let standing: Vec<Vec3> = floors
                .into_iter()
                .map(|floor| Vec3::new(px, py, floor))
                .collect();
            let afloat = standing.iter().filter_map(|point| afloat_over(map, *point));
            let dry = standing
                .iter()
                .copied()
                .filter(|point| !too_deep(map, *point));
            let places = afloat
                .chain(dry)
                .filter(|point| !harmful(map, *point) && hull.has_room(*point));
            row.push(places.map(NavPlace::at).collect::<Vec<_>>());
        }
        row
    });
    for column in found.into_iter().flatten() {
        grid.columns
            .push(u32::try_from(grid.places.len()).unwrap_or(0));
        grid.places.extend(column);
    }
    grid.columns
        .push(u32::try_from(grid.places.len()).unwrap_or(0));
    let afloat: Vec<bool> = grid
        .places
        .iter()
        .map(|place| is_afloat(map, &hull, place.point))
        .collect();
    link_places(&mut grid, &hull, &afloat);
    let mut leaps = find_leaps(&grid, &hull, &afloat);
    leaps.extend(find_lifts(map, &grid));
    grid.leaps = NavLeaps::new(leaps);
    grid
}

/// The way from `from` to `to` that isn't a walk, if there's one, for places `cells`
/// apart with nothing to walk on between them.
fn leap_between(
    hull: &Hull<'_>,
    (from, from_afloat): (Vec3, bool),
    (to, to_afloat): (Vec3, bool),
    cells: i32,
) -> Option<NavWay> {
    let rise = to.z - from.z;
    if from_afloat != to_afloat {
        let reach = cells <= LEAP_REACH;
        let fits = if from_afloat {
            rise <= WATER_LIP
        } else {
            rise <= 0.0 && -rise <= MOST_DROP
        };
        return (reach && fits && hull.arcs_clear(from, to)).then_some(NavWay::Swim);
    }
    if from_afloat {
        return None;
    }
    if cells <= LEAP_REACH && rise < -STEP && -rise <= MOST_DROP && hull.falls_clear(from, to) {
        return Some(NavWay::Drop);
    }
    let jump = cells > 1 && rise <= MOST_CLIMB && -rise <= MOST_DROP;
    (jump && hull.arcs_clear(from, to)).then_some(NavWay::Jump)
}

/// Every way off an edge, over a gap, and in and out of water.
fn find_leaps(grid: &NavGrid, hull: &Hull<'_>, afloat: &[bool]) -> Vec<NavLeap> {
    let [width, height] = grid.size.map(|size| i32::try_from(size).unwrap_or(0));
    let column = |x: i32, y: i32| {
        (x >= 0 && y >= 0 && x < width && y < height).then(|| {
            let at = usize::try_from(y * width + x).unwrap_or(0);
            grid.columns[at]..grid.columns[at + 1]
        })
    };
    let rows: Vec<i32> = (0..height).collect();
    let found = in_parallel(&rows, |y| {
        let mut out = Vec::new();
        for x in 0..width {
            let Some(here) = column(x, *y) else {
                continue;
            };
            for place in here {
                let from = grid.places[place as usize];
                let from_afloat = afloat[place as usize];
                for (way, (dx, dy)) in NAV_WAYS.iter().enumerate() {
                    // Where something can walk, it doesn't need to leap.
                    if from.links[way] != NO_LINK {
                        continue;
                    }
                    for cells in 1..=LONGEST_JUMP {
                        let Some(others) = column(x + dx * cells, *y + dy * cells) else {
                            break;
                        };
                        let leap = others.into_iter().find_map(|other| {
                            let to = grid.places[other as usize].point;
                            let way = leap_between(
                                hull,
                                (from.point, from_afloat),
                                (to, afloat[other as usize]),
                                cells,
                            )?;
                            Some(NavLeap {
                                from: from.point,
                                to,
                                way,
                            })
                        });
                        if let Some(leap) = leap {
                            out.push(leap);
                            break;
                        }
                    }
                }
            }
        }
        out
    });
    let mut seen = HashSet::new();
    let square = |point: Vec3| {
        #[expect(clippy::cast_possible_truncation, reason = "grids are small")]
        let cell = |value: f32| (value / (grid.spacing * LEAP_SPREAD as f32)).floor() as i32;
        [cell(point.x), cell(point.y), cell(point.z)]
    };
    found
        .into_iter()
        .flatten()
        .filter(|leap| seen.insert((leap.way, square(leap.from), square(leap.to))))
        .collect()
}

/// A way up each lift. A `func_plat` is placed at its top and rests at its bottom, and
/// the hull doesn't hold it, so its shaft's floor and the ledge at its top are places.
fn find_lifts(map: &Map, grid: &NavGrid) -> Vec<NavLeap> {
    let mut out = Vec::new();
    for entity in map.entities.iter() {
        if entity.get("classname") != Some("func_plat") {
            continue;
        }
        let number = entity
            .get("model")
            .and_then(|model| model.strip_prefix('*'))
            .and_then(|number| number.parse::<usize>().ok());
        let Some(model) = number.and_then(|number| map.models.get(number)) else {
            continue;
        };
        let size = model.maxs - model.mins;
        let travel = entity
            .get("height")
            .and_then(|height| height.parse::<f32>().ok())
            .unwrap_or(size.z - 8.0);
        let (x, y) = (
            (model.mins.x + model.maxs.x) * 0.5,
            (model.mins.y + model.maxs.y) * 0.5,
        );
        let top = model.maxs.z + 24.0;
        let Some(bottom) = grid.nearest(Vec3::new(x, y, top - travel)) else {
            continue;
        };
        let bottom = grid.places[bottom as usize].point;
        let reach = size.x.max(size.y) * 0.5 + grid.spacing * 2.0;
        let ledges = NAV_WAYS.iter().filter_map(|(dx, dy)| {
            #[expect(clippy::cast_precision_loss, reason = "one step each way")]
            let spot = Vec3::new(x + *dx as f32 * reach, y + *dy as f32 * reach, top);
            let place = grid.nearest(spot)?;
            let point = grid.places[place as usize].point;
            ((point.z - top).abs() <= STEP).then_some(point)
        });
        out.extend(ledges.map(|to| NavLeap {
            from: bottom,
            to,
            way: NavWay::Lift,
        }));
    }
    out
}

/// Links each place to what can be walked to next to it. What's afloat only joins what
/// isn't by a leap, so that a way stays dry where it can.
fn link_places(grid: &mut NavGrid, hull: &Hull<'_>, afloat: &[bool]) {
    let [width, height] = grid.size.map(|size| i32::try_from(size).unwrap_or(0));
    let in_column = |grid: &NavGrid, x: i32, y: i32| {
        (x >= 0 && y >= 0 && x < width && y < height).then(|| {
            let column = usize::try_from(y * width + x).unwrap_or(0);
            grid.columns[column]..grid.columns[column + 1]
        })
    };
    let rows: Vec<i32> = (0..height).collect();
    let shared: &NavGrid = grid;
    let linked = in_parallel(&rows, |y| {
        let (grid, y) = (shared, *y);
        let mut row = Vec::new();
        for x in 0..width {
            let Some(here) = in_column(grid, x, y) else {
                continue;
            };
            for place in here {
                let point = grid.places[place as usize].point;
                let mut links = grid.places[place as usize].links;
                let joined = |grid: &NavGrid, dx: i32, dy: i32| {
                    in_column(grid, x + dx, y + dy).and_then(|others| {
                        others.into_iter().find(|other| {
                            afloat[place as usize] == afloat[*other as usize]
                                && hull.steps_between(point, grid.places[*other as usize].point)
                        })
                    })
                };
                for (way, (dx, dy)) in NAV_WAYS.iter().enumerate() {
                    // A corner is only cut if both sides of it are open.
                    let open = *dx == 0
                        || *dy == 0
                        || (joined(grid, *dx, 0).is_some() && joined(grid, 0, *dy).is_some());
                    if let Some(other) = joined(grid, *dx, *dy).filter(|_| open) {
                        links[way] = other;
                    }
                }
                row.push(links);
            }
        }
        row
    });
    // Rows come back in order, and places are kept in that order.
    for (place, links) in grid.places.iter_mut().zip(linked.into_iter().flatten()) {
        place.links = links;
    }
}
