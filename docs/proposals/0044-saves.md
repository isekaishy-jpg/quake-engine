# Proposal 0044: saved games

Status: accepted, 2026-11-04, increment 8.

## Research

- The original writes every edict's fields as text. That works because QuakeC's fields are flat, and functions are numbers in `progs.dat`.
- Here, game state is in two places: engine fields in `Simulation`, and everything else in Luau tables, including functions, entities and vectors.
- A Luau closure can't be written down. Five places in the hand ported scripts kept one in `think`.
- A removed entity can still be referred to, and has to come back as a removed one.
- The client needs telling again what it heard once: light styles, sounds at fixed places, the music, and every model's name. `makestatic` style state lives there.
- Numbers must come back exact, or a game drifts from what was saved.

## Design

- A save is JSON, through `serde` and `serde_json`, with `float_roundtrip`. Its first line is a small header with the version and what the menus show, so listing saves doesn't read them whole.
- `Simulation::save` names every field in a pattern, so a new one can't be forgotten without a compile error.
- `engine_script::save` walks globals and every entity. A function is saved by its global name, and one with no global name is an error that says where it is. Those five closures became named functions, with their state in fields.
- `Server::resume` builds a game from a save without spawning the map, then tells the new client what it needs.
- Saves and the config go in the user's data folder, never the game's. A save is written to a temporary file first, so a failed write can't cost the old one.
- Saving is refused when dead, in an intermission, or in a level change, as `Host_Savegame_f` does.

## Proposal

- **Selected approach:** the above, with twelve slots in the menus, and `save` and `load` for any plain name.
- **Tradeoffs:** a save is a few hundred kilobytes, where a binary one would be smaller, but it can be read by a person. A script that keeps state in a file-level local isn't saved, so scripts must keep it in a global or field. A save of another version isn't read.
- **Acceptance criteria:** on every `id1` map, a game with a fight running, saved through JSON and resumed, stays identical to the original tick by tick for five seconds, in player, stats, entities and brush models. That test fails if restoring is deliberately broken.
- **Future extension:** moving old saves forward between versions, and saving particles and other things only the client knows.

## Review

See [review 0011](../reviews/0011-increment-8.md).
