//! Traces through the map. Only this file knows `box3d`.

use std::sync::Arc;

use box3d_rust::body::create_body;
use box3d_rust::hull::create_hull;
use box3d_rust::shape::create_hull_shape;
use box3d_rust::types::{
    default_body_def, default_query_filter, default_shape_def, default_world_def,
};
use box3d_rust::world::{World, world_cast_ray_closest};
use box3d_rust::{Pos, Vec3 as BoxVec3};
use engine_core::math::Vec3;
use engine_core::player::Liquid;
use engine_map::{ClipChild, Contents, Map};

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

/// A map's collision for a player-sized box.
pub struct CollisionWorld {
    /// The solids.
    world: World,
    /// The map, for the hull tree and contents.
    map: Arc<Map>,
    /// The root of the player's hull.
    root: ClipChild,
    /// How many solids loaded.
    solid_count: usize,
    /// How many solids `box3d` refused.
    refused_count: usize,
}

impl CollisionWorld {
    /// Builds the collision of `map`'s world model.
    #[must_use]
    pub fn new(map: Arc<Map>) -> Self {
        let mut world = World::new(&default_world_def());
        let body = create_body(&mut world, &default_body_def());
        let shape = default_shape_def();
        let model = map.models.first().cloned().unwrap_or_default();
        let root = model.hulls[0];
        let solids = build_solids(&map.tree, root, model.mins, model.maxs);
        let (mut solid_count, mut refused_count) = (0, 0);
        for solid in solids {
            let points: Vec<BoxVec3> = solid.into_iter().map(box_vec).collect();
            match create_hull(&points, MAX_HULL_VERTICES) {
                Some(hull) => {
                    let _shape = create_hull_shape(&mut world, body, &shape, &hull);
                    solid_count += 1;
                }
                None => refused_count += 1,
            }
        }
        Self {
            world,
            map,
            root,
            solid_count,
            refused_count,
        }
    }

    /// How many solids were loaded, and how many were refused.
    #[must_use]
    pub fn solid_counts(&self) -> (usize, usize) {
        (self.solid_count, self.refused_count)
    }

    /// Whether a player at `point` would be in solid.
    #[must_use]
    pub fn in_solid(&self, point: Vec3) -> bool {
        self.map.tree.clip_solid(self.root, point)
    }

    /// What fills `point`.
    #[must_use]
    pub fn contents(&self, point: Vec3) -> PointContents {
        let tree = &self.map.tree;
        let leaf = tree.leaf(tree.leaf_at(point));
        match leaf.map_or(Contents::Solid, |leaf| leaf.contents) {
            Contents::Empty | Contents::Sky => PointContents::Empty,
            Contents::Solid => PointContents::Solid,
            Contents::Water => PointContents::Liquid(Liquid::Water),
            Contents::Slime => PointContents::Liquid(Liquid::Slime),
            Contents::Lava => PointContents::Liquid(Liquid::Lava),
        }
    }

    /// Moves a player's box from `start` towards `end`, stopping short of
    /// anything solid. Rays can't see from inside a solid, so the hull tree
    /// answers that first.
    #[must_use]
    pub fn trace(&self, start: Vec3, end: Vec3) -> Trace {
        if self.in_solid(start) {
            return Trace {
                fraction: 0.0,
                end: start,
                normal: Vec3::ZERO,
                start_solid: true,
                all_solid: self.in_solid(end),
            };
        }
        let clear = Trace {
            fraction: 1.0,
            end,
            normal: Vec3::ZERO,
            start_solid: false,
            all_solid: false,
        };
        let delta = end - start;
        if delta == Vec3::ZERO {
            return clear;
        }
        let origin = Pos {
            x: start.x,
            y: start.y,
            z: start.z,
        };
        let filter = default_query_filter();
        let hit = world_cast_ray_closest(&self.world, origin, box_vec(delta), &filter);
        if !hit.hit {
            // A ray ignores a hit at its very start, so it can miss a wall
            // it's touching. The hull tree knows better.
            return if self.in_solid(end) {
                self.trace_by_tree(start, delta)
            } else {
                clear
            };
        }
        let normal = Vec3::new(hit.normal.x, hit.normal.y, hit.normal.z);
        // Quake's crossing point: DIST_EPSILON on the near side of the plane.
        let at = start + delta * hit.fraction;
        let start_distance = normal.dot(start - at);
        let end_distance = normal.dot(end - at);
        let fraction = if start_distance > end_distance {
            ((start_distance - DIST_EPSILON) / (start_distance - end_distance)).clamp(0.0, 1.0)
        } else {
            hit.fraction
        };
        let fraction = self.back_out(start, delta, fraction);
        Trace {
            fraction,
            end: start + delta * fraction,
            normal,
            start_solid: false,
            all_solid: false,
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
            end: start + delta * clear,
            normal: (delta * -1.0).normalized(),
            start_solid: false,
            all_solid: false,
        }
    }
}
