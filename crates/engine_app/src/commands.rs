//! The application's own console commands.

use std::error::Error;
use std::fs::File;
use std::io::BufWriter;
use std::time::Duration;

use engine_assets::AssetError;
use engine_client::Buttons;
use engine_console::{Args, Console};
use engine_net::{
    Conditioned, Conditions, DEFAULT_PORT, HostOptions, JoinOptions, NetClient, NetServer,
    Transport, full_address, short_id,
};
use engine_render::Capture;
use engine_server::Kicked;
use fixture_quake::{DEFAULT_CONFIG, GAME_COMMANDS};

use crate::app::{MOVE_BINDINGS, World};
use crate::keys::to_hex;
use crate::saves;

/// The width of a screenshot, unless it's given.
const SCREENSHOT_WIDTH: u32 = 1280;

/// The height of a screenshot, unless it's given.
const SCREENSHOT_HEIGHT: u32 = 720;

/// Registers the commands.
pub(crate) fn register(console: &mut Console<World>) {
    console.register("quit", quit);
    console.register("path", path);
    console.register("dir", dir);
    console.register("exec", exec);
    console.register("stuffcmds", stuffcmds);
    console.register("impulse", impulse);
    console.register("stats", stats);
    console.register("map", map);
    console.register("bind", bind);
    console.register("unbind", unbind);
    console.register("unbindall", unbindall);
    console.register("screenshot", screenshot);
    console.register("gpu_times", gpu_times);
    console.register("exposure", exposure);
    console.register("screen", screen);
    console.register("setpos", setpos);
    console.register("sun_toward", sun_toward);
    console.register("time_of_day", time_of_day);
    console.register("camera", camera);
    console.register("noclip", noclip);
    console.register("changelevel", changelevel);
    console.register("restart", restart);
    console.register("newgame", newgame);
    console.register("reset_defaults", reset_defaults);
    console.register("check_bindings", check_bindings);
    console.register("togglemenu", togglemenu);
    console.register("toggleconsole", toggleconsole);
    console.register("clear", clear);
    console.register("menu_main", open_menu);
    console.register("menu_save", open_menu);
    console.register("menu_load", open_menu);
    console.register("menu_options", open_menu);
    console.register("menu_keys", open_menu);
    console.register("menu_video", open_menu);
    console.register("menu_audio", open_menu);
    console.register("menu_quit", open_menu);
    console.register("help", open_menu);
    console.register("bf", bonus_flash);
    console.register("pause", pause);
    console.register("centerview", centerview);
    console.register("+showscores", show_scores);
    console.register("-showscores", hide_scores);
    console.register("startdemos", start_screen);
    console.register("host", host);
    console.register("connect", connect);
    console.register("disconnect", disconnect);
    console.register("hostcode", hostcode);
    console.register("rotate_host_key", rotate_host_key);
    console.register("forget_host", forget_host);
    console.register("kick", kick);
    console.register("ban", ban);
    console.register("unban", unban);
    console.register("bans", bans);
    console.register("addbot", add_bot);
    console.register("removebots", remove_bots);
    console.register("say", say);
    console.register("say_team", say_team);
    console.register("messagemode", message_mode);
    console.register("messagemode2", message_mode_team);
    console.register("menu_multiplayer", open_menu);
    console.register("save", save);
    console.register("load", load);
    for command in GAME_COMMANDS {
        console.register(command, game_command);
    }
    console.register("status", game_command);
    for ignored in ["+mlook", "-mlook", "+klook", "-klook", "sizeup", "sizedown"] {
        console.register(ignored, nothing);
    }
    for setting in ORIGINAL_SETTINGS {
        console.register(setting, nothing);
    }
    for button in Buttons::NAMES {
        console.register(&format!("+{button}"), button_down);
        console.register(&format!("-{button}"), button_up);
    }
}

/// `quit`: stops the engine.
fn quit(_console: &mut Console<World>, world: &mut World, _args: &Args) {
    world.quit = true;
}

/// `path`: shows the search path, highest priority first.
fn path(console: &mut Console<World>, world: &mut World, _args: &Args) {
    console.print("Current search path:");
    for label in world.filesystem.search_path() {
        console.print(label);
    }
}

