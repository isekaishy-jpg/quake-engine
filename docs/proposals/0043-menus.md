# Proposal 0043: the menus

Status: accepted, 2026-11-04, increment 8.

## Research

- `menu.c` is a state machine: each screen has a draw function and a key function, and everything drawn is a picture from `gfx`, or text in the font's other colour.
- Of the original's options, screen size is dropped, and lookspring and lookstrafe have no meaning with mouse look always on. The video screen was tied to 1996's modes.
- The catalog keeps Main, Quit, Help, Single Player, Load, Save, Options, Keys and Video. Multiplayer waits for online play.
- Settings changed in a menu have to survive, and the game's folder is read-only. `Host_WriteConfiguration` writes bindings and saved cvars on exit.

## Design

- `fixture_quake::Menu` is a state machine like the original's, with rows of options as data. It reaches the engine only through the `MenuHost` trait, so it holds no console or renderer.
- The Video screen has vertical sync, the frame rate, field of view and interface size.
- `config.cfg` goes in the user's folder, which is mounted first. The modern movement keys only apply when there's no config, so a player's own choices last.
- The view dims behind a menu, as `Draw_FadeScreen` does, and the console's picture fills in with no game.
- With no game running, the last line of `quake.rc`, `startdemos`, opens the menus. Stock demos can't play here.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** the Multiplayer row shows, since it's part of the picture, and the cursor steps over it. Menus can't be changed from Luau yet, for the reason proposal 0040 gives. Window mode isn't offered, because the platform can't change it while running.
- **Acceptance criteria:** tests walk the menus with a pretend host: new game, asking first, sliders, slots, binding keys, and quitting. Captures show the screens.
- **Future extension:** Multiplayer with online play, and window mode.

## Review

See [review 0011](../reviews/0011-increment-8.md).
