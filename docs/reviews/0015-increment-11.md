# Review 0015: increment 11

Status: review record, 2026-11-05. Reviewed: proposals 0066 to 0091, and the work on the map format, compiling, rendering, tiles, pathing, the platform and the editor.

The human asked for every rendering feature and the editor to go through the workflow overnight, with known limitations turned into work, a test map exercising everything, and circular quality and bug passes until none were left. [The plan](../research/rendering-editor-plan.md) has the stages, and what was chosen on the human's behalf.

## How it was checked

- `cargo xtask check` before each commit, apart from the one below.
- GPU tests for each rendering feature, with a real device, and a capture of each in a real map.
- Real map and netplay suites at each point that could touch them.
- Two test maps built through the tools, and every map loaded, played and, for `showcase`, saved and loaded.
- Four passes of reading the work back, after the stages were done.

Tests that pass say little about how something looks, so captures were read too. That found more than once what tests couldn't.

## Findings

| # | Finding | Handling |
|---|---|---|
| 1 | The final pass read the scene with no barrier after it, and GPU tests failed | A barrier, in the final pass's commit. Captures had been hiding it |
| 2 | Ray brightness came from the share of the whole view in a beam, so beams faded with distance, and most steps were wasted | The march is cut to the beam, and brightness follows the path through it |
| 3 | The upload and batching of materials each had their own rule, and a blended liquid lost the device | One rule |
| 4 | Height fog filled pools | Fog stops at a pool's surface |
| 5 | Bounced light counted a whole sample for slivers of faces, so thin faces glowed | Each sample counts its share of the face |
| 6 | Light samples past a thin face's edge took light that wasn't its own | Samples are moved onto their faces |
| 7 | A hotspot was chosen for each piece of a cut face | It's chosen from the whole face |
| 8 | `ALWAYS_AWAKE` was set above the bits scripts can set | A test caught it, and it fits now |
| 9 | `walk_to` and `seeded` read an entity's ID wrongly | Found by building `showcase`, with a test now |
| 10 | Probes left out ambient light, so models went black in rooms lit by nothing else | Probes in the open hold it |
| 11 | A capsule did nothing where light had no direction | It shades straight down |
| 12 | The sky cube's corner table left a quarter of each face undrawn, from the sky stage on | Found by `tile_world`. A test now sweeps every direction |
| 13 | A brush entity slept by its origin, which is only how far it's moved | It goes by its bounds |
| 14 | `r_msaa 3` left the final pass reading single-sample depth as multisampled | Counts round down to a power of two |
| 15 | A map in one cell had a tree whose root wasn't node 0 | The first cut always happens |
| 16 | A second crisp light on a face was unshadowed and cut by the first's edge | Only the first is crisp |
| 17 | With shadows off, the last redraws went on each frame | They stop |
| 18 | The showcase's script used closures, which can't be saved | Named functions, as the game's scripts have |
| 19 | Compiling made a bake but no binary map, though the human asked for binary on compile | `.ebin`, which plays with no text |
| 20 | Editing a mesh or tile left a stale bake accepted | A bake's hash covers what a map took to build, except materials, on purpose: a changed colour shouldn't lose a bake |
| 21 | Parallax showed terraces close up | It finds where the view crosses the height |
| 22 | A crack between sky faces drew an outline with MSAA | A one pixel hole isn't a silhouette |
| 23 | Widening visible sets took time by the cube of the leaves | Neighbours are found once |
| 24 | Meshes were decided on, and no stage had them | Stage 25 |

## Process

- Commit `4b963a4` went in before its check had passed, because a script didn't wait for it, and `e3ea0c0` fixed the lint it failed on. Every commit since has gone through a script that checks first.
- `723ada7` was committed with one file through a wrong path, with the check passing on the full tree. It was amended at once, to what it was meant to hold, before anything else built on it.
- `stencil.rs` went into the final pass's commit unused, and came into use in the next.

## Left out, on purpose

| What | Why |
|---|---|
| Streaming geometry out of memory | Proposal 0089 sets out why, and it's for the human to review |
| SMAA's tables | `r_aa 2` searches edges with no tables, and a side by side showed no need. Proposal 0082 |
| Jumps, water and lifts in paths | The human set these for later |
| Renaming the Sol crates | The names are the human's to choose, and it's no part of this work |