/// `dir [prefix]`: lists files on the search path.
fn dir(console: &mut Console<World>, world: &mut World, args: &Args) {
    let files = world.filesystem.list(args.get(1));
    let count = files.len();
    for file in files {
        console.print(file);
    }
    console.print(format!("{count} files"));
}

/// `exec file`: runs a script next.
fn exec(console: &mut Console<World>, world: &mut World, args: &Args) {
    let name = args.get(1);
    match world.filesystem.read(name) {
        Ok(bytes) => {
            console.print(format!("execing {name}"));
            console.insert(&String::from_utf8_lossy(&bytes));
        }
        Err(AssetError::NotFound(_)) => console.print(format!("couldn't exec {name}")),
        Err(error) => console.print(format!("couldn't exec {name}: {error}")),
    }
}

/// `stuffcmds`: runs the `+` commands from the command line.
fn stuffcmds(console: &mut Console<World>, world: &mut World, _args: &Args) {
    let commands = std::mem::take(&mut world.startup_commands);
    console.insert(&commands);
}

/// `stats`: shows the tick and frame counts, and the view.
fn stats(console: &mut Console<World>, world: &mut World, _args: &Args) {
    let seconds = world.run_time.as_secs_f64();
    console.print(format!(
        "{} ticks and {} frames, {} reduced, in {seconds:.2} s",
        world.ticks, world.frames, world.reduced_frames
    ));
    console.print(format!(
        "{} motion events, yaw {:.4}, pitch {:.4}",
        world.motion_events,
        world.look.yaw(),
        world.look.pitch()
    ));
    let load = world.audio.peak_load().unwrap_or(0.0) * 100.0;
    let gap = world.audio.longest_gap().unwrap_or_default().as_secs_f64() * 1000.0;
    console.print(format!(
        "audio: device {}, {} playing, {} static in {} voices, {} of {} sounds missing, \
         music {:?}, thread peaked at {load:.1}%, longest gap {gap:.1} ms, ambient {:.2?}, \
         ears {:.2?}",
        world.audio.has_device(),
        world.audio.playing().len(),
        world.audio.static_count(),
        world.audio.static_volumes().len(),
        world.sounds.missing(),
        world.sounds.len(),
        world.music_track,
        world.audio.ambient_volumes(),
        world.audio.static_volumes()
    ));
    console.print(format!(
        "models: {} loaded, {} missing, {} particles, {} lights",
        world.models.len(),
        world.models.missing(),
        world.effects.authored().count(),
        world.effects.lights().count()
    ));
    if let Some(renderer) = &world.renderer {
        let stats = renderer.statistics();
        let at = world
            .client
            .as_ref()
            .map_or_else(Default::default, |client| client.predicted().origin);
        console.print(format!(
            "{}: {} of {} triangles in {} batches, at {:.1} {:.1} {:.1}",
            renderer.device_name(),
            stats.drawn_triangles,
            stats.total_triangles,
            stats.batches,
            at.x,
            at.y,
            at.z
        ));
    }
}

/// `map name`: starts a new game on a map.
fn map(console: &mut Console<World>, world: &mut World, args: &Args) {
    match world.new_game(args.get(1), console) {
        Ok(()) => console.print(format!("loaded {}", args.get(1))),
        Err(error) => console.print(format!("couldn't load {}: {error}", args.get(1))),
    }
}

/// `changelevel name`: goes to a map as if the level had ended there, with
/// what the player has.
fn changelevel(console: &mut Console<World>, world: &mut World, args: &Args) {
    if let Some(client) = &mut world.client {
        client.send_console(&format!("changelevel {}", args.get(1)));
    } else {
        console.print("changelevel needs a game running");
    }
}

/// `restart`: starts the level again.
fn restart(console: &mut Console<World>, world: &mut World, _args: &Args) {
    let (name, parms, flags) = (
        world.level.name.clone(),
        world.level.parms.clone(),
        world.level.flags,
    );
    if !name.is_empty() {
        world.carry(parms, flags);
        if let Err(error) = world.load_map(&name, console) {
            console.print(format!("couldn't load {name}: {error}"));
        }
    }
}

/// `newgame`: starts a game for one from the first map, leaving any other.
fn newgame(console: &mut Console<World>, _world: &mut World, _args: &Args) {
    console.add("disconnect; deathmatch 0; coop 0; map start");
}

