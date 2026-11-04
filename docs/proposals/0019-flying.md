# Proposal 0019: key bindings and flying

Status: accepted, 2026-11-03, increment 2.

## Research

- `keys.c` binds names to commands. A binding starting with `+` runs on press, and the matching `-` runs on release. Names are case-insensitive, and configs use names like `UPARROW`, `CTRL`, `MOUSE1` and `MWHEELUP`.
- `cl_input.c` holds `+forward` and the rest as button state, read each frame.
- The shipped `default.cfg` binds `a` to `+lookup` and `d` to `+moveup`, from before mouse look became the norm.
- A fly camera is simple, but it should run on ticks so it doesn't depend on frame rate. Between ticks, the view interpolates with `TickClock::fraction`.

## Design

- `engine_client::Bindings` maps key names to commands, translating the engine's key names to Quake's names.
- `engine_client::Buttons` holds button state, and `FlyCamera` moves on each tick and interpolates for drawing.
- The app registers `bind`, `unbind`, `unbindall` and the movement buttons.
- After `quake.rc`, the app binds WASD, space and C for flying. Other keys keep the user's config.
- `map <name>` loads a map and starts at `info_player_start`.
- `Escape` toggles mouse capture, and a click in the window recaptures it. There's no menu yet.
- `screenshot <path>` writes a PNG, with `png` private to `engine_app`.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** the WASD bindings replace what `config.cfg` says for those keys. The menu's key setup can own this later.
- **Acceptance criteria:** tests cover binding names, press and release commands, and exact camera movement per tick. Keys and mouse move the camera on a real map.
- **Future extension:** real movement replaces `FlyCamera` in increment 3, on the same buttons.

## Review

See [review 0005](../reviews/0005-increment-2.md).
