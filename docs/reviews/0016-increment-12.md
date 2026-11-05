# Review 0016: increment 12

Status: review record, 2026-11-05. Reviewed: proposals 0092 to 0094, and what increment 11's report named as not done.

The human pushed increment 11, and asked for what was left over, the rename included, to be finished.

## How it was checked

- `cargo xtask check` before each commit, with no commit made on a failed check.
- GPU tests for figure shadows and SMAA, on a real device.
- Real map and netplay suites after the rename, and again at the end.
- Captures in `showcase` of figure shadows, hull outlines, a shade with moving shadows off, each anti-aliasing setting, and a `mega` surface.

## Findings

| # | Finding | Handling |
|---|---|---|
| 1 | Reading a bake moved a `mega` surface's light a second time, so those surfaces went black in the game. Increment 11 had this, and no map used `mega` to show it, though the test maps were meant to exercise everything | Light moves once, when baking. A test reads a bake back, and fails without the fix. `showcase` now has a `mega` pillar |
| 2 | Review 0015 and proposal 0082 said a side by side showed no need for SMAA, and it had no SMAA in it. Built and compared, the edge search turned a shallow edge into a sawtooth | SMAA replaced it as `r_aa 2`. Proposal 0093 has the comparison |
| 3 | A shadow redraw that waited for the budget was lost if everything then stopped, leaving a stale shadow | It's owed until it happens |
| 4 | A moving thing that disappeared left its shadow behind | Going counts as moving |
| 5 | Nearest-first redraws let something that never stops hold a far light back for ever, which figures would have made common | What's waited longest goes first |
| 6 | Parts for every nearby model could push the furthest of a crowd out of the shader's 16 shades | Parts go to the nearest while there's room |
| 7 | Capsules came from solid boxes, so corpses shaded nothing | Shades come from what's drawn |

## Process

- Three commits failed `cargo xtask check` first, on a lint in a test, unformatted Rust and a lint on a type. The gate held all three back, and each went in once it passed.
- The real map suites were first run without `QUAKE_DIR`, and every test failed in no time at all. Run with it, they passed.
- A shell command with a stray `cat` stalled until it was stopped, and the test fix it carried had to be run again.

## Left for the human

| What | Where it stands |
|---|---|
| Streaming tiles | `tile_world` bakes to 2.1 MB a tile on disk. All in memory, that's about 530 MB for a 16 by 16 grid and 2.1 GB for 32 by 32, and the largest grid allowed, 65 by 65, would be about 8.8 GB. It needs `Map` split by tile, through collision, the server, the renderer, visibility and bakes, and nothing today gets near that size, so it's left for the human to call, as proposal 0089 said |
| The host, worker and cache crates under the workspace lints | Measured at 1,703 items, about 800 of them doc comments to write by hand, with 47 uses of `unsafe` to allow by name. The open questions keep this apart from the rename. The test layout rule would also need test seams moved out of 189 places under `src/` |
| Jumps, water and lifts in paths | The human set these for later |