/// `reset_defaults`: goes back to the game's settings, and this engine's keys.
fn reset_defaults(console: &mut Console<World>, _world: &mut World, _args: &Args) {
    console.add(&format!("exec {DEFAULT_CONFIG}"));
    console.add(MOVE_BINDINGS);
}

/// `check_bindings`: a player with no keys at all can't reach the menus to fix it, so they get
/// the usual ones back.
fn check_bindings(console: &mut Console<World>, world: &mut World, _args: &Args) {
    if world.bindings.iter().next().is_none() {
        console.add(&format!("exec {DEFAULT_CONFIG}"));
        console.add(MOVE_BINDINGS);
    }
}

/// `togglemenu`: opens or closes the menus.
fn togglemenu(_console: &mut Console<World>, world: &mut World, _args: &Args) {
    if world.console_view.is_open() && world.client.is_some() {
        world.console_view.set_open(false);
        return;
    }
    if world.ui_script.is_some() {
        world.toggle_script_menu();
    } else {
        // With no UI at all, the console is what's left.
        let open = !world.console_view.is_open();
        world.console_view.set_open(open);
    }
}

/// `toggleconsole`: opens or closes the console.
fn toggleconsole(_console: &mut Console<World>, world: &mut World, _args: &Args) {
    world.script_menu = false;
    let open = !world.console_view.is_open() || world.client.is_none();
    world.console_view.set_open(open);
}

/// `clear`: empties the console.
fn clear(_console: &mut Console<World>, world: &mut World, _args: &Args) {
    world.console_view.clear();
}

/// Opens the menu screen a command is named for.
fn open_menu(_console: &mut Console<World>, world: &mut World, args: &Args) {
    let screen = match args.get(0) {
        "menu_save" => "save",
        "menu_load" => "load",
        "menu_options" => "options",
        "menu_keys" => "keys",
        "menu_video" => "video",
        "menu_audio" => "audio",
        "menu_quit" => "quit",
        "menu_multiplayer" => "multiplayer",
        "help" => "help",
        _ => "main",
    };
    world.open_script_menu(screen);
}

/// `bf`: flashes the view.
fn bonus_flash(_console: &mut Console<World>, world: &mut World, _args: &Args) {
    world.view_tint.bonus_flash();
}

/// `pause`: stops the game, or starts it again.
fn pause(_console: &mut Console<World>, world: &mut World, _args: &Args) {
    // A game with others in it can't stop for one of them.
    world.paused = !world.paused && world.client.is_some() && !world.is_online();
}

/// `centerview`: looks straight ahead.
fn centerview(_console: &mut Console<World>, world: &mut World, _args: &Args) {
    let yaw = world.look.yaw();
    world.look.set_view(yaw, 0.0);
}

/// `+showscores`: holds the scores open.
fn show_scores(_console: &mut Console<World>, world: &mut World, _args: &Args) {
    world.game_screen.set_show_scores(true);
}

/// `-showscores`: lets the scores go.
fn hide_scores(_console: &mut Console<World>, world: &mut World, _args: &Args) {
    world.game_screen.set_show_scores(false);
}

/// `startdemos`: the last line of `quake.rc`. Stock demos can't play here, so
/// with no game running this opens the menus.
fn start_screen(console: &mut Console<World>, world: &mut World, _args: &Args) {
    if world.client.is_none() {
        console.add("menu_main");
    }
}

/// `host [port]`: lets others join the game running here.
fn host(console: &mut Console<World>, world: &mut World, args: &Args) {
    if world.server.is_none() {
        console.print("host needs a game running: start a map first");
        return;
    }
    let port = args.get(1).parse().unwrap_or(DEFAULT_PORT);
    let players = console
        .cvar("maxplayers")
        .map_or(8.0, engine_console::Cvar::number);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a few players"
    )]
    let players = players.clamp(1.0, 16.0) as usize;
    let password = console
        .cvar("sv_password")
        .map_or("", engine_console::Cvar::value);
    let guarded = !password.is_empty();
    let options = HostOptions {
        password: password.to_owned(),
        identity: world.keys.host.clone(),
        banned: world.keys.banned.clone(),
    };
    match NetServer::bind(port, players, options) {
        Ok(server) => {
            let entry = if guarded {
                "with a password"
            } else {
                "open to anyone"
            };
            console.print(format!("hosting on port {}, {entry}", server.port()));
            console.print(format!("host code {}", world.keys.host.code()));
            world.net_server = Some(server);
        }
        Err(error) => console.print(format!("couldn't host: {error}")),
    }
}

