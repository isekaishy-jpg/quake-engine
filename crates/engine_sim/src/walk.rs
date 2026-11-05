//! How monsters get about, from `sv_move.c` and `SV_Physics_Step`.

use std::rc::Rc;

use engine_core::math::Vec3;
use engine_core::player::VIEW_HEIGHT;
use engine_map::Contents;
use engine_physics::CollisionWorld;

use crate::flight::{
    FLAG_ON_GROUND, FLOOR_NORMAL, MAX_VELOCITY, MoveKind, MoveTrace, clip_velocity,
};
use crate::simulation::{Callback, Entity, Simulation, Solid};
use crate::table::EntityId;

/// The tallest step a monster climbs.
const STEP_SIZE: f32 = 18.0;

/// The flag of something that flies.
pub const FLAG_FLY: u32 = 1;

/// The flag of something that swims.
pub const FLAG_SWIM: u32 = 2;

/// The flag of something whose floor went away from under part of it.
pub const FLAG_PARTIAL_GROUND: u32 = 1024;

/// The most surfaces a falling monster slides along in one move.
const MAX_BUMPS: usize = 4;

/// How often monsters take a new look at where the player can be seen from.
const CHECK_SECONDS: f64 = 0.1;

/// A speed fast enough to land with a thud, as a fraction of gravity.
const HARD_LANDING: f32 = -0.1;

/// What a monster is after, which it steers by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Target {
    /// Where it's heading.
    pub goal: Option<EntityId>,
    /// Who it's angry with.
    pub enemy: Option<EntityId>,
}

/// An angle from 0 up to 360.
fn angle_mod(angle: f32) -> f32 {
    angle.rem_euclid(360.0)
}

/// The world and the box of an entity, for tracing.
struct Walker {
    /// The collision.
    world: Rc<CollisionWorld>,
    /// The entity.
    entity: Entity,
}

impl Simulation {
    /// A number from 0 to 32767, as C's `rand`.
    fn rand(&mut self) -> u32 {
        self.seed = self.seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        (self.seed >> 16) & 0x7fff
    }

    /// The world, and a copy of entity `id`.
    fn walker(&self, id: EntityId) -> Option<Walker> {
        Some(Walker {
            world: Rc::clone(self.collision()?),
            entity: *self.entity(id)?,
        })
    }

    /// Traces `id`'s box.
    fn trace_box(&self, walker: &Walker, id: EntityId, from: Vec3, to: Vec3) -> MoveTrace {
        let size = (walker.entity.mins, walker.entity.maxs);
        self.trace_move((from, to), size, MoveKind::Normal, Some(id), &walker.world)
    }

    /// Traces a point down past everything but the world and brush models.
    fn trace_down(&self, walker: &Walker, id: EntityId, from: Vec3, to: Vec3) -> MoveTrace {
        let size = (Vec3::ZERO, Vec3::ZERO);
        let kind = MoveKind::NoMonsters;
        self.trace_move((from, to), size, kind, Some(id), &walker.world)
    }

    /// `SV_CheckBottom`: whether every part of the bottom of `id` is on
    /// something, or on a stair.
    #[must_use]
    pub fn check_bottom(&self, id: EntityId) -> bool {
        let Some(walker) = self.walker(id) else {
            return false;
        };
        let low = walker.entity.origin + walker.entity.mins;
        let high = walker.entity.origin + walker.entity.maxs;
        let corners = [
            (low.x, low.y),
            (low.x, high.y),
            (high.x, low.y),
            (high.x, high.y),
        ];
        let all_solid = corners
            .iter()
            .all(|&(x, y)| self.point_contents(Vec3::new(x, y, low.z - 1.0)) == Contents::Solid);
        if all_solid {
            return true;
        }
        let middle = ((low.x + high.x) * 0.5, (low.y + high.y) * 0.5);
        let below = low.z - 2.0 * STEP_SIZE;
        let trace = self.trace_down(
            &walker,
            id,
            Vec3::new(middle.0, middle.1, low.z),
            Vec3::new(middle.0, middle.1, below),
        );
        if trace.fraction >= 1.0 {
            return false;
        }
        let mid = trace.end.z;
        corners.iter().all(|&(x, y)| {
            let trace =
                self.trace_down(&walker, id, Vec3::new(x, y, low.z), Vec3::new(x, y, below));
            trace.fraction < 1.0 && mid - trace.end.z <= STEP_SIZE
        })
    }

