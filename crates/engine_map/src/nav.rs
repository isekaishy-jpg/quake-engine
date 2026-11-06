//! Finding a way round a map: a walkable grid with paths through it, and splines
//! for movement set out by hand (proposal 0090).

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::hash::Hash;

use engine_core::math::Vec3;

/// The most links a place has, one each way round.
pub const LINKS: usize = 8;

/// The eight ways round a place, as columns.
pub const NAV_WAYS: [(i32, i32); LINKS] = [
    (1, 0),
    (0, 1),
    (-1, 0),
    (0, -1),
    (1, 1),
    (-1, 1),
    (1, -1),
    (-1, -1),
];

/// No link.
pub(crate) const NONE: u32 = u32::MAX;

/// How far off the walkable grid a point can start or end, in cells.
pub(crate) const SEARCH_REACH: i32 = 3;

/// How many places a search looks at before giving up, so a goal nothing
/// reaches can't walk a whole world.
const SEARCH_BUDGET: usize = 60_000;

/// How far apart points of a line are checked, as a share of a cell.
const LINE_STEP: f32 = 0.5;

/// A place something can stand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavPlace {
    /// Where its centre stands.
    pub point: Vec3,
    /// The places that can be walked to from here.
    pub links: [u32; LINKS],
}

impl NavPlace {
    /// A place with nothing linked.
    #[must_use]
    pub const fn at(point: Vec3) -> Self {
        Self {
            point,
            links: [NONE; LINKS],
        }
    }
}

/// Where things can walk, as places on a grid, several high where floors stack.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NavGrid {
    /// The grid's low corner.
    pub origin: Vec3,
    /// How far apart places are.
    pub spacing: f32,
    /// How many columns across and along.
    pub size: [u32; 2],
    /// The first place of each column, with one more at the end.
    pub columns: Vec<u32>,
    /// The places, column by column.
    pub places: Vec<NavPlace>,
}

/// A place in a search, by how good it looks.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Open<P> {
    /// How long a way through here looks.
    score: f32,
    /// The place.
    place: P,
}

impl<P: PartialEq> Eq for Open<P> {}

impl<P: PartialEq> Ord for Open<P> {
    fn cmp(&self, other: &Self) -> Ordering {
        other.score.total_cmp(&self.score)
    }
}

impl<P: PartialEq> PartialOrd for Open<P> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// What a search needs to know about somewhere to walk.
pub(crate) trait Walk {
    /// A place.
    type Place: Copy + Eq + Hash;

    /// How far apart places are.
    fn spacing(&self) -> f32;

    /// Where a place is.
    fn point(&self, place: Self::Place) -> Vec3;

    /// The places a place links to.
    fn links(&self, place: Self::Place) -> [Option<Self::Place>; LINKS];

    /// The place nearest `point`.
    fn nearest_place(&self, point: Vec3) -> Option<Self::Place>;
}

/// Whether something can walk straight from `from` to `to`, staying on the grid
/// and only changing height by steps it can take.
fn walks_straight<W: Walk>(walk: &W, from: W::Place, to: W::Place) -> bool {
    let (start, end) = (walk.point(from), walk.point(to));
    let step = walk.spacing() * LINE_STEP;
    let length = (end - start).length();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a short count"
    )]
    let count = (length / step).ceil() as u32;
    let mut here = from;
    for index in 1..=count {
        #[expect(clippy::cast_precision_loss, reason = "a short count")]
        let share = index as f32 / count.max(1) as f32;
        let point = start + (end - start) * share;
        let Some(next) = step_towards(walk, here, point) else {
            return false;
        };
        here = next;
    }
    true
}

/// `here`, or the linked place nearest `point`, if it's within a cell.
fn step_towards<W: Walk>(walk: &W, here: W::Place, point: Vec3) -> Option<W::Place> {
    let mut best = here;
    let flat = |at: Vec3| {
        let offset = at - point;
        (offset.x * offset.x + offset.y * offset.y).sqrt()
    };
    let mut closest = flat(walk.point(here));
    for link in walk.links(here).into_iter().flatten() {
        let distance = flat(walk.point(link));
        if distance < closest {
            closest = distance;
            best = link;
        }
    }
    (closest <= walk.spacing() * 0.75).then_some(best)
}

/// The places from `start` to `goal`, by A*, or `None` if there's no way
/// within [`SEARCH_BUDGET`].
fn search<W: Walk>(walk: &W, start: W::Place, goal: W::Place) -> Option<Vec<W::Place>> {
    let mut came_from: HashMap<W::Place, W::Place> = HashMap::new();
    let mut cost: HashMap<W::Place, f32> = HashMap::new();
    let target = walk.point(goal);
    let mut open = BinaryHeap::new();
    cost.insert(start, 0.0);
    open.push(Open {
        score: 0.0,
        place: start,
    });
    let mut visited = 0;
    while let Some(Open { place, .. }) = open.pop() {
        if place == goal {
            let mut route = vec![goal];
            let mut at = goal;
            while at != start {
                at = *came_from.get(&at)?;
                route.push(at);
            }
            route.reverse();
            return Some(route);
        }
        visited += 1;
        if visited > SEARCH_BUDGET {
            return None;
        }
        let here = walk.point(place);
        let so_far = cost.get(&place).copied().unwrap_or(f32::MAX);
        for link in walk.links(place).into_iter().flatten() {
            let there = walk.point(link);
            let next = so_far + (there - here).length();
            if next < cost.get(&link).copied().unwrap_or(f32::MAX) {
                cost.insert(link, next);
                came_from.insert(link, place);
                open.push(Open {
                    score: next + (there - target).length(),
                    place: link,
                });
            }
        }
    }
    None
}