/// `addbot [ping]`: adds a bot on a connection of that many milliseconds.
fn add_bot(console: &mut Console<World>, world: &mut World, args: &Args) {
    match world.add_bot(args.get(1).parse().unwrap_or(0)) {
        Some(name) => console.print(format!("{name} joins")),
        None => console.print("addbot needs a game running here: start a map first"),
    }
}

/// `removebots`: sends the bots home.
fn remove_bots(_console: &mut Console<World>, world: &mut World, _args: &Args) {
    world.bots.clear();
}

/// Settings a Quake `config.cfg` holds for things this engine doesn't have, such as modems
/// and video modes. They mean nothing here, and each would say it's unknown at every start.
const ORIGINAL_SETTINGS: &[&str] = &[
    "_cl_color",
    "_cl_name",
    "_config_com_baud",
    "_config_com_irq",
    "_config_com_modem",
    "_config_com_port",
    "_config_modem_clear",
    "_config_modem_dialtype",
    "_config_modem_hangup",
    "_config_modem_init",
    "_snd_mixahead",
    "_vid_default_mode",
    "_vid_default_mode_win",
    "_vid_wait_override",
    "_windowed_mouse",
    "cl_backspeed",
    "cl_forwardspeed",
    "joystick",
    "lookspring",
    "lookstrafe",
    "m_forward",
    "m_side",
    "saved1",
    "saved2",
    "saved3",
    "saved4",
    "savedgamecfg",
    "vid_config_x",
    "vid_config_y",
    "vid_nopageflip",
    "vid_stretch_by_2",
];

/// The network the `net_fake` settings ask for, or `None` for the real one.
fn fake_network(console: &Console<World>) -> Option<Conditions> {
    let number = |name: &str| console.cvar(name).map_or(0.0, engine_console::Cvar::number);
    let (latency, jitter, loss, burst) = (
        number("net_fake_latency"),
        number("net_fake_jitter"),
        number("net_fake_loss"),
        number("net_fake_burst"),
    );
    let half = Duration::from_secs_f32(latency.max(0.0) / 2000.0);
    (latency > 0.0 || jitter > 0.0 || loss > 0.0).then(|| Conditions {
        send_delay: half,
        receive_delay: half,
        jitter: Duration::from_secs_f32(jitter.max(0.0) / 1000.0),
        loss: f64::from(loss.clamp(0.0, 100.0)) / 100.0,
        burst: f64::from(burst.clamp(0.0, 100.0)) / 100.0,
    })
}

/// `connect address [code]`: joins a game, checking the host against its code
/// if it's given, or else against who was there last time. It goes through the
/// `net_fake` settings if any are set: `net_fake_latency` and `net_fake_jitter`
/// in milliseconds, and `net_fake_loss` in percent, of what's sent, with `net_fake_burst`
/// the percent chance that what follows a loss is lost too.
fn connect(console: &mut Console<World>, world: &mut World, args: &Args) {
    let password = console
        .cvar("password")
        .map_or("", engine_console::Cvar::value);
    let address = full_address(args.get(1));
    let code = args.get(2);
    let options = JoinOptions {
        password: password.to_owned(),
        identity: world.keys.player.clone(),
        known_host: world.keys.known_host(&address),
        code: (!code.is_empty()).then(|| code.to_owned()),
    };
    match NetClient::connect(args.get(1), options) {
        Ok(client) => {
            console.print(format!("connecting to {}", args.get(1)));
            let transport: Box<dyn Transport> = match fake_network(console) {
                Some(conditions) => Box::new(Conditioned::new(Box::new(client), conditions, 1)),
                None => Box::new(client),
            };
            world.begin_connection(transport);
            world.joining = Some(address);
        }
        Err(error) => console.print(format!("couldn't connect: {error}")),
    }
}

/// `disconnect`: leaves the game.
fn disconnect(_console: &mut Console<World>, world: &mut World, _args: &Args) {
    world.leave_game();
}

/// `hostcode`: shows the code that goes with this host's address.
fn hostcode(console: &mut Console<World>, world: &mut World, _args: &Args) {
    console.print(format!("host code {}", world.keys.host.code()));
}