    /// The moves of something that flies or swims.
    fn fly_step(&mut self, id: EntityId, step: Vec3, enemy: Option<EntityId>) -> bool {
        let Some(walker) = self.walker(id) else {
            return false;
        };
        let entity = walker.entity;
        let enemy_origin = enemy.and_then(|enemy| self.origin(enemy));
        // Try one move with vertical motion, then one without.
        for attempt in 0..2 {
            let mut target = entity.origin + step;
            if let Some(origin) = enemy_origin.filter(|_| attempt == 0) {
                let above = entity.origin.z - origin.z;
                if above > 40.0 {
                    target.z -= 8.0;
                }
                if above < 30.0 {
                    target.z += 8.0;
                }
            }
            let trace = self.trace_box(&walker, id, entity.origin, target);
            if trace.fraction >= 1.0 {
                let swims = entity.flags & FLAG_SWIM != 0;
                if swims && self.point_contents(trace.end) == Contents::Empty {
                    return false;
                }
                if let Some(stored) = self.entity_mut(id) {
                    stored.origin = trace.end;
                }
                return true;
            }
            if enemy_origin.is_none() {
                break;
            }
        }
        false
    }

    /// `SV_movestep`: moves `id` by `step`, over slopes and stairs. If it can't,
    /// nothing moves and this returns false.
    fn move_step(&mut self, id: EntityId, step: Vec3, enemy: Option<EntityId>) -> bool {
        let Some(walker) = self.walker(id) else {
            return false;
        };
        let entity = walker.entity;
        if entity.flags & (FLAG_FLY | FLAG_SWIM) != 0 {
            return self.fly_step(id, step, enemy);
        }
        let up = Vec3::new(0.0, 0.0, STEP_SIZE);
        let mut start = entity.origin + step + up;
        let end = start - up - up;
        let mut trace = self.trace_box(&walker, id, start, end);
        if trace.all_solid {
            return false;
        }
        if trace.start_solid {
            start = start - up;
            trace = self.trace_box(&walker, id, start, end);
            if trace.all_solid || trace.start_solid {
                return false;
            }
        }
        let partial = entity.flags & FLAG_PARTIAL_GROUND != 0;
        if trace.fraction >= 1.0 {
            // With the ground pulled out, it may as well fall.
            if partial && let Some(stored) = self.entity_mut(id) {
                stored.origin = entity.origin + step;
                stored.flags &= !FLAG_ON_GROUND;
                return true;
            }
            return false;
        }
        if let Some(stored) = self.entity_mut(id) {
            stored.origin = trace.end;
        }
        if !self.check_bottom(id) {
            if partial {
                return true;
            }
            if let Some(stored) = self.entity_mut(id) {
                stored.origin = entity.origin;
            }
            return false;
        }
        if let Some(stored) = self.entity_mut(id) {
            stored.flags &= !FLAG_PARTIAL_GROUND;
            stored.ground = trace.entity;
        }
        true
    }

    /// `PF_walkmove`: moves `id` by `distance` along `yaw`, all or nothing.
    pub fn walk_move(
        &mut self,
        id: EntityId,
        yaw: f32,
        distance: f32,
        enemy: Option<EntityId>,
    ) -> bool {
        let can_move = self
            .entity(id)
            .is_some_and(|entity| entity.flags & (FLAG_ON_GROUND | FLAG_FLY | FLAG_SWIM) != 0);
        if !can_move {
            return false;
        }
        let (sin, cos) = yaw.to_radians().sin_cos();
        self.move_step(id, Vec3::new(cos * distance, sin * distance, 0.0), enemy)
    }

