# Proposal 0118: the status bar and menus as scripts

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

Whether they should be scripts was the human's decision. They were Rust in the fixture, and only a canvas reached the engine. It's decided here: a game's look is what an author most wants to change, and it's the part of the engine most like a level, so it belongs in content, with what the human asked for of agents and authors.

- What's drawn each frame has to stay cheap, and a script that fails mustn't stop a frame.
- Game scripts run on the server, and have the game. A menu needs none of that, and shouldn't have it.
- Immediate mode is the simplest thing to script: each frame, say what's there. There's no widget tree to keep in step with anything.

## Design

- **`scripts/ui.luau`** runs in a sandbox of its own, on the client, with no reach into the game. The engine calls `draw_status`, `draw_menu`, `open_menu` and `menu_key`, and each gets the screen's size, the time, numbers about the game, and the settings the script listed in `settings`.
- **Commands, not calls.** `ui.frame`, `ui.text` and the rest add to a list, which the engine then draws. So a script can't touch the renderer, and all it can do is ask.
- **What it can do:** `ui.run` a console line, `ui.sound`, and `ui.close`.
- **Fallback.** It's used when the content has it and the engine's look. Otherwise, or if it won't run, the fixture's Rust status bar and menus carry on, so Quake with no art looks as it did. The Rust stays, since it's the fixture's.
- **The status bar only replaces the bar.** Words in the middle of the screen and the intermission stay as they were.
- The menu in `content/scripts/ui.luau` has levels and options, with the kit's frames, the kris pointer, the smear on what's chosen, and the impact on what's pressed.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** the keyboard works the menu, as the fixture's does, and the mouse doesn't yet. A script's errors are dropped each frame, once it's running. The console shows nothing, so one that fails partway through leaves its menu blank.
- **Acceptance criteria:** a script can draw a status bar and a menu and hear keys. It can't reach beyond its sandbox. With no script or no art, nothing changes.
- **Future extension:** the mouse, and scores and the intermission as scripts.

## Review

The menu and status bar were looked at in a level, and `the_engines_own_script_runs` takes the shipped script through its keys.