/// The way from `from` to `to`, with its corners cut where that's walkable, or
/// `None` if there's no way. It starts and ends at the places nearest them.
pub(crate) fn find_path_on<W: Walk>(walk: &W, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
    let (start, goal) = (walk.nearest_place(from)?, walk.nearest_place(to)?);
    let route = search(walk, start, goal)?;
    let mut kept = vec![start];
    let mut at = 0;
    while at + 1 < route.len() {
        let mut furthest = at + 1;
        for later in (at + 2..route.len()).rev() {
            if walks_straight(walk, route[at], route[later]) {
                furthest = later;
                break;
            }
        }
        kept.push(route[furthest]);
        at = furthest;
    }
    Some(kept.iter().map(|place| walk.point(*place)).collect())
}

impl NavGrid {
    /// The column of a grid position, if it's on the grid.
    fn column(&self, x: i32, y: i32) -> Option<usize> {
        let (x, y) = (u32::try_from(x).ok()?, u32::try_from(y).ok()?);
        (x < self.size[0] && y < self.size[1]).then(|| (y * self.size[0] + x) as usize)
    }

    /// The grid position of `point`.
    fn cell_of(&self, point: Vec3) -> (i32, i32) {
        let local = (point - self.origin) * (1.0 / self.spacing.max(1.0));
        #[expect(clippy::cast_possible_truncation, reason = "grids are small")]
        let cell = (local.x.round() as i32, local.y.round() as i32);
        cell
    }

    /// The places of a column.
    pub(crate) fn places_in(&self, column: usize) -> std::ops::Range<u32> {
        let first = self.columns.get(column).copied().unwrap_or(0);
        let last = self.columns.get(column + 1).copied().unwrap_or(first);
        first..last
    }

    /// The place nearest `point`, near it on the grid.
    #[must_use]
    pub fn nearest(&self, point: Vec3) -> Option<u32> {
        let (x, y) = self.cell_of(point);
        let mut best = None;
        let mut closest = f32::MAX;
        for ring in 0..=SEARCH_REACH {
            for dy in -ring..=ring {
                for dx in -ring..=ring {
                    let Some(column) = self.column(x + dx, y + dy) else {
                        continue;
                    };
                    for place in self.places_in(column) {
                        let at = self.places[place as usize].point;
                        let distance = (at - point).length();
                        if distance < closest {
                            closest = distance;
                            best = Some(place);
                        }
                    }
                }
            }
            if best.is_some() {
                break;
            }
        }
        best
    }

    /// The way from `from` to `to` over the grid, with its corners cut where
    /// that's walkable, or `None` if there's no way. It starts and ends at the
    /// places nearest them.
    #[must_use]
    pub fn find_path(&self, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
        find_path_on(self, from, to)
    }
}

impl Walk for NavGrid {
    type Place = u32;

    fn spacing(&self) -> f32 {
        self.spacing
    }

    fn point(&self, place: u32) -> Vec3 {
        self.places[place as usize].point
    }

    fn links(&self, place: u32) -> [Option<u32>; LINKS] {
        self.places[place as usize]
            .links
            .map(|link| (link != NONE).then_some(link))
    }

    fn nearest_place(&self, point: Vec3) -> Option<u32> {
        self.nearest(point)
    }
}

/// A smooth line through points, for movement set out by hand.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Spline {
    /// What scripts call it.
    pub name: String,
    /// The points it passes through.
    pub points: Vec<Vec3>,
    /// Whether it joins back to its start.
    pub closed: bool,
}

impl Spline {
    /// The point at `share` of the way along, from 0 to 1, by point and not by
    /// distance. A closed line wraps round.
    #[must_use]
    pub fn point_at(&self, share: f32) -> Option<Vec3> {
        let count = self.points.len();
        let first = *self.points.first()?;
        if count < 2 {
            return Some(first);
        }
        #[expect(clippy::cast_precision_loss, reason = "splines are short")]
        let legs = if self.closed { count } else { count - 1 } as f32;
        let along = if self.closed {
            share.rem_euclid(1.0)
        } else {
            share.clamp(0.0, 1.0)
        } * legs;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "within the spline"
        )]
        let leg = (along as usize).min(legs as usize - 1);
        #[expect(clippy::cast_precision_loss, reason = "splines are short")]
        let part = along - leg as f32;
        #[expect(clippy::cast_possible_wrap, reason = "splines are short")]
        let point = |index: isize| {
            let total = count as isize;
            let at = if self.closed {
                index.rem_euclid(total)
            } else {
                index.clamp(0, total - 1)
            };
            #[expect(clippy::cast_sign_loss, reason = "wrapped or clamped to the points")]
            self.points[at as usize]
        };
        #[expect(clippy::cast_possible_wrap, reason = "splines are short")]
        let at = leg as isize;
        let (a, b, c, d) = (point(at - 1), point(at), point(at + 1), point(at + 2));
        let (t, t2, t3) = (part, part * part, part * part * part);
        // Catmull-Rom, which passes through its points.
        Some(
            (b * 2.0
                + (c - a) * t
                + (a * 2.0 - b * 5.0 + c * 4.0 - d) * t2
                + (b * 3.0 - a - c * 3.0 + d) * t3)
                * 0.5,
        )
    }
}