/// `rotate_host_key`: changes this host's key. Players who know the old one are
/// told as they come back, and until they all have been, the old one still
/// leads to the new.
fn rotate_host_key(console: &mut Console<World>, world: &mut World, _args: &Args) {
    if let Some(net_server) = &mut world.net_server {
        net_server.rotate_identity();
        world.keys.host = net_server.identity();
    } else {
        world.keys.host.rotate();
    }
    world.keys.save_host();
    console.print(format!(
        "new host code {}, with {} change to pass on",
        world.keys.host.code(),
        world.keys.host.link_count()
    ));
}

/// `forget_host address`: forgets who's at an address, for when it's someone
/// else's now.
fn forget_host(console: &mut Console<World>, world: &mut World, args: &Args) {
    let address = full_address(args.get(1));
    if world.keys.forget_host(&address) {
        console.print(format!("forgot {address}"));
    } else {
        console.print(format!("{address} isn't known"));
    }
}

/// `kick name`: puts a guest out.
fn kick(console: &mut Console<World>, world: &mut World, args: &Args) {
    let found = world.server.as_mut().map_or(Kicked::Nobody, |server| {
        server.kick(args.get(1), "you've been kicked")
    });
    if found == Kicked::Nobody {
        console.print(format!("nobody here is called {}", args.get(1)));
    }
}

/// `ban name`: puts a guest out, and keeps them out.
fn ban(console: &mut Console<World>, world: &mut World, args: &Args) {
    let found = world.server.as_mut().map_or(Kicked::Nobody, |server| {
        server.kick(args.get(1), "you've been banned")
    });
    match found {
        Kicked::Key(key) => {
            let _new = world.keys.banned.insert(key);
            world.keys.save_bans();
            if let Some(net_server) = &mut world.net_server {
                net_server.ban(key);
            }
            console.print(format!("banned {}: {}", args.get(1), short_id(&key)));
        }
        Kicked::Anonymous => console.print("kicked them, but they've no key to ban"),
        Kicked::Nobody => console.print(format!("nobody here is called {}", args.get(1))),
    }
}

/// `unban id`: lets someone back, by the start of their key.
fn unban(console: &mut Console<World>, world: &mut World, args: &Args) {
    let found = world
        .keys
        .banned
        .iter()
        .find(|key| !args.get(1).is_empty() && to_hex(key).starts_with(args.get(1)))
        .copied();
    if let Some(key) = found {
        let _was = world.keys.banned.remove(&key);
        world.keys.save_bans();
        if let Some(net_server) = &mut world.net_server {
            net_server.unban(&key);
        }
        console.print(format!("unbanned {}", short_id(&key)));
    } else {
        console.print(format!("nobody banned starts {}", args.get(1)));
    }
}

/// `bans`: lists who's banned.
fn bans(console: &mut Console<World>, world: &mut World, _args: &Args) {
    for key in &world.keys.banned {
        console.print(short_id(key));
    }
    console.print(format!("{} banned", world.keys.banned.len()));
}

/// `say text`: tells everyone.
fn say(console: &mut Console<World>, world: &mut World, args: &Args) {
    if let Some(client) = &mut world.client {
        client.send_console(&format!("say {}", args.rest()));
    } else {
        console.print("say needs a game running");
    }
}

/// `say_team text`: tells your team.
fn say_team(console: &mut Console<World>, world: &mut World, args: &Args) {
    if let Some(client) = &mut world.client {
        client.send_console(&format!("say_team {}", args.rest()));
    } else {
        console.print("say_team needs a game running");
    }
}

/// `messagemode`: types one line to say to everyone, over the view.
fn message_mode(_console: &mut Console<World>, world: &mut World, _args: &Args) {
    if world.client.is_some() {
        world.console_view.open_chat(false);
    }
}

/// `messagemode2`: types one line to say to your team, over the view.
fn message_mode_team(_console: &mut Console<World>, world: &mut World, _args: &Args) {
    if world.client.is_some() {
        world.console_view.open_chat(true);
    }
}

/// `save name`: saves the game.
fn save(console: &mut Console<World>, world: &mut World, args: &Args) {
    let name = args.get(1);
    match saves::save(world, name) {
        Ok(()) => console.print(format!("saved {name}")),
        Err(error) => console.print(format!("couldn't save {name}: {error}")),
    }
}

