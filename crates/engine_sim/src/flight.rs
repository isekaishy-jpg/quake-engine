//! What flies: `SV_Physics_Toss` and `SV_PushEntity` in `sv_phys.c`, and tracing
//! against entities, from `SV_Move` in `world.c`.

use std::rc::Rc;

use engine_core::math::Vec3;
use engine_map::Contents;
use engine_physics::{CollisionWorld, PointContents, enter_box};

use crate::simulation::{Callback, Entity, MoveType, Simulation, Solid};
use crate::table::EntityId;

/// `sv_maxvelocity`, on each axis.
pub(crate) const MAX_VELOCITY: f32 = 2000.0;

/// `FL_ONGROUND`.
pub const FLAG_ON_GROUND: u32 = 512;

/// `FL_MONSTER`.
const FLAG_MONSTER: u32 = 32;

/// Below this, part of a velocity stops, from `STOP_EPSILON`.
const STOP_EPSILON: f32 = 0.1;

/// A move's stop on a floor, as a normal's height.
pub(crate) const FLOOR_NORMAL: f32 = 0.7;

/// How fast a bouncing thing has to rise to keep bouncing.
const MIN_BOUNCE: f32 = 60.0;

/// How far short of a box a move stops.
const BOX_EPSILON: f32 = 0.031_25;

/// How a move treats entities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveKind {
    /// It hits everything solid.
    Normal,
    /// It only hits brush models and the world.
    NoMonsters,
    /// It hits everything, and monsters as if they were larger.
    Missile,
}

/// A move's result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoveTrace {
    /// How far it got, from 0 to 1.
    pub fraction: f32,
    /// Where it stopped.
    pub end: Vec3,
    /// The plane it hit.
    pub normal: Vec3,
    /// Whether it began in solid.
    pub start_solid: bool,
    /// Whether it never left solid.
    pub all_solid: bool,
    /// Whether it hit anything, the world included.
    pub hit: bool,
    /// The entity it hit, if it wasn't the world.
    pub entity: Option<EntityId>,
    /// Whether it passed through open air.
    pub in_open: bool,
    /// Whether it passed through liquid.
    pub in_water: bool,
}

/// `ClipVelocity`: a velocity with what goes into a plane taken out of it.
pub(crate) fn clip_velocity(velocity: Vec3, normal: Vec3, overbounce: f32) -> Vec3 {
    let stopped = |part: f32| {
        if part.abs() < STOP_EPSILON { 0.0 } else { part }
    };
    let clipped = velocity - normal * (velocity.dot(normal) * overbounce);
    Vec3::new(stopped(clipped.x), stopped(clipped.y), stopped(clipped.z))
}

impl Simulation {
    /// `SV_Move`: moves a box from `start` towards `end`, stopping at the world
    /// and at anything solid that `pass` doesn't ignore.
    #[must_use]
    pub fn trace_move(
        &self,
        ends: (Vec3, Vec3),
        size: (Vec3, Vec3),
        kind: MoveKind,
        pass: Option<EntityId>,
        world: &CollisionWorld,
    ) -> MoveTrace {
        self.trace_without((ends, size), kind, pass, world, None)
    }