    /// `PF_changeyaw`: turns `id` towards its ideal yaw at its turning speed.
    pub fn change_yaw(&mut self, id: EntityId) {
        let Some(entity) = self.entity_mut(id) else {
            return;
        };
        let current = angle_mod(entity.angles.y);
        let ideal = entity.ideal_yaw;
        if (current - ideal).abs() < f32::EPSILON {
            return;
        }
        let mut turn = ideal - current;
        if ideal > current {
            if turn >= 180.0 {
                turn -= 360.0;
            }
        } else if turn <= -180.0 {
            turn += 360.0;
        }
        let speed = entity.yaw_speed;
        entity.angles.y = angle_mod(current + turn.clamp(-speed, speed));
    }

    /// `SV_StepDirection`: turns to `yaw`, and steps if facing that way.
    fn step_direction(
        &mut self,
        id: EntityId,
        yaw: f32,
        distance: f32,
        enemy: Option<EntityId>,
    ) -> bool {
        if let Some(entity) = self.entity_mut(id) {
            entity.ideal_yaw = yaw;
        }
        self.change_yaw(id);
        let Some(before) = self.entity(id).map(|entity| entity.origin) else {
            return false;
        };
        let (sin, cos) = yaw.to_radians().sin_cos();
        let step = Vec3::new(cos * distance, sin * distance, 0.0);
        if !self.move_step(id, step, enemy) {
            return false;
        }
        if let Some(entity) = self.entity_mut(id) {
            let delta = entity.angles.y - entity.ideal_yaw;
            if delta > 45.0 && delta < 315.0 {
                // Not turned far enough, so don't take the step.
                entity.origin = before;
            }
        }
        true
    }