/// `load name`: goes back to a saved game.
fn load(console: &mut Console<World>, world: &mut World, args: &Args) {
    let name = args.get(1);
    match saves::load(world, name) {
        Ok(()) => console.print(format!("loaded {name}")),
        Err(error) => console.print(format!("couldn't load {name}: {error}")),
    }
}

/// Passes a command to the game's scripts, through the server.
fn game_command(_console: &mut Console<World>, world: &mut World, args: &Args) {
    if let Some(client) = &mut world.client {
        client.send_console(&format!("{} {}", args.get(0), args.rest()));
    }
}

/// Accepts a command the original has and this engine doesn't need.
fn nothing(_console: &mut Console<World>, _world: &mut World, _args: &Args) {}

/// `bind key [command]`: sets a binding, or shows it.
fn bind(console: &mut Console<World>, world: &mut World, args: &Args) {
    let key = args.get(1);
    match args.count() {
        0 | 1 => console.print("bind <key> [command]"),
        2 => {
            let text = world.bindings.get(key).map_or_else(
                || format!("\"{key}\" is not bound"),
                |command| format!("\"{key}\" = \"{command}\""),
            );
            console.print(text);
        }
        _ => {
            let command = (2..args.count())
                .map(|index| args.get(index))
                .collect::<Vec<_>>()
                .join(" ");
            world.bindings.bind(key, &command);
        }
    }
}

/// `unbind key`: removes a binding.
fn unbind(_console: &mut Console<World>, world: &mut World, args: &Args) {
    world.bindings.unbind(args.get(1));
}

/// `unbindall`: removes every binding.
fn unbindall(_console: &mut Console<World>, world: &mut World, _args: &Args) {
    world.bindings.clear();
}

/// `+button`: holds a movement button.
fn button_down(_console: &mut Console<World>, world: &mut World, args: &Args) {
    let _known = world.buttons.set(args.get(0).trim_start_matches('+'), true);
}

/// `-button`: lets go of a movement button.
fn button_up(_console: &mut Console<World>, world: &mut World, args: &Args) {
    let _known = world
        .buttons
        .set(args.get(0).trim_start_matches('-'), false);
}

/// Writes `capture` to a PNG file at `path`.
fn write_png(capture: &Capture, path: &str) -> Result<(), Box<dyn Error>> {
    let file = BufWriter::new(File::create(path)?);
    let mut encoder = png::Encoder::new(file, capture.width, capture.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&capture.rgba)?;
    Ok(())
}

/// `screenshot path [width height]`: writes the current view to a PNG file, drawn at that size
/// and with the UI laid out for it, so that any resolution can be looked at.
fn screenshot(console: &mut Console<World>, world: &mut World, args: &Args) {
    let default_path = saves::screenshot_path(&world.user_directory);
    let path = if args.get(1).is_empty() {
        default_path.as_str()
    } else {
        args.get(1)
    };
    let width = args.get(2).parse().unwrap_or(SCREENSHOT_WIDTH);
    let height = args.get(3).parse().unwrap_or(SCREENSHOT_HEIGHT);
    let mut picture = world.view(0.0, 1.0, 0.0);
    picture.quads = world.overlay((width, height));
    let Some(renderer) = &mut world.renderer else {
        console.print("no renderer");
        return;
    };
    let result = renderer
        .capture(&picture.view, picture.drawables(), width, height)
        .map_err(Into::into)
        .and_then(|capture| write_png(&capture, path));
    match result {
        Ok(()) => console.print(format!("wrote {path}")),
        Err(error) => console.print(format!("couldn't write {path}: {error}")),
    }
}

/// `gpu_times`: starts the GPU's clock, and after that says what each part of a frame took on
/// average since it was last asked. `gpu_times off` stops it.
fn gpu_times(console: &mut Console<World>, world: &mut World, args: &Args) {
    let Some(renderer) = &mut world.renderer else {
        console.print("no renderer");
        return;
    };
    let on = args.get(1) != "off";
    if !renderer.time_gpu(on) {
        console.print("this GPU has no clock");
        return;
    }
    let times = renderer.take_gpu_times();
    if times.frames == 0 {
        console.print(if on { "timing" } else { "stopped" });
        return;
    }
    console.print(format!(
        "over {} frames, in ms: scene {:.3}, post {:.3}, air {:.3}, glow {:.3}, shadows {:.3}, moving {:.3}, deform {:.3}, models {:.3}",
        times.frames,
        times.scene,
        times.post,
        times.air,
        times.glow,
        times.shadows,
        times.moving,
        times.deform,
        times.models
    ));
}