    /// A trace as if brush model `ignored` weren't there.
    pub(crate) fn trace_without(
        &self,
        path: ((Vec3, Vec3), (Vec3, Vec3)),
        kind: MoveKind,
        pass: Option<EntityId>,
        world: &CollisionWorld,
        ignored: Option<u32>,
    ) -> MoveTrace {
        let ((start, end), (mins, maxs)) = path;
        let instances = self.solid_instances(ignored);
        let hull = world.with(&instances).move_box(start, mins, maxs, end);
        let hit = hull.fraction < 1.0 || hull.start_solid;
        let mut best = MoveTrace {
            fraction: hull.fraction,
            end: hull.end,
            normal: hull.normal,
            start_solid: hull.start_solid,
            all_solid: hull.all_solid,
            hit,
            entity: hull.model.and_then(|model| self.entity_of_model(model)),
            in_open: hull.in_open,
            in_water: hull.in_water,
        };
        let owner = pass.and_then(|id| self.entity(id)).and_then(|e| e.owner);
        let pass_has_size = pass
            .and_then(|id| self.entity(id))
            .is_some_and(|entity| entity.maxs.x > entity.mins.x);
        for (id, entity) in self.solid_entities() {
            if Some(id) == pass || Some(id) == owner {
                continue;
            }
            if kind == MoveKind::NoMonsters || entity.owner.is_some() && entity.owner == pass {
                continue;
            }
            if pass_has_size && entity.maxs.x <= entity.mins.x {
                continue;
            }
            let (low, high) = if kind == MoveKind::Missile && entity.flags & FLAG_MONSTER != 0 {
                (Vec3::new(-15.0, -15.0, -15.0), Vec3::new(15.0, 15.0, 15.0))
            } else {
                (mins, maxs)
            };
            let origin = self.origin(id).unwrap_or(entity.origin);
            let box_low = origin + entity.mins - high;
            let box_high = origin + entity.maxs - low;
            let Some((fraction, normal, inside)) = enter_box(start, end, box_low, box_high) else {
                continue;
            };
            if inside {
                best.start_solid = true;
                best.hit = true;
                best.fraction = 0.0;
                best.end = start;
                best.entity = Some(id);
                continue;
            }
            if fraction < best.fraction {
                let length = (end - start).length().max(1.0);
                let stop = (fraction - BOX_EPSILON / length).max(0.0);
                best.fraction = stop;
                best.end = start.lerp(end, stop);
                best.normal = normal;
                best.hit = true;
                best.entity = Some(id);
            }
        }
        best
    }

    /// Traces a line for a script. With no collision, nothing's in the way.
    #[must_use]
    pub fn trace_line(
        &self,
        start: Vec3,
        end: Vec3,
        kind: MoveKind,
        pass: Option<EntityId>,
    ) -> MoveTrace {
        match self.collision() {
            Some(world) => {
                self.trace_move((start, end), (Vec3::ZERO, Vec3::ZERO), kind, pass, world)
            }
            None => MoveTrace {
                fraction: 1.0,
                end,
                normal: Vec3::ZERO,
                start_solid: false,
                all_solid: false,
                hit: false,
                entity: None,
                in_open: false,
                in_water: false,
            },
        }
    }

    /// What the map holds at `point`.
    #[must_use]
    pub fn point_contents(&self, point: Vec3) -> Contents {
        self.collision()
            .map_or(Contents::Empty, |world| world.map_contents(point))
    }

    /// `PF_droptofloor`: moves an entity down onto what's under it, up to 256
    /// units, and returns whether there was anything.
    pub fn drop_to_floor(&mut self, id: EntityId) -> bool {
        let (Some(world), Some(entity)) = (self.collision().map(Rc::clone), self.entity(id)) else {
            return false;
        };
        let start = entity.origin;
        let size = (entity.mins, entity.maxs);
        let end = start - Vec3::new(0.0, 0.0, 256.0);
        let trace = self.trace_move((start, end), size, MoveKind::Normal, Some(id), &world);
        if trace.fraction >= 1.0 || trace.all_solid {
            return false;
        }
        if let Some(entity) = self.entity_mut(id) {
            entity.origin = trace.end;
            entity.flags |= FLAG_ON_GROUND;
            entity.ground = trace.entity;
        }
        true
    }

    /// The entity showing brush model `model`.
    fn entity_of_model(&self, model: u32) -> Option<EntityId> {
        self.brushes()
            .find(|(_, entity)| entity.model == Some(model))
            .map(|(id, _)| id)
    }

    /// The boxes of everything solid but brush models, for what `player` can't
    /// walk through. As in `SV_ClipToLinks`, that leaves out what they own, or
    /// their own rocket would stop them.
    #[must_use]
    pub fn solid_boxes(&self, player: EntityId) -> Vec<(Vec3, Vec3)> {
        self.solid_entities()
            .filter(|(id, entity)| *id != player && entity.owner != Some(player))
            .map(|(id, entity)| {
                let origin = self.origin(id).unwrap_or(entity.origin);
                (origin + entity.mins, origin + entity.maxs)
            })
            .collect()
    }

