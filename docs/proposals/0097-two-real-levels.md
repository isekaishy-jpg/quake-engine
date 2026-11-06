# Proposal 0097: two real levels

Status: accepted, 2026-11-05, increment 12.

## Research

- `showcase`, `tile_world` and the small test maps prove features, and the human is right that they aren't levels: they're rooms in a pattern. Good fixtures need what real levels have, with places that differ, routes that loop back, landmarks, height and pacing.
- The human asked for two: a classic shooter level, and a large zone with the demands of an underground city: elevators, prefab walkways, waterways, and a ruined city above leading into it. Both are original. Neither copies any existing level's layout, names or look.
- What makes a classic level work: a hub you keep returning to, a locked door you see long before you can open it, a key that sends you somewhere new, a return by a different route, secrets that reward looking, and fights that change the room you're in.
- What makes a zone work: you can see where you're going, each district has its own shape and purpose, and there's more than one way between levels.
- Six texture packs arrived from another agent: 96 original AI-generated 256 pixel diffuse images, with materials, under the prefixes `qg01` to `qg03` and `uc01` to `uc03`. Their handoff records that no game files were used. Some repeat with visible joins, so broad surfaces need checking. They have no normal maps.
- Some of `uc01` and `uc03` lean on lime slime, skulls and flasks, which is one game's look. The zone uses their stone and metal, and not those motifs, so that it looks like itself.
- Brushes take arbitrary planes, and prefabs can make them, but each prefab stands alone, so round and sloped shapes would be written again in every file.

## Design

### Tools

- `content/prefabs/_kit.luau` holds shared shapes: prisms from a polygon, sloped slabs, and rings. A prefab gets it as `kit`, and names starting with `_` aren't listed as prefabs.
- New prefabs, made to be reused: `arch`, `column`, `broken_wall`, `rubble`, `gable`, `walkway`, `stone_bridge`, `spiral_stairs`, `pipe`, `sluice_gate`, `lantern_post`, `market_stall`, `house_shell`, `well`, `rock_wall`, `rock_spire`, `tower`, `city_house`, `quay_steps`. Any prefab can be turned, with `turn`.
- A long descent is a train between two stops, as a plain lift only rises from the bottom.
- A level is a Python script under `content/levels/` that writes a batch for the platform's tools, as `showcase` is, so levels are rebuilt the way an agent or the editor would build them.

### The Ember Keep

A classic level, in `qg01` and `qg02`. One map with no tiles.

| Part | What happens |
|---|---|
| The Causeway | Start outside, on a bridge over a lava moat, with the keep's gate shut ahead. A side door is the only way on |
| The Undercroft | Tight stone halls and the first fights, coming up inside the keep |
| The Great Hall | The hub. Pillars, a balcony, and the gold door seen from the start of it. Three ways leave it |
| The Forge | Lava channels and a lift. Holds the silver key on a platform over the fire |
| The Crypt | Behind the silver door. Dark, flooded in part. Holds the gold key, and the floor drops into an ambush |
| The Gate Tower | A button opens the main gate as a shortcut, joining the end back to the start |
| The Sanctum | Behind the gold door. The last fight, and the exit |

Three secrets: a wall that opens behind the Forge, a ledge reached from the balcony, and a flooded passage in the Crypt.

### Vesper Deep

A zone in `uc01`, `qg03` and `uc03` stone and metal, on a grid of tiles, so it exercises streaming.

| Part | What it is |
|---|---|
| The Ruined Quarter | Daylight. A broken gate, a street of roofless houses, rubble and a dry fountain in a square. Crisp sun, sky and fog |
| The Roofless Minster | A ruined church at the north end, with light falling through its windows in rays. The way down is under its altar |
| The Long Shaft | A great lift down through the rock, with windows opening on the cavern as it falls. Stairs wind round it as a second way |
| The High Terrace | Where you arrive. The whole city is seen from here, with its lamps and the canal below |
| The Canal | A long waterway that bends through the city, fed by a fall. Boats' moorings, quays and bridges |
| The Walkways | Timber and iron decks between towers, at three heights, joined by lifts and stairs |
| The Market | A terrace of stalls and lanterns, with moving lights and people to patrol it |
| The Pumpworks | Great pipes and sluice gates. A button opens the way to the far side |
| The Foundry | Fire, smoke and heat at the deep end, with lava in channels and sparks |

Placeholder figures stand in for people. Features go where there's a reason for them: rays in the Minster, light in the canal's water, fog in the cavern, a moving light in the Market, stamps in the terrace wall, splines for what patrols, and tiles under all of it.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** levels written as scripts are slower to shape than in an editor, but they're repeatable, and show what the tools can do. Geometry stays made of brushes, so curves are facets. The packs have no normal maps, so walls are lit flat. The city's people are stand-ins, and paths don't include lifts, so a route test walks each leg between them.
- **Acceptance criteria:** each level plays from start to end by its intended route, which a test holds by walking its walkable grid between its beats, with doors and keys in order. Every location looks like somewhere, in captures from a player's height. Both compile with no missing materials and hold above 60 frames a second.
- **Future extension:** real character models, and music and sound for each district.

## Review

Each level's route is walked by a test, and each part was looked at in the game. [Review 0016](../reviews/0016-increment-12.md) has what building them found, which was mostly in the compiler.
