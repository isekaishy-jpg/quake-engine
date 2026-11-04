# Proposal 0025: the Luau host

Status: accepted, 2026-11-04, increment 4.

## Research

- The stack decision is Luau through `mlua`, replacing QuakeC and `progs.dat`. `mlua` 0.12.2 is MIT, and its `vendored` feature builds Luau with the local C++ compiler.
- Luau has a native `vector` type with arithmetic, which fits QuakeC's vectors, and `mlua::Vector` carries it. Unlike QuakeC, a vector's parts can't be assigned, so ported code builds new vectors.
- `Lua::sandbox` makes the standard libraries read-only, and `set_interrupt` lets the host stop a runaway script, which replaces QuakeC's runaway guard.
- `Lua::sandbox` leaves mlua's `require` in place, which reads files from disk, so the host removes it and `loadstring` first.
- QuakeC keeps `self` and `other` as globals. Passing them as arguments removes the save and restore code around every call.
- The original runs game code in the middle of physics, with engine data one pointer away. Rust won't allow script callbacks to reach data that the physics loop is borrowing.
- The research plan asked for a Luau style guide before Luau code.

## Design

- `engine_script::Game` owns a sandboxed Luau state. It shares the `Simulation` through `Rc<RefCell<_>>`, borrowed only for the length of one API call.
- The simulation never calls scripts. It returns what's due, such as a think or a block, and `engine_server` passes each to the script in slot order. That keeps the order, and makes re-entry impossible.
- A small Luau prelude gives entities QuakeC's feel. An entity is a table, game fields are plain fields, and engine fields such as `origin` and `nextthink` go through a metatable to Rust.
- Callbacks take arguments: `think(self)`, `touch(self, other)`, `use(self, activator)`, `blocked(self, other)`.
- An error in a script is reported and the game carries on. A call that runs for over a second is stopped.
- Sources arrive as a list of names and text, so the host knows no game. Quake's scripts live in `fixture_quake`.
- `docs/style.md` gets a short Luau section: files start with what they port, and names follow the QuakeC originals, so a port can be checked against its source.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** engine fields cost a call per read, which is fine for hundreds of entities. Automatic formatting of Luau isn't enforced yet, because no tool for it is part of the build.
- **Acceptance criteria:** tests cover spawning, engine fields, game fields, callbacks, script errors, a runaway loop and sandbox limits.
- **Future extension:** the API grows with each part of the port, and `StyLua` can join `xtask`.

## Review

See [review 0007](../reviews/0007-increment-4.md).