    /// `SV_Physics_Toss`, after its think.
    pub(crate) fn fly(
        &mut self,
        id: EntityId,
        seconds: f32,
        world: &CollisionWorld,
        callbacks: &mut Vec<Callback>,
    ) {
        let Some(mut entity) = self.entity(id).copied() else {
            return;
        };
        if entity.flags & FLAG_ON_GROUND != 0 {
            return;
        }
        if matches!(entity.move_type, MoveType::Toss | MoveType::Bounce) {
            entity.velocity.z -= self.vars.gravity * seconds;
        }
        let limit = |part: f32| part.clamp(-MAX_VELOCITY, MAX_VELOCITY);
        entity.velocity = Vec3::new(
            limit(entity.velocity.x),
            limit(entity.velocity.y),
            limit(entity.velocity.z),
        );
        entity.angles = entity.angles + entity.angular_velocity * seconds;
        let kind = if entity.move_type == MoveType::FlyMissile {
            MoveKind::Missile
        } else if matches!(entity.solid, Solid::Trigger | Solid::Not) {
            MoveKind::NoMonsters
        } else {
            MoveKind::Normal
        };
        let end = entity.origin + entity.velocity * seconds;
        let trace = self.trace_move(
            (entity.origin, end),
            (entity.mins, entity.maxs),
            kind,
            Some(id),
            world,
        );
        entity.origin = trace.end;
        if trace.hit {
            self.impact(id, &entity, trace.entity, callbacks);
        }
        if trace.fraction < 1.0 {
            let overbounce = if entity.move_type == MoveType::Bounce {
                1.5
            } else {
                1.0
            };
            entity.velocity = clip_velocity(entity.velocity, trace.normal, overbounce);
            let bounces = entity.move_type == MoveType::Bounce;
            if trace.normal.z > FLOOR_NORMAL && (entity.velocity.z < MIN_BOUNCE || !bounces) {
                entity.flags |= FLAG_ON_GROUND;
                entity.velocity = Vec3::ZERO;
                entity.angular_velocity = Vec3::ZERO;
            }
        }
        Self::check_water(&mut entity, id, world, callbacks);
        if let Some(stored) = self.entity_mut(id) {
            *stored = entity;
        }
    }

    /// `SV_Physics_Noclip`, after its think.
    pub(crate) fn drift(&mut self, id: EntityId, seconds: f32) {
        if let Some(entity) = self.entity_mut(id) {
            entity.angles = entity.angles + entity.angular_velocity * seconds;
            entity.origin = entity.origin + entity.velocity * seconds;
        }
    }

    /// `SV_Impact`: both sides of a hit get a touch.
    pub(crate) fn impact(
        &self,
        id: EntityId,
        entity: &Entity,
        other: Option<EntityId>,
        callbacks: &mut Vec<Callback>,
    ) {
        if entity.solid != Solid::Not {
            callbacks.push(Callback::Touch { id, other });
        }
        if let Some(other) = other
            && self
                .entity(other)
                .is_some_and(|entity| entity.solid != Solid::Not)
        {
            callbacks.push(Callback::Touch {
                id: other,
                other: Some(id),
            });
        }
    }

    /// `SV_CheckWaterTransition`, for an entity left where it is.
    pub(crate) fn check_water_of(
        &mut self,
        id: EntityId,
        world: &CollisionWorld,
        callbacks: &mut Vec<Callback>,
    ) {
        if let Some(entity) = self.entity_mut(id) {
            Self::check_water(entity, id, world, callbacks);
        }
    }

    /// `SV_CheckWaterTransition`.
    fn check_water(
        entity: &mut Entity,
        id: EntityId,
        world: &CollisionWorld,
        callbacks: &mut Vec<Callback>,
    ) {
        let wet = matches!(world.contents(entity.origin), PointContents::Liquid(_));
        if entity.water_known && wet != entity.in_water {
            callbacks.push(Callback::Splash(id));
        }
        entity.water_known = true;
        entity.in_water = wet;
    }
}
