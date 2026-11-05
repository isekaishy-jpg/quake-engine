//! Working out where things can walk (proposal 0090).

use engine_core::math::Vec3;
use engine_map::{ClipChild, Contents, Map, NAV_WAYS, NavGrid, NavPlace};

/// How far apart places are.
const SPACING: f32 = 16.0;

/// How high a step something can take, as `STEPSIZE` is.
const STEP: f32 = 18.0;

/// How far from a wall a place has to be, past where a walker fits.
const CLEARANCE: f32 = 2.0;

/// How finely a column is looked down.
const SCAN: f32 = 8.0;

/// What's needed to ask about a clipping hull.
struct Hull<'a> {
    /// The map.
    map: &'a Map,
    /// Its root.
    root: ClipChild,
}

impl Hull<'_> {
    /// Whether a player's centre can't be at `point`.
    fn solid(&self, point: Vec3) -> bool {
        self.map.tree.clip_solid(self.root, point)
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
    /// so a path doesn't run along the very edge of where it fits.
    fn has_room(&self, point: Vec3) -> bool {
        let lift = Vec3::new(0.0, 0.0, 4.0);
        [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)]
            .iter()
            .all(|(x, y)| !self.solid(point + lift + Vec3::new(*x, *y, 0.0) * CLEARANCE))
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

/// The walkable grid of `map`'s world, for what fits clipping hull `size`:
/// 0 for a player, 1 for a large monster.
#[must_use]
pub(crate) fn build_nav(map: &Map, size: usize) -> NavGrid {
    let Some(world) = map.models.first() else {
        return NavGrid::default();
    };
    let root = world.hulls[size];
    let hull = Hull { map, root };
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
    };
    for y in 0..size[1] {
        for x in 0..size[0] {
            grid.columns
                .push(u32::try_from(grid.places.len()).unwrap_or(0));
            #[expect(clippy::cast_precision_loss, reason = "grids are small")]
            let (px, py) = (low.x + x as f32 * SPACING, low.y + y as f32 * SPACING);
            for floor in hull.floors(px, py, high.z + 64.0, low.z - 64.0) {
                let point = Vec3::new(px, py, floor);
                if !harmful(map, point) && hull.has_room(point) {
                    grid.places.push(NavPlace::at(point));
                }
            }
        }
    }
    grid.columns
        .push(u32::try_from(grid.places.len()).unwrap_or(0));
    link_places(&mut grid, &hull);
    grid
}

/// Links each place to what can be walked to next to it.
fn link_places(grid: &mut NavGrid, hull: &Hull<'_>) {
    let [width, height] = grid.size.map(|size| i32::try_from(size).unwrap_or(0));
    let in_column = |grid: &NavGrid, x: i32, y: i32| {
        (x >= 0 && y >= 0 && x < width && y < height).then(|| {
            let column = usize::try_from(y * width + x).unwrap_or(0);
            grid.columns[column]..grid.columns[column + 1]
        })
    };
    for y in 0..height {
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
                            hull.steps_between(point, grid.places[*other as usize].point)
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
                grid.places[place as usize].links = links;
            }
        }
    }
}
