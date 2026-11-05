//! Traces through the map. Only this file knows `box3d`.

use engine_core::player::{PLAYER_MAXS, PLAYER_MINS};
use std::sync::Arc;

use box3d_rust::body::create_body;
use box3d_rust::hull::create_hull;
use box3d_rust::shape::create_hull_shape;
use box3d_rust::types::{
    default_body_def, default_query_filter, default_shape_def, default_world_def,
};
use box3d_rust::world::{World, world_cast_ray_closest};
use box3d_rust::{Pos, Vec3 as BoxVec3};
use engine_core::entity::ModelInstance;
use engine_core::math::Vec3;
use engine_core::player::Liquid;
use engine_map::{ClipChild, Contents, Map};

use crate::hull::{HullRoot, enter_box, trace_hull};
use crate::solids::build_solids;

/// How far short of a surface a trace stops, from `pmovetst.c`. It keeps the
/// player out of solid, which rays can't see from inside.
pub const DIST_EPSILON: f32 = 0.03125;

/// The most corners `box3d` takes in a hull.
const MAX_HULL_VERTICES: i32 = 128;

/// The most steps a stopping point is moved back.
const MAX_BACK_OUT_STEPS: u32 = 16;

/// How many halvings find a surface, which is finer than a float can tell on
/// a trace of any length.
const SEARCH_STEPS: u32 = 24;

/// How far around a model's bounds a trace still checks it, which covers the
/// growth of the largest hull. `solids` starts further out, so that what it
/// clips is always bigger than anything this can reach.
const MODEL_BOUNDS_MARGIN: f32 = 64.0;

/// The result of a trace.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trace {
    /// How far it got, from 0 to 1.
    pub fraction: f32,
    /// Where it stopped.
    pub end: Vec3,
    /// The normal of what it hit, or zero if it hit nothing.
    pub normal: Vec3,
    /// Whether it started in solid.
    pub start_solid: bool,
    /// Whether all of it was in solid.
    pub all_solid: bool,
    /// The brush model it hit, or `None` for the world or nothing.
    pub model: Option<u32>,
    /// Whether it passed through open air.
    pub in_open: bool,
    /// Whether it passed through liquid.
    pub in_water: bool,
}

impl Trace {
    /// A trace that got to `end` without touching anything.
    fn clear(end: Vec3) -> Self {
        Self {
            fraction: 1.0,
            end,
            normal: Vec3::ZERO,
            start_solid: false,
            all_solid: false,
            model: None,
            in_open: false,
            in_water: false,
        }
    }
}

/// What fills a point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointContents {
    /// Nothing.
    Empty,
    /// Solid.
    Solid,
    /// A liquid.
    Liquid(Liquid),
}

/// Converts a vector for `box3d`.
fn box_vec(value: Vec3) -> BoxVec3 {
    BoxVec3 {
        x: value.x,
        y: value.y,
        z: value.z,
    }
}

/// One model's solids, in their own `box3d` world.
struct ModelSolids {
    /// The solids, where the map built them.
    world: World,
    /// The root of the player's hull.
    root: ClipChild,
    /// The smallest corner of its bounds.
    mins: Vec3,
    /// The largest corner of its bounds.
    maxs: Vec3,
}

/// Casts a ray, returning the fraction and normal of the first hit.
fn cast(world: &World, start: Vec3, delta: Vec3) -> Option<(f32, Vec3)> {
    let origin = Pos {
        x: start.x,
        y: start.y,
        z: start.z,
    };
    let filter = default_query_filter();
    let hit = world_cast_ray_closest(world, origin, box_vec(delta), &filter);
    hit.hit.then(|| {
        (
            hit.fraction,
            Vec3::new(hit.normal.x, hit.normal.y, hit.normal.z),
        )
    })
}

/// A map's collision for a player-sized box. It never changes: where moving
/// models are comes from [`CollisionWorld::with`], so movement stays a pure
/// function of what it's given (proposal 0026).
pub struct CollisionWorld {
    /// The map, for the hull tree and contents.
    map: Arc<Map>,
    /// Every model's solids. The first is the world.
    models: Vec<Option<ModelSolids>>,
    /// How many solids loaded.
    solid_count: usize,
    /// How many solids `box3d` refused.
    refused_count: usize,
}