/// `exposure`: how bright what's seen is, and how far exposure has gone by itself, to tune a
/// level's range by.
fn exposure(console: &mut Console<World>, world: &mut World, _args: &Args) {
    let Some(renderer) = &world.renderer else {
        console.print("no renderer");
        return;
    };
    match renderer.exposure() {
        (Some(brightness), stops) => console.print(format!(
            "what's seen is 2^{brightness:.2} bright, and exposure's gone {stops:+.2} stops"
        )),
        (None, _) => console.print("exposure isn't going by itself here"),
    }
}

/// `screen`: what's being sent to the screen, and what it can be sent.
fn screen(console: &mut Console<World>, world: &mut World, _args: &Args) {
    let Some(renderer) = &world.renderer else {
        console.print("no renderer");
        return;
    };
    let name = |output: engine_render::ScreenOutput| match output {
        engine_render::ScreenOutput::Standard => "standard",
        engine_render::ScreenOutput::ScRgb => "HDR as scRGB",
        engine_render::ScreenOutput::Hdr10 => "HDR10",
    };
    let offers: Vec<&str> = renderer.screen_offers().into_iter().map(name).collect();
    console.print(format!(
        "sending {}, and the screen takes {}",
        name(renderer.screen_output()),
        offers.join(", ")
    ));
}

/// `setpos x y z [yaw pitch]`: moves the player, through the server.
/// `camera third 96 8 0`: changes camera, as a script would.
fn camera(console: &mut Console<World>, world: &mut World, args: &Args) {
    let line: Vec<&str> = (1..args.count()).map(|index| args.get(index)).collect();
    if line.is_empty() {
        console.print(
            "camera first | third [boom rise side] | fixed x y z yaw pitch idle | scene name",
        );
        return;
    }
    world.change_camera(&line.join(" "), 0.0);
}

fn setpos(console: &mut Console<World>, world: &mut World, args: &Args) {
    let number = |index: usize| args.get(index).parse::<f32>().ok();
    let (Some(x), Some(y), Some(z)) = (number(1), number(2), number(3)) else {
        console.print("setpos <x> <y> <z> [yaw pitch]");
        return;
    };
    if let Some(client) = &mut world.client {
        client.send_console(&format!("setpos {x} {y} {z}"));
    }
    if let (Some(yaw), Some(pitch)) = (number(4), number(5)) {
        world.look.set_view(f64::from(yaw), f64::from(pitch));
    }
}

/// `sun_toward`: puts a sun that isn't baked towards a direction, and with none, back where the
/// level has it. Below the horizon it's night.
fn sun_toward(console: &mut Console<World>, world: &mut World, args: &Args) {
    let number = |index: usize| args.get(index).parse::<f32>().ok();
    world.sun_toward = match (number(1), number(2), number(3)) {
        (Some(x), Some(y), Some(z)) => Some(engine_core::math::Vec3::new(x, y, z)),
        _ => {
            console.print("sun_toward <x> <y> <z>, or nothing for where the level has it");
            None
        }
    };
}

/// `time_of_day`: sets the hour, from 0 to 24, for everyone, in a level with a day.
fn time_of_day(console: &mut Console<World>, world: &mut World, args: &Args) {
    let Ok(hour) = args.get(1).parse::<f32>() else {
        console.print("time_of_day <hour>, from 0 to 24");
        return;
    };
    if let Some(client) = &mut world.client {
        client.send_console(&format!("time_of_day {hour}"));
    }
}

/// `impulse`: sends a number to the game with the next command.
fn impulse(_console: &mut Console<World>, world: &mut World, args: &Args) {
    world.buttons.impulse = args.get(1).parse().unwrap_or(0);
}

/// `noclip`: flies through walls, through the server.
fn noclip(_console: &mut Console<World>, world: &mut World, _args: &Args) {
    if let Some(client) = &mut world.client {
        client.send_console("noclip");
    }
}
