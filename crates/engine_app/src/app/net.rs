//! Connections, bots and following the server between levels.

use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use engine_client::{Bot, BotSkill, Client, Effects, StepSmoother};
use engine_console::Console;
use engine_core::math::Vec3;
use engine_net::{Conditioned, Conditions, ServerMessage, Transport, loopback};
use engine_physics::CollisionWorld;
use engine_server::{Level, Seat, SeatInfo};
use fixture_quake::{LoadedLevel, load_level};

use super::{CONNECT_SECONDS, World, player_client};

impl World {
    /// Adds a bot on a connection of `ping` milliseconds. Returns its name.
    pub(crate) fn add_bot(&mut self, ping: u64) -> Option<String> {
        let world = Rc::clone(self.collision.as_ref()?);
        let server = self.server.as_mut()?;
        self.bots_made += 1;
        let number = self.bots_made;
        let conditions = Conditions {
            send_delay: Duration::from_millis(ping / 2),
            ..Conditions::default()
        };
        let (bot_end, server_end) = loopback();
        let seed = u64::from(number) * 7919;
        let name = format!("bot{number}");
        let colors = u8::try_from(number % 14).unwrap_or(0);
        let server_end = Conditioned::new(Box::new(server_end), conditions, seed);
        server.add_seat(Seat::new(
            Box::new(server_end),
            SeatInfo {
                name: name.clone(),
                colors: colors << 4 | (13 - colors),
            },
        ));
        let bot_end = Conditioned::new(Box::new(bot_end), conditions, seed + 1);
        let mut client = Client::new(Box::new(bot_end), world, Vec3::ZERO);
        client.send_ready();
        self.bots
            .push(Bot::new(client, BotSkill::default(), number));
        Some(name)
    }

    /// Plays a tick of every bot, following the server to a new level.
    pub(super) fn run_bots(&mut self, seconds: f32) {
        let Some(world) = self.collision.clone() else {
            return;
        };
        for bot in std::mem::take(&mut self.bots) {
            let mut bot = bot;
            bot.tick(seconds, &world);
            if bot.client_mut().take_level().is_some() {
                let (transport, skill, seed) = bot.into_parts();
                let mut client = Client::new(transport, Rc::clone(&world), Vec3::ZERO);
                client.send_ready();
                bot = Bot::new(client, skill, seed);
            }
            self.bots.push(bot);
        }
    }

    /// Starts connecting to a server, leaving any game going.
    pub(crate) fn begin_connection(&mut self, transport: Box<dyn Transport>) {
        self.leave_game();
        self.connecting = Some(transport);
        self.connect_time = 0.0;
    }

    /// Leaves whatever game is going.
    pub(crate) fn leave_game(&mut self) {
        if let Some(net_server) = &mut self.net_server {
            net_server.close();
            net_server.flush();
        }
        self.net_server = None;
        self.bots.clear();
        self.console_view.cancel_chat();
        self.collision = None;
        self.connecting = None;
        self.client = None;
        self.server = None;
        self.map = None;
        self.level = Level::default();
        self.level_pending = None;
        self.audio.stop_sounds();
        self.effects = Effects::default();
    }

    /// Loads the map a server named, and tells it so.
    pub(super) fn join_level(
        &mut self,
        name: &str,
        transport: Box<dyn Transport>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let LoadedLevel {
            map,
            palette,
            start,
        } = load_level(&self.filesystem, name)?;
        let collision = Rc::new(CollisionWorld::new(Arc::clone(&map)));
        let mut client = player_client(transport, collision, start.origin);
        client.send_info(&self.identity.name, self.identity.colors);
        client.send_ready();
        client.flush_network();
        self.client = Some(client);
        self.step = StepSmoother::default();
        self.after_load(&map, palette)
    }

    /// Hears from the network, and lets anyone who's arrived in.
    pub(super) fn update_network(&mut self, seconds: f64, console: &mut Console<Self>) {
        let elapsed = Duration::from_secs_f64(seconds);
        if let Some(net_server) = &mut self.net_server {
            net_server.update(elapsed);
            while let Some(transport) = net_server.take_arrival() {
                if let Some(server) = &mut self.server {
                    server.add_seat(Seat::new(transport, SeatInfo::default()));
                }
            }
            if net_server.take_identity_changed() {
                self.keys.host = net_server.identity();
                self.keys.save_host();
            }
        }
        if let Some(client) = &mut self.client {
            client.update_network(elapsed);
        }
        self.update_connecting(seconds, elapsed, console);
        let lost = self.server.is_none()
            && self
                .client
                .as_ref()
                .is_some_and(|client| !client.is_connected());
        if lost {
            console.print("the server has gone");
            self.leave_game();
        }
    }

    /// Waits for a connection to name its level.
    pub(super) fn update_connecting(
        &mut self,
        seconds: f64,
        elapsed: Duration,
        console: &mut Console<Self>,
    ) {
        let Some(mut transport) = self.connecting.take() else {
            return;
        };
        transport.update(elapsed);
        transport.flush();
        self.connect_time += seconds;
        let level =
            std::iter::from_fn(|| transport.receive()).find_map(
                |bytes| match ServerMessage::decode(&bytes) {
                    Some(ServerMessage::Level(name)) => Some(name),
                    _ => None,
                },
            );
        if let Some(level) = level {
            if let (Some(address), Some(key)) = (self.joining.take(), transport.peer_identity()) {
                self.keys.remember_host(address, key);
            }
            if let Err(error) = self.join_level(&level, transport) {
                console.print(format!("couldn't load {level}: {error}"));
                self.leave_game();
            }
        } else if !transport.is_connected() || self.connect_time > CONNECT_SECONDS {
            let reason = transport.refusal();
            let reason = reason.as_deref().unwrap_or("no answer");
            console.print(format!("couldn't connect: {reason}"));
        } else {
            self.connecting = Some(transport);
        }
    }

    /// Follows the server to a new level.
    pub(super) fn follow_level(&mut self, console: &mut Console<Self>) {
        let wanted = self.client.as_mut().and_then(Client::take_level);
        if let (Some(name), None) = (wanted, &self.server)
            && let Some(client) = self.client.take()
            && let Err(error) = self.join_level(&name, client.into_transport())
        {
            console.print(format!("couldn't load {name}: {error}"));
            self.leave_game();
        }
    }
}