/// A map's collision, with its moving models in place.
#[derive(Clone, Copy)]
pub struct Collision<'a> {
    /// The fixed part.
    world: &'a CollisionWorld,
    /// Where models are.
    instances: &'a [ModelInstance],
    /// A model to pass through.
    ignored: Option<u32>,
    /// Solid boxes, as their corners.
    boxes: &'a [(Vec3, Vec3)],
}

impl std::fmt::Debug for CollisionWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CollisionWorld")
            .field("solid_count", &self.solid_count)
            .finish_non_exhaustive()
    }
}

impl CollisionWorld {
    /// The map this was built from.
    #[must_use]
    pub fn map(&self) -> &Arc<Map> {
        &self.map
    }

    /// Builds the collision of every model in `map`.
    #[must_use]
    pub fn new(map: Arc<Map>) -> Self {
        let (mut solid_count, mut refused_count) = (0, 0);
        let models = map
            .models
            .iter()
            .map(|model| {
                let root = model.hulls[0];
                if root == ClipChild::Empty {
                    return None;
                }
                let mut world = World::new(&default_world_def());
                let body = create_body(&mut world, &default_body_def());
                let shape = default_shape_def();
                for solid in build_solids(&map.tree, root, model.mins, model.maxs) {
                    let points: Vec<BoxVec3> = solid.into_iter().map(box_vec).collect();
                    match create_hull(&points, MAX_HULL_VERTICES) {
                        Some(hull) => {
                            let _shape = create_hull_shape(&mut world, body, &shape, &hull);
                            solid_count += 1;
                        }
                        None => refused_count += 1,
                    }
                }
                Some(ModelSolids {
                    world,
                    root,
                    mins: model.mins,
                    maxs: model.maxs,
                })
            })
            .collect();
        Self {
            map,
            models,
            solid_count,
            refused_count,
        }
    }

    /// How many solids were loaded, and how many were refused.
    #[must_use]
    pub fn solid_counts(&self) -> (usize, usize) {
        (self.solid_count, self.refused_count)
    }

    /// This collision with models at `instances`.
    #[must_use]
    pub fn with<'a>(&'a self, instances: &'a [ModelInstance]) -> Collision<'a> {
        Collision {
            world: self,
            instances,
            ignored: None,
            boxes: &[],
        }
    }

    /// Whether a player at `point` would be in solid, with no moving models.
    #[must_use]
    pub fn in_solid(&self, point: Vec3) -> bool {
        self.with(&[]).in_solid(point)
    }

    /// What fills `point`.
    #[must_use]
    pub fn contents(&self, point: Vec3) -> PointContents {
        self.with(&[]).contents(point)
    }

    /// A trace with no moving models.
    #[must_use]
    pub fn trace(&self, start: Vec3, end: Vec3) -> Trace {
        self.with(&[]).trace(start, end)
    }

    /// What the map holds at `point`, with sky told apart from open air.
    #[must_use]
    pub fn map_contents(&self, point: Vec3) -> Contents {
        let tree = &self.map.tree;
        tree.leaf(tree.leaf_at(point))
            .map_or(Contents::Solid, |leaf| leaf.contents)
    }

    /// The solids of model `number`.
    fn model(&self, number: u32) -> Option<&ModelSolids> {
        self.models
            .get(usize::try_from(number).ok()?)
            .and_then(Option::as_ref)
    }
}

impl<'a> Collision<'a> {
    /// The same collision, passing through model `model`.
    #[must_use]
    pub fn ignoring(self, model: u32) -> Self {
        Self {
            ignored: Some(model),
            ..self
        }
    }

    /// The same collision, with solid boxes in the way too.
    #[must_use]
    pub fn with_boxes(self, boxes: &'a [(Vec3, Vec3)]) -> Self {
        Self { boxes, ..self }
    }

