//! What keys do.

use engine_client::canonical_key;

use super::Engine;
use crate::screen::MenuActions;

impl Engine {
    /// Takes a key, which goes to what's on top, as `Key_Event`.
    pub(super) fn key(&mut self, name: &str, pressed: bool, repeat: bool) {
        let key = canonical_key(name);
        if key == "SHIFT" {
            self.world.console_view.set_shift(pressed);
        }
        if !pressed {
            // Key ups always go to the game, so nothing stays held.
            if let Some(command) = self.world.bindings.release(&key) {
                self.console.add(&command);
            }
            return;
        }
        if self.world.script_menu {
            self.world.menu_key_to_script(&key);
        } else if self.world.menu.is_open() {
            let text = self.world.console_view.typed(&key);
            let mut menu = std::mem::take(&mut self.world.menu);
            menu.key(
                &key,
                text,
                &mut MenuActions {
                    console: &mut self.console,
                    world: &mut self.world,
                },
            );
            self.world.menu = menu;
        } else if self.world.console_view.is_open() {
            self.console_key(&key);
        } else if self.world.console_view.is_chatting() {
            if let Some(line) = self.world.console_view.chat_key(&key) {
                self.console.add(&line);
            }
        } else if key == "ESCAPE" {
            if !repeat {
                self.console.add("togglemenu");
            }
        } else if !repeat && let Some(command) = self.world.bindings.get(&key) {
            let command = command.to_owned();
            if command == "+attack" {
                self.fire_now();
            }
            self.console.add(&command);
        }
    }

    /// Sends a shot now, without waiting for the console or the next tick.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "f32 keeps angles well, and a share of 256 fits a byte"
    )]
    pub(super) fn fire_now(&mut self) {
        if !self.world.is_running() || !self.world.is_playing_screen() {
            return;
        }
        let offset = (self.clock.fraction() * 256.0) as u8;
        let (yaw, pitch) = (self.world.look.yaw() as f32, self.world.look.pitch() as f32);
        if let Some(client) = &mut self.world.client {
            let seed = client.send_fire(yaw, pitch, offset);
            if let Some((rule, carrying_on)) = self.world.next_shot() {
                self.world.guess_shot(&rule, carrying_on, seed);
            }
        }
        if self.world.showing_boxes {
            self.world.show_seen_boxes();
        }
    }

    /// Runs the half tick, once in each tick, when it's due.
    pub(super) fn run_half_tick(&mut self) {
        if !self.half_tick_done && self.clock.fraction() >= 0.5 {
            self.half_tick_done = true;
            if self.world.is_running()
                && let Some(server) = &mut self.world.server
            {
                server.half_tick();
            }
        }
    }

    /// Takes a key while the console is open.
    pub(super) fn console_key(&mut self, key: &str) {
        match key {
            "ESCAPE" => self.console.add("togglemenu"),
            "`" | "~" => self.console.add("toggleconsole"),
            "TAB" => self.complete(),
            "MWHEELUP" => {
                let _nothing = self.world.console_view.key("PGUP");
            }
            "MWHEELDOWN" => {
                let _nothing = self.world.console_view.key("PGDN");
            }
            _ => {
                if let Some(line) = self.world.console_view.key(key) {
                    self.console.add(&line);
                }
            }
        }
    }

    /// Completes what's being typed, as far as every match agrees.
    pub(super) fn complete(&mut self) {
        let typed = self.world.console_view.input().trim_start().to_owned();
        if typed.is_empty() || typed.contains(' ') {
            return;
        }
        let matches = self.console.completions(&typed);
        let Some(first) = matches.first() else {
            return;
        };
        let shared = matches.iter().fold(first.len(), |shared, name| {
            first
                .bytes()
                .zip(name.bytes())
                .take(shared)
                .take_while(|(a, b)| a == b)
                .count()
        });
        let mut completed = first[..shared].to_owned();
        if let [_one] = matches.as_slice() {
            completed.push(' ');
        } else {
            for name in &matches {
                self.world
                    .console_view
                    .print(&format!("  {name}"), f64::NEG_INFINITY);
            }
        }
        self.world.console_view.set_input(&completed);
    }
}
