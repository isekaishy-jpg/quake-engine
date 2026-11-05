//! A port of `PlayerMove` from `pmove.c`, with WinQuake's values.

use engine_core::math::{Vec3, view_axes};
use engine_core::player::{
    Liquid, PLAYER_MAXS, PLAYER_MINS, PlayerState, USUAL_GRAVITY, UserCommand, VIEW_HEIGHT,
};
use engine_physics::{Collision, PointContents, Trace};

/// Velocity below this after a clip is stopped, to avoid tiny movements.
const STOP_EPSILON: f32 = 0.1;

/// The most surfaces a single move slides along.
const MAX_CLIP_PLANES: usize = 5;

/// How many times a single move tries again after a hit.
const MAX_BUMPS: usize = 4;

/// The steepest normal that counts as ground.
const MIN_GROUND_NORMAL: f32 = 0.7;

/// Rising faster than this leaves the ground.
const LEAVE_GROUND_SPEED: f32 = 180.0;

/// How far below the feet the edge check looks.
const EDGE_DEPTH: f32 = 34.0;

/// How far ahead the edge check looks.
const EDGE_REACH: f32 = 16.0;

/// How much an edge multiplies friction.
const EDGE_FRICTION: f32 = 2.0;

/// How far a stuck player is nudged, on each axis.
const NUDGE: f32 = 0.125;

/// The rules movement follows, which a server could change.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoveVars {
    /// Units a second squared.
    pub gravity: f32,
    /// Below this speed, friction acts as if at this speed.
    pub stop_speed: f32,
    /// The fastest a player can ask to go.
    pub max_speed: f32,
    /// How quickly the player reaches a wanted speed.
    pub accelerate: f32,
    /// How quickly the player reaches a wanted speed in water.
    pub water_accelerate: f32,
    /// Ground friction.
    pub friction: f32,
    /// Water friction.
    pub water_friction: f32,
    /// The highest step a player walks up.
    pub step_height: f32,
    /// The speed a jump adds.
    pub jump_speed: f32,
    /// The most air acceleration can aim for.
    pub air_wish_limit: f32,
    /// The fastest noclip moves.
    pub noclip_speed: f32,
    /// What share of a wanted speed a swimmer gets.
    pub swim_share: f32,
    /// How fast a swimmer who's doing nothing sinks.
    pub sink_speed: f32,
    /// How fast a player swims up out of water, slime and lava.
    pub surface_speeds: [f32; 3],
    /// How hard climbing out of water pushes forwards and upwards.
    pub climb_out: (f32, f32),
}

impl Default for MoveVars {
    /// The values from `sv_phys.c` and `sv_user.c`.
    fn default() -> Self {
        Self {
            gravity: USUAL_GRAVITY,
            stop_speed: 100.0,
            max_speed: 320.0,
            accelerate: 10.0,
            water_accelerate: 10.0,
            friction: 4.0,
            water_friction: 1.0,
            step_height: 18.0,
            jump_speed: 270.0,
            air_wish_limit: 30.0,
            noclip_speed: 800.0,
            swim_share: 0.7,
            sink_speed: 60.0,
            surface_speeds: [100.0, 80.0, 50.0],
            climb_out: (50.0, 310.0),
        }
    }
}

/// `dot(in, normal)` removed from `velocity`, from `PM_ClipVelocity`.
fn clip_velocity(velocity: Vec3, normal: Vec3) -> Vec3 {
    let clipped = velocity - normal * velocity.dot(normal);
    let settle = |value: f32| {
        if value.abs() < STOP_EPSILON {
            0.0
        } else {
            value
        }
    };
    Vec3::new(settle(clipped.x), settle(clipped.y), settle(clipped.z))
}

/// The direction and length of `value`.
fn direction_and_length(value: Vec3) -> (Vec3, f32) {
    (value.normalized(), value.length())
}

/// One run of [`player_move`].
struct Mover<'a> {
    /// The state being changed.
    state: &'a mut PlayerState,
    /// The input.
    command: &'a UserCommand,
    /// The rules.
    vars: &'a MoveVars,
    /// The map, with its moving models.
    world: Collision<'a>,
    /// Where brush models that were hit are noted.
    touched: &'a mut Vec<u32>,
    /// The view's forward direction.
    forward: Vec3,
    /// The view's right direction.
    right: Vec3,
}