    /// Moves a box of any size from `start` towards `end`, through the world
    /// and moving models. The hull comes from its size, as `SV_HullForEntity`
    /// chooses. Entities are a job for whoever knows them.
    #[must_use]
    pub fn move_box(&self, start: Vec3, mins: Vec3, maxs: Vec3, end: Vec3) -> Trace {
        let width = maxs.x - mins.x;
        let (hull, hull_mins) = if width < 3.0 {
            (None, mins)
        } else if width <= 32.0 {
            (Some(0), Vec3::new(-16.0, -16.0, -24.0))
        } else {
            (Some(1), Vec3::new(-32.0, -32.0, -24.0))
        };
        let shift = hull_mins - mins;
        let map = &self.world.map;
        let root_of = |number: u32| {
            let model = map.models.get(usize::try_from(number).ok()?)?;
            match hull {
                None => model.root.map(HullRoot::Point),
                Some(hull) => Some(HullRoot::Clip(model.hulls[hull])),
            }
        };
        let mut best = Trace::clear(end);
        if let Some(root) = root_of(0) {
            let trace = trace_hull(&map.tree, root, start - shift, end - shift);
            best = Trace {
                fraction: trace.fraction,
                end: trace.end + shift,
                normal: trace.normal,
                start_solid: trace.start_solid,
                all_solid: trace.all_solid,
                model: None,
                in_open: trace.in_open,
                in_water: trace.in_water,
            };
        }
        let solid = self.instances.iter().filter(|instance| {
            instance.solid && instance.model != 0 && Some(instance.model) != self.ignored
        });
        for instance in solid {
            let Some(root) = root_of(instance.model) else {
                continue;
            };
            let offset = shift + instance.origin;
            let trace = trace_hull(&map.tree, root, start - offset, end - offset);
            if trace.all_solid || trace.start_solid || trace.fraction < best.fraction {
                best = Trace {
                    fraction: trace.fraction,
                    end: trace.end + offset,
                    normal: trace.normal,
                    start_solid: trace.start_solid || best.start_solid,
                    all_solid: trace.all_solid,
                    model: Some(instance.model),
                    in_open: best.in_open,
                    in_water: best.in_water,
                };
            }
        }
        best
    }

    /// The boxes grown by a player's size, so a player's centre can't enter.
    fn player_boxes(&self) -> impl Iterator<Item = (Vec3, Vec3)> {
        self.boxes
            .iter()
            .map(|(low, high)| (*low - PLAYER_MAXS, *high - PLAYER_MINS))
    }

    /// The moving models that can stop a player.
    fn solid_instances(&self) -> impl Iterator<Item = (&'a ModelInstance, &'a ModelSolids)> {
        let ignored = self.ignored;
        let world = self.world;
        self.instances
            .iter()
            .filter(move |instance| {
                instance.solid && instance.model != 0 && Some(instance.model) != ignored
            })
            .filter_map(move |instance| Some((instance, world.model(instance.model)?)))
    }

    /// Whether a player at `point` would be in solid.
    #[must_use]
    pub fn in_solid(&self, point: Vec3) -> bool {
        let tree = &self.world.map.tree;
        let in_box = self.player_boxes().any(|(low, high)| {
            point.x >= low.x
                && point.y >= low.y
                && point.z >= low.z
                && point.x <= high.x
                && point.y <= high.y
                && point.z <= high.z
        });
        if in_box {
            return true;
        }
        let in_world = self
            .world
            .model(0)
            .is_some_and(|world| tree.clip_solid(world.root, point));
        in_world
            || self
                .solid_instances()
                .any(|(instance, model)| tree.clip_solid(model.root, point - instance.origin))
    }

    /// Whether a player at `point` would be inside model `number`, placed at
    /// `origin`.
    #[must_use]
    pub fn in_model(&self, number: u32, origin: Vec3, point: Vec3) -> bool {
        self.world
            .model(number)
            .is_some_and(|model| self.world.map.tree.clip_solid(model.root, point - origin))
    }

    /// What fills `point`.
    #[must_use]
    pub fn contents(&self, point: Vec3) -> PointContents {
        let tree = &self.world.map.tree;
        let leaf = tree.leaf(tree.leaf_at(point));
        match leaf.map_or(Contents::Solid, |leaf| leaf.contents) {
            Contents::Empty | Contents::Sky => PointContents::Empty,
            Contents::Solid => PointContents::Solid,
            Contents::Water => PointContents::Liquid(Liquid::Water),
            Contents::Slime => PointContents::Liquid(Liquid::Slime),
            Contents::Lava => PointContents::Liquid(Liquid::Lava),
        }
    }