    /// `SV_NewChaseDir`: picks a way towards `goal`, going round what's in the
    /// way.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a direction is a small whole number of degrees"
    )]
    #[expect(
        clippy::cast_precision_loss,
        reason = "a direction is a small whole number of degrees"
    )]
    fn new_chase_dir(&mut self, id: EntityId, target: Target, distance: f32) {
        let Some(entity) = self.entity(id).copied() else {
            return;
        };
        let enemy = target.enemy;
        let goal = target
            .goal
            .and_then(|goal| self.origin(goal))
            .unwrap_or(Vec3::ZERO);
        let old = angle_mod(((entity.ideal_yaw / 45.0) as i32 * 45) as f32);
        let back = angle_mod(old - 180.0);
        let (delta_x, delta_y) = (goal.x - entity.origin.x, goal.y - entity.origin.y);
        let mut first = if delta_x > 10.0 {
            Some(0.0)
        } else if delta_x < -10.0 {
            Some(180.0)
        } else {
            None
        };
        let mut second = if delta_y < -10.0 {
            Some(270.0)
        } else if delta_y > 10.0 {
            Some(90.0)
        } else {
            None
        };
        // Try the direct route.
        if let (Some(x), Some(y)) = (first, second) {
            let direct = match (x == 0.0, y == 90.0) {
                (true, true) => 45.0,
                (true, false) => 315.0,
                (false, true) => 135.0,
                (false, false) => 215.0,
            };
            if direct != back && self.step_direction(id, direct, distance, enemy) {
                return;
            }
        }
        // Try the other directions.
        if self.rand() & 1 != 0 || delta_y.abs() > delta_x.abs() {
            std::mem::swap(&mut first, &mut second);
        }
        for direction in [first, second].into_iter().flatten() {
            if direction != back && self.step_direction(id, direction, distance, enemy) {
                return;
            }
        }
        // There's no direct path, so pick another direction.
        if self.step_direction(id, old, distance, enemy) {
            return;
        }
        let forwards = self.rand() & 1 != 0;
        for step in 0..8_u8 {
            let index = if forwards { step } else { 7 - step };
            let direction = f32::from(index) * 45.0;
            if direction != back && self.step_direction(id, direction, distance, enemy) {
                return;
            }
        }
        if self.step_direction(id, back, distance, enemy) {
            return;
        }
        if let Some(entity) = self.entity_mut(id) {
            entity.ideal_yaw = old;
        }
        // A bridge may have gone from underneath it.
        if !self.check_bottom(id)
            && let Some(entity) = self.entity_mut(id)
        {
            entity.flags |= FLAG_PARTIAL_GROUND;
        }
    }

    /// `SV_CloseEnough`: whether `id`'s box is within `distance` of `goal`'s.
    fn close_enough(&self, id: EntityId, goal: Option<EntityId>, distance: f32) -> bool {
        let Some(entity) = self.entity(id) else {
            return false;
        };
        let (goal_low, goal_high) = goal
            .and_then(|goal| Some((self.origin(goal)?, self.entity(goal)?)))
            .map_or((Vec3::ZERO, Vec3::ZERO), |(origin, goal)| {
                (origin + goal.mins, origin + goal.maxs)
            });
        let low = entity.origin + entity.mins;
        let high = entity.origin + entity.maxs;
        let apart = |goal_low: f32, goal_high: f32, low: f32, high: f32| {
            goal_low > high + distance || goal_high < low - distance
        };
        !(apart(goal_low.x, goal_high.x, low.x, high.x)
            || apart(goal_low.y, goal_high.y, low.y, high.y)
            || apart(goal_low.z, goal_high.z, low.z, high.z))
    }

    /// `SV_MoveToGoal`: moves `id` towards its goal, by `distance`.
    pub fn move_to_goal(&mut self, id: EntityId, distance: f32, target: Target) {
        let Some(entity) = self.entity(id).copied() else {
            return;
        };
        if entity.flags & (FLAG_ON_GROUND | FLAG_FLY | FLAG_SWIM) == 0 {
            return;
        }
        // If the next step hits the enemy, it's there.
        if target.enemy.is_some() && self.close_enough(id, target.goal, distance) {
            return;
        }
        if self.rand() & 3 == 1
            || !self.step_direction(id, entity.ideal_yaw, distance, target.enemy)
        {
            self.new_chase_dir(id, target, distance);
        }
    }

    /// `PF_checkclient`: the player something at `id`, with eyes `view_offset`
    /// above its origin, might see. Monsters look for one player at a time, and
    /// move on to the next a few times a second, as `PF_newcheckclient` does.
    pub fn player_in_view(&mut self, id: EntityId, view_offset: Vec3) -> Option<EntityId> {
        let map = std::sync::Arc::clone(self.map());
        let tree = &map.tree;
        let ids = self.player_ids();
        if self.time() - self.last_check >= CHECK_SECONDS || self.player_view.is_empty() {
            self.last_check = self.time();
            let alive = |sim: &Self, index: usize| {
                sim.player_of(ids[index % ids.len()])
                    .is_some_and(|state| !state.dead)
            };
            let start = self.check_index + 1;
            let next = (start..start + ids.len())
                .find(|&index| alive(self, index))
                .unwrap_or(start);
            self.check_index = next % ids.len();
            let target = self
                .player_of(ids[self.check_index])
                .map_or(Vec3::ZERO, |state| state.origin);
            let eyes = target + Vec3::new(0.0, 0.0, VIEW_HEIGHT);
            let mut visible = vec![false; tree.leaves.len()];
            tree.mark_visible(tree.leaf_at(eyes), &mut visible);
            self.player_view = visible;
        }
        let entity = self.entity(id)?;
        let leaf = tree.leaf_at(entity.origin + view_offset);
        let seen = leaf != 0
            && usize::try_from(leaf)
                .ok()
                .and_then(|leaf| self.player_view.get(leaf))
                .copied()
                .unwrap_or(false);
        seen.then(|| ids.get(self.check_index).copied()).flatten()
    }

    /// `SV_FlyMove`: moves a falling thing, sliding along what it hits.
    fn fly_move(
        &mut self,
        id: EntityId,
        seconds: f32,
        world: &CollisionWorld,
        callbacks: &mut Vec<Callback>,
    ) {
        let Some(mut entity) = self.entity(id).copied() else {
            return;
        };
        let primal = entity.velocity;
        let mut original = entity.velocity;
        let mut planes: Vec<Vec3> = Vec::with_capacity(MAX_BUMPS + 1);
        let mut time_left = seconds;
        for _ in 0..MAX_BUMPS {
            if entity.velocity == Vec3::ZERO {
                break;
            }
            let end = entity.origin + entity.velocity * time_left;
            let size = (entity.mins, entity.maxs);
            let trace = self.trace_move(
                (entity.origin, end),
                size,
                MoveKind::Normal,
                Some(id),
                world,
            );
            if trace.all_solid {
                entity.velocity = Vec3::ZERO;
                break;
            }
            if trace.fraction > 0.0 {
                entity.origin = trace.end;
                original = entity.velocity;
                planes.clear();
            }
            if trace.fraction >= 1.0 {
                break;
            }
            if trace.normal.z > FLOOR_NORMAL {
                let on_brush = trace
                    .entity
                    .is_none_or(|hit| self.entity(hit).is_some_and(|hit| hit.solid == Solid::Bsp));
                if on_brush {
                    entity.flags |= FLAG_ON_GROUND;
                    entity.ground = trace.entity;
                }
            }
            self.impact(id, &entity, trace.entity, callbacks);
            time_left -= time_left * trace.fraction;
            planes.push(trace.normal);
            let along = planes.iter().find_map(|plane| {
                let new = clip_velocity(original, *plane, 1.0);
                planes
                    .iter()
                    .all(|other| other == plane || new.dot(*other) >= 0.0)
                    .then_some(new)
            });
            if let Some(along) = along {
                entity.velocity = along;
            } else if let [first, second] = planes[..] {
                let crease = first.cross(second);
                entity.velocity = crease * crease.dot(entity.velocity);
            } else {
                entity.velocity = Vec3::ZERO;
                break;
            }
            // Stop dead rather than shake in a sloping corner.
            if entity.velocity.dot(primal) <= 0.0 {
                entity.velocity = Vec3::ZERO;
                break;
            }
        }
        if let Some(stored) = self.entity_mut(id) {
            *stored = entity;
        }
    }

    /// `SV_Physics_Step`, before its think: only what's in the air moves.
    pub(crate) fn fall(
        &mut self,
        id: EntityId,
        seconds: f32,
        world: &CollisionWorld,
        callbacks: &mut Vec<Callback>,
    ) {
        let gravity = self.vars.gravity;
        let Some(entity) = self.entity_mut(id) else {
            return;
        };
        if entity.flags & (FLAG_ON_GROUND | FLAG_FLY | FLAG_SWIM) != 0 {
            return;
        }
        let hard = entity.velocity.z < gravity * HARD_LANDING;
        entity.velocity.z -= gravity * seconds;
        let limit = |part: f32| part.clamp(-MAX_VELOCITY, MAX_VELOCITY);
        entity.velocity = Vec3::new(
            limit(entity.velocity.x),
            limit(entity.velocity.y),
            limit(entity.velocity.z),
        );
        self.fly_move(id, seconds, world, callbacks);
        let landed = self
            .entity(id)
            .is_some_and(|entity| entity.flags & FLAG_ON_GROUND != 0);
        if landed && hard {
            callbacks.push(Callback::Land(id));
        }
    }
}