impl Mover<'_> {
    /// Traces from `start` to `end`.
    fn trace(&self, start: Vec3, end: Vec3) -> Trace {
        self.world.trace(start, end)
    }

    /// `PM_FlyMove`: moves along the velocity, sliding along what it hits.
    fn fly_move(&mut self) {
        let primal = self.state.velocity;
        let original = self.state.velocity;
        let mut planes = [Vec3::ZERO; MAX_CLIP_PLANES];
        let mut plane_count = 0;
        let mut time_left = self.command.seconds;
        for _ in 0..MAX_BUMPS {
            let end = self.state.origin + self.state.velocity * time_left;
            let trace = self.trace(self.state.origin, end);
            if trace.start_solid || trace.all_solid {
                self.state.velocity = Vec3::ZERO;
                return;
            }
            if trace.fraction > 0.0 {
                self.state.origin = trace.end;
                plane_count = 0;
            }
            if trace.fraction >= 1.0 {
                break;
            }
            self.touched.extend(trace.model);
            time_left -= time_left * trace.fraction;
            if plane_count >= MAX_CLIP_PLANES {
                self.state.velocity = Vec3::ZERO;
                break;
            }
            planes[plane_count] = trace.normal;
            plane_count += 1;
            let planes = &planes[..plane_count];
            let mut slid = false;
            for (index, plane) in planes.iter().enumerate() {
                let velocity = clip_velocity(original, *plane);
                let fits = planes
                    .iter()
                    .enumerate()
                    .all(|(other, normal)| other == index || velocity.dot(*normal) >= 0.0);
                self.state.velocity = velocity;
                if fits {
                    slid = true;
                    break;
                }
            }
            if !slid {
                if planes.len() != 2 {
                    self.state.velocity = Vec3::ZERO;
                    break;
                }
                let crease = planes[0].cross(planes[1]);
                self.state.velocity = crease * crease.dot(self.state.velocity);
            }
            if self.state.velocity.dot(primal) <= 0.0 {
                self.state.velocity = Vec3::ZERO;
                break;
            }
        }
        if self.state.water_jump_time > 0.0 {
            self.state.velocity = primal;
        }
    }

    /// `PM_GroundMove`: walks, trying a stair step if something's in the way.
    fn ground_move(&mut self) {
        self.state.velocity.z = 0.0;
        if self.state.velocity == Vec3::ZERO {
            return;
        }
        let seconds = self.command.seconds;
        let mut destination = self.state.origin + self.state.velocity * seconds;
        destination.z = self.state.origin.z;
        let trace = self.trace(self.state.origin, destination);
        if trace.fraction >= 1.0 {
            self.state.origin = trace.end;
            return;
        }
        let original = self.state.origin;
        let original_velocity = self.state.velocity;
        self.fly_move();
        let down = self.state.origin;
        let down_velocity = self.state.velocity;
        self.state.origin = original;
        self.state.velocity = original_velocity;

        let raised = original + Vec3::new(0.0, 0.0, self.vars.step_height);
        let trace = self.trace(self.state.origin, raised);
        if !trace.start_solid && !trace.all_solid {
            self.state.origin = trace.end;
        }
        self.fly_move();
        let lowered = self.state.origin - Vec3::new(0.0, 0.0, self.vars.step_height);
        let trace = self.trace(self.state.origin, lowered);
        if trace.normal.z < MIN_GROUND_NORMAL {
            self.state.origin = down;
            self.state.velocity = down_velocity;
            return;
        }
        if !trace.start_solid && !trace.all_solid {
            self.state.origin = trace.end;
        }
        let up = self.state.origin;
        let flat_distance = |point: Vec3| {
            let x = point.x - original.x;
            let y = point.y - original.y;
            x * x + y * y
        };
        if flat_distance(down) > flat_distance(up) {
            self.state.origin = down;
            self.state.velocity = down_velocity;
        } else {
            self.state.velocity.z = down_velocity.z;
        }
    }

    /// `PM_Friction`.
    fn friction(&mut self) {
        if self.state.water_jump_time > 0.0 {
            return;
        }
        let velocity = self.state.velocity;
        let speed = velocity.length();
        if speed < 1.0 {
            self.state.velocity.x = 0.0;
            self.state.velocity.y = 0.0;
            return;
        }
        let mut friction = self.vars.friction;
        if self.state.on_ground {
            let ahead = Vec3::new(velocity.x, velocity.y, 0.0) * (EDGE_REACH / speed);
            let start = self.state.origin + ahead + Vec3::new(0.0, 0.0, PLAYER_MINS.z);
            let stop = start - Vec3::new(0.0, 0.0, EDGE_DEPTH);
            if self.trace(start, stop).fraction >= 1.0 {
                friction *= EDGE_FRICTION;
            }
        }
        let seconds = self.command.seconds;
        let mut drop = 0.0;
        if self.state.water_level >= 2 {
            drop += speed * self.vars.water_friction * f32::from(self.state.water_level) * seconds;
        } else if self.state.on_ground {
            let control = speed.max(self.vars.stop_speed);
            drop += control * friction * seconds;
        }
        let new_speed = (speed - drop).max(0.0);
        self.state.velocity = velocity * (new_speed / speed);
    }

    /// `PM_Accelerate`.
    fn accelerate(&mut self, direction: Vec3, wish_speed: f32, acceleration: f32) {
        if self.state.water_jump_time > 0.0 {
            return;
        }
        let add = wish_speed - self.state.velocity.dot(direction);
        if add <= 0.0 {
            return;
        }
        let gain = (acceleration * self.command.seconds * wish_speed).min(add);
        self.state.velocity = self.state.velocity + direction * gain;
    }

    /// `PM_AirAccelerate`: the aim is capped, but the gain isn't, which is
    /// what makes air strafing work.
    fn air_accelerate(&mut self, direction: Vec3, wish_speed: f32) {
        if self.state.water_jump_time > 0.0 {
            return;
        }
        let add = wish_speed.min(self.vars.air_wish_limit) - self.state.velocity.dot(direction);
        if add <= 0.0 {
            return;
        }
        let gain = (self.vars.accelerate * wish_speed * self.command.seconds).min(add);
        self.state.velocity = self.state.velocity + direction * gain;
    }

    /// `PM_WaterMove`.
    fn water_move(&mut self) {
        let command = self.command;
        let mut wish = self.forward * command.forward + self.right * command.side;
        if command.forward == 0.0 && command.side == 0.0 && command.up == 0.0 {
            wish.z -= self.vars.sink_speed;
        } else {
            wish.z += command.up;
        }
        let (direction, length) = direction_and_length(wish);
        let wish_speed = length.min(self.vars.max_speed) * self.vars.swim_share;
        self.accelerate(direction, wish_speed, self.vars.water_accelerate);

        let destination = self.state.origin + self.state.velocity * command.seconds;
        let start = destination + Vec3::new(0.0, 0.0, self.vars.step_height + 1.0);
        let trace = self.trace(start, destination);
        if !trace.start_solid && !trace.all_solid {
            self.state.origin = trace.end;
            return;
        }
        self.fly_move();
    }

    /// `PM_AirMove`, which also covers walking.
    fn air_move(&mut self) {
        let command = self.command;
        let forward = Vec3::new(self.forward.x, self.forward.y, 0.0).normalized();
        let right = Vec3::new(self.right.x, self.right.y, 0.0).normalized();
        let wish = forward * command.forward + right * command.side;
        let (direction, length) = direction_and_length(wish);
        let wish_speed = length.min(self.vars.max_speed);
        let gravity = self.vars.gravity * command.seconds;
        if self.state.on_ground {
            self.state.velocity.z = 0.0;
            self.accelerate(direction, wish_speed, self.vars.accelerate);
            self.state.velocity.z -= gravity;
            self.ground_move();
        } else {
            self.air_accelerate(direction, wish_speed);
            self.state.velocity.z -= gravity;
            self.fly_move();
        }
    }

    /// Whether `point` is in a liquid, and which.
    fn liquid_at(&self, point: Vec3) -> Liquid {
        match self.world.contents(point) {
            PointContents::Liquid(liquid) => liquid,
            PointContents::Empty | PointContents::Solid => Liquid::None,
        }
    }

    /// `PM_CatagorizePosition`: works out the ground and water level.
    fn categorize(&mut self) {
        // The dead leave the ground on any hop, not having legs to hold them.
        let leaving = if self.state.dead {
            0.0
        } else {
            LEAVE_GROUND_SPEED
        };
        if self.state.velocity.z > leaving {
            self.state.on_ground = false;
            self.state.ground_model = None;
        } else {
            let below = self.state.origin - Vec3::new(0.0, 0.0, 1.0);
            let trace = self.trace(self.state.origin, below);
            self.state.on_ground = trace.normal.z >= MIN_GROUND_NORMAL;
            self.state.ground_model = trace.model.filter(|_| self.state.on_ground);
            if self.state.on_ground {
                self.touched.extend(trace.model);
                self.state.water_jump_time = 0.0;
                if !trace.start_solid && !trace.all_solid {
                    self.state.origin = trace.end;
                }
            }
        }
        let origin = self.state.origin;
        let at = |height: f32| origin + Vec3::new(0.0, 0.0, height);
        self.state.water_level = 0;
        self.state.liquid = self.liquid_at(at(PLAYER_MINS.z + 1.0));
        if self.state.liquid != Liquid::None {
            self.state.water_level = 1;
            if self.liquid_at(at((PLAYER_MINS.z + PLAYER_MAXS.z) * 0.5)) != Liquid::None {
                self.state.water_level = 2;
                if self.liquid_at(at(VIEW_HEIGHT)) != Liquid::None {
                    self.state.water_level = 3;
                }
            }
        }
    }

    /// `JumpButton`.
    fn jump(&mut self) {
        if self.state.water_jump_time > 0.0 {
            self.state.water_jump_time =
                (self.state.water_jump_time - self.command.seconds).max(0.0);
            return;
        }
        if self.state.water_level >= 2 {
            self.state.on_ground = false;
            self.state.velocity.z = match self.state.liquid {
                Liquid::Slime => self.vars.surface_speeds[1],
                Liquid::Lava => self.vars.surface_speeds[2],
                Liquid::Water | Liquid::None => self.vars.surface_speeds[0],
            };
            return;
        }
        if !self.state.on_ground || self.state.jump_held {
            return;
        }
        self.state.on_ground = false;
        self.state.velocity.z += self.vars.jump_speed;
        self.state.jump_held = true;
    }

    /// `CheckWaterJump`: hops out of water at a ledge.
    fn check_water_jump(&mut self) {
        if self.state.water_jump_time > 0.0 || self.state.velocity.z < -LEAVE_GROUND_SPEED {
            return;
        }
        let flat = Vec3::new(self.forward.x, self.forward.y, 0.0).normalized();
        let mut spot = self.state.origin + flat * 24.0;
        spot.z += 8.0;
        if self.world.contents(spot) != PointContents::Solid {
            return;
        }
        spot.z += 24.0;
        if self.world.contents(spot) != PointContents::Empty {
            return;
        }
        let (forwards, upwards) = self.vars.climb_out;
        self.state.velocity = flat * forwards;
        self.state.velocity.z = upwards;
        self.state.water_jump_time = 2.0;
        self.state.jump_held = true;
    }

    /// `NudgePosition`: tries small moves to get out of solid.
    fn nudge(&mut self) {
        const OFFSETS: [f32; 3] = [0.0, -NUDGE, NUDGE];
        let base = self.state.origin;
        for z in OFFSETS {
            for x in OFFSETS {
                for y in OFFSETS {
                    let point = base + Vec3::new(x, y, z);
                    if !self.world.in_solid(point) {
                        self.state.origin = point;
                        return;
                    }
                }
            }
        }
    }

    /// Flies through everything, along the view.
    fn noclip_move(&mut self) {
        let command = self.command;
        let wish = self.forward * command.forward
            + self.right * command.side
            + Vec3::new(0.0, 0.0, command.up);
        let (direction, length) = direction_and_length(wish);
        self.state.velocity = direction * length.min(self.vars.noclip_speed);
        self.state.origin = self.state.origin + self.state.velocity * command.seconds;
        self.state.on_ground = false;
        self.state.water_level = 0;
        self.state.liquid = Liquid::None;
    }

    /// `PlayerMove`.
    fn run(&mut self) {
        if self.state.noclip {
            self.noclip_move();
            return;
        }
        self.nudge();
        self.categorize();
        if self.state.water_level == 2 {
            self.check_water_jump();
        }
        if self.state.velocity.z < 0.0 {
            self.state.water_jump_time = 0.0;
        }
        if self.command.jump {
            self.jump();
        } else {
            self.state.jump_held = false;
        }
        self.friction();
        if self.state.water_level >= 2 {
            self.water_move();
        } else {
            self.air_move();
        }
        self.categorize();
    }
}

/// Moves `state` by `command`, adding any brush models it hit to `touched`.
pub fn player_move(
    state: &mut PlayerState,
    command: &UserCommand,
    vars: &MoveVars,
    world: Collision<'_>,
    touched: &mut Vec<u32>,
) {
    if state.frozen {
        return;
    }
    // The dead lie where they fell.
    let still = UserCommand {
        forward: 0.0,
        side: 0.0,
        up: 0.0,
        jump: false,
        ..*command
    };
    let command = if state.dead { &still } else { command };
    // `SV_AirMove`: fresh from a teleport, nobody can back up.
    let onwards = UserCommand {
        forward: command.forward.max(0.0),
        ..*command
    };
    let held = state.no_backing > 0.0 && state.water_level < 2;
    let command = if held { &onwards } else { command };
    state.no_backing = (state.no_backing - command.seconds).max(0.0);
    let [forward, right, _up] = view_axes(command.yaw, command.pitch);
    Mover {
        state,
        command,
        vars,
        world,
        touched,
        forward,
        right,
    }
    .run();
}