    /// The first hit along a trace, with what it hit.
    fn first_hit(&self, start: Vec3, end: Vec3) -> Option<(f32, Vec3, Option<u32>)> {
        let delta = end - start;
        let mut best = self
            .world
            .model(0)
            .and_then(|world| cast(&world.world, start, delta))
            .map(|(fraction, normal)| (fraction, normal, None));
        for (low, high) in self.player_boxes() {
            if let Some((fraction, normal, false)) = enter_box(start, end, low, high)
                && best.is_none_or(|(nearest, _, _)| fraction < nearest)
            {
                best = Some((fraction, normal, None));
            }
        }
        let margin = Vec3::new(
            MODEL_BOUNDS_MARGIN,
            MODEL_BOUNDS_MARGIN,
            MODEL_BOUNDS_MARGIN,
        );
        for (instance, model) in self.solid_instances() {
            let low = model.mins + instance.origin - margin;
            let high = model.maxs + instance.origin + margin;
            let misses = start.x.max(end.x) < low.x
                || start.y.max(end.y) < low.y
                || start.z.max(end.z) < low.z
                || start.x.min(end.x) > high.x
                || start.y.min(end.y) > high.y
                || start.z.min(end.z) > high.z;
            if misses {
                continue;
            }
            if let Some((fraction, normal)) = cast(&model.world, start - instance.origin, delta)
                && best.is_none_or(|(nearest, _, _)| fraction < nearest)
            {
                best = Some((fraction, normal, Some(instance.model)));
            }
        }
        best
    }

    /// Moves a player's box from `start` towards `end`, stopping short of
    /// anything solid. Rays can't see from inside a solid, so the hull trees
    /// answer that first.
    #[must_use]
    pub fn trace(&self, start: Vec3, end: Vec3) -> Trace {
        if self.in_solid(start) {
            return Trace {
                fraction: 0.0,
                start_solid: true,
                all_solid: self.in_solid(end),
                ..Trace::clear(start)
            };
        }
        let clear = Trace::clear(end);
        let delta = end - start;
        if delta == Vec3::ZERO {
            return clear;
        }
        let Some((hit_fraction, normal, model)) = self.first_hit(start, end) else {
            // A ray ignores a hit at its very start, so it can miss a wall
            // it's touching. The hull trees know better.
            return if self.in_solid(end) {
                self.trace_by_tree(start, delta)
            } else {
                clear
            };
        };
        // Quake's crossing point: DIST_EPSILON on the near side of the plane.
        let at = start + delta * hit_fraction;
        let start_distance = normal.dot(start - at);
        let end_distance = normal.dot(end - at);
        let fraction = if start_distance > end_distance {
            ((start_distance - DIST_EPSILON) / (start_distance - end_distance)).clamp(0.0, 1.0)
        } else {
            hit_fraction
        };
        let fraction = self.back_out(start, delta, fraction);
        Trace {
            fraction,
            normal,
            model,
            ..Trace::clear(start + delta * fraction)
        }
    }

    /// Moves a stopping point back along the trace until it's out of solid,
    /// as the loop at the end of `SV_RecursiveHullCheck` does for the same
    /// rounding errors.
    fn back_out(&self, start: Vec3, delta: Vec3, fraction: f32) -> f32 {
        let step = DIST_EPSILON / delta.length();
        let mut fraction = fraction;
        for _ in 0..MAX_BACK_OUT_STEPS {
            if fraction <= 0.0 || !self.in_solid(start + delta * fraction) {
                return fraction.max(0.0);
            }
            fraction -= step;
        }
        0.0
    }

    /// Finds where a trace enters solid by halving the distance.
    fn trace_by_tree(&self, start: Vec3, delta: Vec3) -> Trace {
        let (mut clear, mut blocked) = (0.0_f32, 1.0_f32);
        for _ in 0..SEARCH_STEPS {
            let middle = (clear + blocked) * 0.5;
            if self.in_solid(start + delta * middle) {
                blocked = middle;
            } else {
                clear = middle;
            }
        }
        Trace {
            fraction: clear,
            normal: (delta * -1.0).normalized(),
            ..Trace::clear(start + delta * clear)
        }
    }
}
