# Review 0016: increment 12

Status: review record, 2026-11-05. Reviewed: proposals 0092 to 0095, and what increment 11's report named as not done.

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
| 8 | Bounced light drew on 1024 samples across a whole map, so on a big map identical walls came out up to twice as bright as each other. `wide_world`'s far colours showed it | A tile keeps 128, and a point looks at nine tiles. A test that a room's bounce doesn't depend on a map's size fails without this |
| 9 | Two versions of that test passed without the fix, because regular rooms and direct light hid the difference | Rewritten until it failed without the fix |
| 10 | A redraw of shadows walked every face in the world | A shadow draws only the tiles its light reaches |

## Process

- Four commits failed `cargo xtask check` first, on a lint in a test, unformatted Rust, a lint on a type and a lint on a cast. The gate held all four back, and each went in once it passed.
- The real map suites were first run without `QUAKE_DIR`, and every test failed in no time at all. Run with it, they passed.
- A shell command with a stray `cat` stalled until it was stopped, and the test fix it carried had to be run again.
- A session started for the lints never ran, as its setup didn't finish, so none of that work has happened.

## Left for the human

| What | Where it stands |
|---|---|
| Baking big maps | `wide_world` takes 46 minutes, about 11 seconds a tile against 1.7 for `tile_world`, so something in a bake grows faster than a map does. What it is hasn't been measured |
| The host, worker and cache crates under the workspace lints | Measured at 1,703 items, about 800 of them doc comments to write by hand, with 47 uses of `unsafe` to allow by name. The open questions keep this apart from the rename. The test layout rule would also need test seams moved out of 189 places under `src/` |
| Jumps, water and lifts in paths | The human set these for later |
