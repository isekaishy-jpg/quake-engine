# Proposal 0137: tools an agent can trust

Status: accepted and implemented, 2026-11-06, the [work list](../research/work-list.md), items 31 to 38.

## Research

A test on 2026-11-06 drove the real `engine-mcp` through a level and through bad input. The loop worked, and a level built only through it looked right. It also found the gaps these items list. The human's word: put them right, so that Claude can use it well.

| Item | What was found |
|---|---|
| 31 | All 42 tools had an empty schema |
| 32 | An unknown type, a missing effect or music, a zone with no shape, an inverted brush, an unknown block and a misspelt field all passed `validate` and `compile` |
| 33 | With a map outside the content, `shot` said ok and showed Quake's menu. `playtest` only said the game stayed up |
| 34 | Paths went from wherever the server started, and `save` could leave the content |
| 35 | `describe` was 2 MB on Vesper Deep |
| 36 | Undo kept whole maps: 22 MB became 419 MB in 40 edits |
| 37 | Nothing could write shaders, effects or prefabs, compile textures or build a container |
| 38 | Something that isn't JSON got no answer at all, and no test spoke to the server |

## Design

- **Schemas.** Each operation lists its arguments, with what each is, whether it's needed and what it means, and `OperationInfo::schema` makes that JSON schema. The list is the one place, so it can't drift from a separate document.
- **Refused at once.** A brush inside out, a block a map can't hold and a mesh that isn't there are errors when they're added. `place_mesh` takes `barrel` or `meshes/barrel.emesh`.
- **`validate` finds the rest,** in `checks.rs`: an effect or music that isn't there, a zone with no shape, an unknown field in a block with a set of them, no start, a missing material. It answers `ok` and `problems`. `compile` refuses a map with any.
- **Entity types come from the game,** which the tools can't see. So a game now says `no spawn function for` a type it doesn't know, as the original did, and `playtest` passes that on.
- **Running says what happened.** `shot` and `playtest` read what the game prints. `shot` is an error if the level didn't load, and takes `from`, `yaw` and `pitch`. `playtest` answers `loaded`, `stayed_up`, `trouble` and `ok`.
- **One rule for paths.** `open` and `save` go from the content, and can't leave it. A map has to be in the content for the game to find it, so that's checked too.
- **`describe` narrows,** by `id`, `kind`, `type`, `prefab`, `near` and `radius`, 200 at a time with `skip`, and says how many match. `open` answers with counts.
- **Undo keeps changes.** A step is the run of blocks that changed, found by comparing with the last settled map, so one edit costs one edit. A step that touches everything, such as `wear`, costs what it changed.
- **New tools:** `read_` and `write_shader`, `_effect` and `_prefab`, each refusing what doesn't work and leaving what was there, and `compile_textures`, `compile_shaders` and `pack`.
- **The server** answers what isn't JSON with a parse error, and tests now speak to it as an agent does.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** the editor can only save into the content now, which a game needs anyway. Unknown entity types are found by running, not by `validate`, as only the game knows them.
- **Acceptance criteria:** each row of the table above is put right, and a level can still be built, compiled, looked at and tested through MCP alone.
- **Future extension:** none planned.

## Review

The same drive through the real server was run again.

| Measure | Before | After |
|---|---|---|
| Memory after 40 edits on Vesper Deep | 419 MB | 35 MB |
| 40 edits | 0.44 s | 0.09 s |
| What `describe` returns by default | 2.1 MB | 60 KB |

`validate` found a real fault straight away: a brush in Vesper Deep with no height, at 480 to 480. It goes with the level's next change.
