# Proposal 0135: the engine and the game

Status: accepted, 2026-11-06, the [work list](../research/work-list.md), item 77. Moves are done by the items named below.

## Research

At first Quake was the game. The rule from [proposal 0008](0008-crate-boundaries.md) has held: only `fixture_quake` knows Quake's formats, and only `engine_app`, the composition root, knows it exists. Twelve of `engine_app`'s files reach into it. Going through what they take, it falls into a few groups.

## Design

| What | Where it is | Where it goes | By |
|---|---|---|---|
| Colour over the view | `hud.rs` | `ViewTint`, done | Item 74 |
| The status bar, menus, scores and the intermission | `hud.rs`, `menu.rs` | Scripts | Items 1, 2 and 6 |
| What's Quake's in those: pictures, faces, weapons on the bar | `hud.rs` | Gone, as Quake uses the engine's UI | Item 6 |
| WAV files | `wav.rs` | `engine_audio`, as anything may use them | Item 103 |
| Cameras | `view.rs` | Engine, with scripts choosing | Items 56 to 60 |
| Weapon rules, for predicting shots | `weapon_rules.rs` | The fixture, as they're the game's, reached as data | Item 6 |
| Where things are, rules and commands | `layout.rs` | One value that says what the game is, once the UI's gone and little is left | Item 6 |
| Reading `.bsp`, `.mdl`, `.spr`, `.wad`, `.pak`, palettes | The fixture | Stays. This is what it's for | |

**The rule from here:** the engine has what any game needs, game code says what happens, and the fixture reads Quake's files. Quake stays playable through all of this, on the new systems, as the test that nothing was lost.

## Proposal

- **Selected approach:** do each move with the work that needs it, and not as one big change. A large move ahead of that work would only be moved again.
- **Tradeoffs:** `engine_app` knows the fixture until item 6.
- **Acceptance criteria:** at the end of item 6, `engine_app` only takes Quake's file formats and one value saying what the game is.
- **Future extension:** none planned.
