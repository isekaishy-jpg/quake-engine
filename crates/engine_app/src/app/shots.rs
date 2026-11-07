//! Firing ahead of the server (proposals 0052 and 0053).

use engine_core::math::view_axes;
use fixture_quake::{WeaponRule, shot_launch, weapon_rule};

use crate::game_screen::Intermission;

use super::World;

impl World {
    /// The rule of a shot that can go now, moving the time of the next one on.
    /// The second part says whether this carries on from the last shot.
    pub(super) fn next_shot(&mut self) -> Option<(WeaponRule, bool)> {
        let rule = self.weapon_ready()?;
        if self.server.is_some() {
            return None;
        }
        let carrying_on = self.shot_clock.take(self.map_time, rule.refire)?;
        Some((rule, carrying_on))
    }

    /// The rule of the weapon in hand, if the player could fire it.
    pub(super) fn weapon_ready(&self) -> Option<WeaponRule> {
        let ammo = self.game_screen.ammo();
        let rule = weapon_rule(self.game_screen.active_weapon(), ammo)?;
        // A click from the dead is a wish to come back, not a shot.
        let idle =
            self.game_screen.is_dead() || self.game_screen.intermission() != Intermission::None;
        (ammo >= rule.ammo && !idle).then_some(rule)
    }

    /// Shows a shot of the player's before the server answers. Nothing here
    /// decides a result (proposals 0052 and 0057).
    #[expect(
        clippy::cast_possible_truncation,
        reason = "f32 keeps angles to a few thousandths of a degree"
    )]
    pub(super) fn guess_shot(&mut self, rule: &WeaponRule, carrying_on: bool, seed: u16) {
        let (yaw, pitch) = (self.look.yaw() as f32, self.look.pitch() as f32);
        let axes = view_axes(yaw, pitch);
        let sounds = !(rule.sound_once && carrying_on);
        if let Some(client) = &mut self.client {
            if let Some(shot) = &rule.shot {
                let origin = client.predicted().origin;
                let (start, velocity) = shot_launch(shot, origin, axes, pitch, seed);
                let kind = (shot.bounces, shot.blasts, shot.fuse);
                client.guess_launch(shot.model, start, velocity, kind);
            }
            client.show_shot(rule.feel);
            if sounds {
                client.expect_own_sound(rule.sound);
            }
        }
        if sounds {
            self.play_local(rule.sound);
        }
    }

    /// Keeps a held weapon going. What goes round on its own needs no more
    /// shots sent, and the rest go out as a click's would, with their seeds.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "f32 keeps angles to a few thousandths of a degree"
    )]
    pub(super) fn hold_fire(&mut self) {
        let Some((rule, carrying_on)) = self.next_shot() else {
            return;
        };
        let (yaw, pitch) = (self.look.yaw() as f32, self.look.pitch() as f32);
        let seed = match &mut self.client {
            Some(client) if !rule.feel.repeats => client.send_fire(yaw, pitch, 0),
            _ => 0,
        };
        self.guess_shot(&rule, carrying_on, seed);
        if self.showing_boxes {
            self.show_seen_boxes();
        }
    }

    /// Shows the other players' boxes as they are on this screen.
    pub(super) fn show_seen_boxes(&mut self) {
        let Some(client) = &mut self.client else {
            return;
        };
        let players: Vec<u64> = client.scores().iter().map(|score| score.entity).collect();
        let drawn = client.draw_entities_buffered(0.0);
        for entity in drawn {
            if players.contains(&entity.state.id) {
                self.effects.show_seen_box(entity.state.origin);
            }
        }
    }
}
