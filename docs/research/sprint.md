# The sprint through the list

The human's instruction, 2026-11-06: take every item on the list through the [workflow](../../DESIGN.md#workflow) without them, end to end, with no list of known limitations. When the work is exhausted, review code quality in a circle and fix until nothing's left, review for bugs and fix until none are left, and sweep performance for the big items only.

Each item ends in a proposal, code, a review and a commit. Decisions that would have been the human's are made here and recorded in the proposal, with the reasoning, so they can be changed afterwards.

Status: `todo`, `doing`, `done`, or `needs a person`, which is only for what can't be done from here, with what was done as far as it can go.

## Items, in order

Earlier items unblock later ones.

| # | Item | From | Status |
|---|---|---|---|
| 1 | Measure a frame on the CPU and GPU, worst frames and hitches, and fix what's found | Open questions | done |
| 2 | Loading: stop compiling a map from text at each load | Review 0016 | done |
| 3 | Texture standard: world size, sizes, delivery spec, sets, naming, import checks | [Note](texture-standard.md) | done |
| 4 | Linear colour, with the murky look kept on purpose | [Note](texture-standard.md) | done |
| 5 | Packing: compiled textures, BC compression, one `.epak` container | [Note](texture-standard.md) | done |
| 6 | Shader system: container, cache, surface code, instances, one way of drawing | [Note](effects-water-shaders.md) | done |
| 7 | Metal, roughness, specular and gloss together | [Note](texture-standard.md) | done |
| 8 | Full meshes: format, normals, drawing path, glTF import | [Note](mesh-support.md) | done |
| 9 | Mesh light: generated light map UVs, probes for props, one face per flat polygon | [Note](mesh-support.md) | done |
| 10 | Mesh collision: generated convex shapes | [Note](mesh-support.md) | done |
| 11 | Baked shadows: several rays per sample, more than one sharp light, joins at an angle, light order and layer limits | [Note](shadow-rendering.md) | done |
| 12 | Moving lights' shadows: soft, without sawtooth or bands | [Note](shadow-rendering.md) | done |
| 13 | Particles: graph and modules, CPU and GPU, attach points | [Note](effects-water-shaders.md) | done |
| 14 | Water: the general system, reflections without stipple, depth, murk | [Note](effects-water-shaders.md) | done |
| 15 | Zones: research, then regions, sub zones, names, music, ambient light | Open questions | done |
| 16 | Engine UI: text, nine-slice, menus, splash, console, animated pictures | Open questions | done |
| 17 | Status bar and menus as scripts | Open questions | done |
| 18 | Editor: full pass for anyone to make maps | Open questions | done |
| 19 | Use layers, masks, paint, decals and hex tiling in the real levels | Review 0016 | done |
| 20 | Level faults: foundry lava, enemies once they move, real people in the city | Review 0016 | done, and a real person's model needs a person |
| 21 | Paths: jumps, water and lifts | Review 0016 | done |
| 22 | Door logic | Open questions | done |
| 23 | Coloured `.lit` light for the original levels | Open questions | done |
| 24 | Review of the host, worker and cache crates, and tests out of `src` | Open questions | done |
| 25 | Projectile launch delay and correction | Open questions | todo |
| 26 | Input at 8 kHz | Open questions | todo |
| 27 | Online numbers on a network | Open questions | todo |
| 28 | Real sound features and entity culling | [Research plan](plan.md) | todo |

## Then

| Round | Status |
|---|---|
| Circular code quality review, fixing until nothing's left | todo |
| Bug review and fix until none are left | todo |
| Performance sweeps for big items | todo |
| Report to the human | todo |

## Log

Newest last. One line for each thing finished, with its commit.

- Item 1: 226d708
- Item 2: d6e312b
- Item 3: e500242
- Item 4: d81da48
- Item 5: d4bbde9
- Item 6: b69275a
- Item 7: 71f8de6
- Item 8: 99456b9
- Item 9: 97bbd79
- Item 10: 12c3641
- Item 11: b9b296f
- Item 12: 3f84b4c
- Item 13: 3d9acf0
- Item 14: 500aa4e
- Item 15: 745556c
- Item 16: f37e544
- Item 17: 7f0b669
- Item 18: 26c7282, which went in before its checks, then dc26f87, which fixed Play, doors, Bigger, walls and the Advanced camera
- Item 19: f746656
- Item 20: 96b69e0, which also fixed how faces share samples at joins
- Item 21: 62948dd
- Item 22: 7ba6352
- Item 23: ab90233
