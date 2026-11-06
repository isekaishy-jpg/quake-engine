# Review 0016: increment 12

Status: review record, 2026-11-05. Reviewed: proposals 0092 to 0097, what increment 11's report named as not done, and two real levels.

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
| 11 | A brush cut every face its planes crossed, whether it touched them or not, so a level with round columns came to 227,794 faces. Box-built maps hid it | A face is only cut if some of it is inside the brush. That level is 5,300 faces, and `showcase` halved |
| 12 | A tree of 5,000 brushes asked for 40 GB | Space is halved until each part holds few brushes |
| 13 | One solid half a thousandth of a unit thick hung the hull builder for ever | Slivers are refused, in any direction |
| 14 | The grid took the next stair up for a wall, so nothing could path down stairs | It looks for room from a step's height |
| 15 | A script's search for a goal nothing reaches flooded the grid every tick | Searches stop at 60,000 places, and chunks are found by key |
| 16 | Every face was tested against every brush, so loading took 24 seconds | A brush only meets its neighbours, and it takes under 5 |
| 17 | A map's solids each worked out the mass of the whole world | They don't, as they don't move |
| 18 | Prefabs could only face one way, and an outline drawn clockwise turned a solid inside out | They turn, and outlines work either way round |
| 19 | The human saw olive-green patches round torches. Each colour channel was cut off alone, so red stopped and green rose, in the bake and in the shader | Light too bright comes down as a whole. Tests in the bake and on screen fail without this |
| 20 | The human saw clouds in blocks and pieces. Their noise hashed with `sin`, which breaks up at large numbers | An integer hash, layers turned against each other, and fine layers faded low in the sky |
| 21 | Torches 12 units off a wall, and too bright, filled it flat with no falloff | They stand further off and burn lower |
| 22 | The human saw a torch leave a circle on each wall of a corner, with nothing between them. Light fell with the full cosine of its angle | Half of that, as ericw-tools does (proposal 0098) |
| 23 | The bump response was a plain ratio, so a surface nearly edge on to its light glared | It's held to 1.5, over a floor under the flat light |
| 24 | Derived relief at full strength made stone look like modelled clay | The packs use 0.42 |
| 25 | A mirror that found nothing showed the sky, even in a cave under it | It only does if the view is under the sky |
| 26 | Big see-through quads were split by the order of faces, which showed as hard diagonals | Mist is small puffs in front of the water, and the body of a fall is opaque |
| 27 | `wide_world` took 40 minutes to bake: every ray asked about every mesh and pane of glass, and sun rays crossed the whole tree | 2 minutes 17 seconds (proposal 0101) |
| 28 | Drawing bounced light from a sample in a wall darkened corners | The ends of each row are always worked out |
| 29 | The human saw lines all over. Every picture in the packs came with a thin dark line along its last row and column, which runs through the stonework at each repeat, and normal maps made it stand out. A first check had flagged 56 pictures and passed most of them over as lines meant to be there, which was wrong | `content/heal_seams.py` mends all 96 from the pictures as delivered, and their maps were worked out again |
| 30 | Four crates were outside the workspace lints, with 1,684 warnings | All four are under them, with `unsafe` allowed by name and a safety comment on each block (proposal 0096) |
| 31 | An out of date bake was ignored without a word, so a level came up bright and flat. The final captures and route tests caught every map like this after one rebake. A fresh bake of the same inputs gave a different hash, and why wasn't found | The game now says so in its console, and every map was baked again and checked |
| 32 | The human saw black specks on the well in Vesper Deep. A moving light's shadow map has about 3 units to a pixel out there, and on a surface at a shallow angle a fixed bias couldn't stop it shading itself | The shadow is tested from just off the surface, by the size of a pixel there, and more so edge on. Each face of a moving light's shadow went from 256 to 512 pixels, filtered over nine samples with no noise, which also cleared the stair steps and stripes |
| 33 | The human saw enemies under geometry. Three started in solid: two on Ember Keep's balcony, inside its floor, and one in a house in Vesper Deep | They've moved, and a test now holds every enemy in both levels to a clear start over a floor |
| 34 | The human saw lava flicker when turning. A ray stopping on an opaque liquid only touches its pool's box, and rounding decided per pixel whether it was mirrored | A touch doesn't count. The brightness of a slow turn over the forge's lava went from jumping by 3 to changing smoothly |
| 35 | Vesper Deep took 12.5 seconds to load. Box3D ran a self-check of the whole physics world twice for each shape, even in release builds where its assertions are gone, and went through every hull to add one | The check is skipped in release builds and hulls are found by hash, so it takes about 5 seconds. Both are patches to `vendor/box3d` |
| 36 | The human saw 67 FPS maximized, on a GTX 1070. GPU timers showed the opaque pass at 6.5 to 9.6 ms of 8 to 12. Surfaces are drawn by texture, so what lay behind what's in view was fully shaded, and a `discard` in the material shader held back the test that skips it | A depth pass goes first, and materials then shade each pixel once. The opaque pass takes about 1.7 ms and the frame about 4 ms at 2000 by 1125 |
| 37 | The human saw baked shadows step sideways, in notches and blocks. A sample past a face's edge was pulled back onto that face, so two faces disagreed where they joined, by up to a sample | A sample stays where it is unless something stands between it and the face. A test with the join between samples fails without this. The first report of this as fixed was wrong: see 38 |
| 38 | The human saw the same steps after 37 was reported fixed. All 20 bakes had been made with 37 switched off. Plain `cargo build` builds only `engine_app` (`default-members`), so `engine-compile.exe` was whatever `cargo test` last left, and that was a run testing the fix switched off. `BAKE_VERSION` matched, so nothing said so. A bake made with the fix off matched the old file byte for byte | Build the tool by name before a bake. `light_runs_on_across_joins` measures every join in the Ember Keep and Vesper Deep, and fails at 8,328 steps on a bake made the old way |
| 39 | With 37 really in, faces still disagreed: 459 places on a sun shadow and 2,638 in other light in Vesper Deep. Each face was finished alone, and nothing compared it with its neighbours afterwards | After a face's light is done, faces on one plane and mapping share their samples. Whichever sample is on its face owns the place, and others join it if they're in sight of it or their faces meet there, so light doesn't come through a wall. Vesper Deep has 5 places left on one pair of faces and the Ember Keep 21 on four. Each rule has a test that fails without it. Bakes take no longer |
| 40 | The human saw repeating pictures that mostly lined up but had no blend, or looked off at the repeat. `heal_seams.py` only drew a few rows again, as a mirror, which left the step in tone and read as a strip of inkblots | It now works from delivered pictures. Those that repeat freely have the line trimmed, tone spread across the picture, and a join along the best match with a crossfade of real picture. Those come out at 232, and stay there: scaling them back to 256 made all that's in them 13% bigger and softer, which the human saw, so they repeat about 9% sooner. Patterns only have the line's rows bridged, trims and two kinds of planks with rivets at a joint only sideways, and what's shown whole is left alone. All 96 were looked at, and a repeat is now about as strong as any row inside the picture, from 2 to 5 times that. A real ledge or strap on the repeat line reads higher and is meant to be there |

## Process

- Four commits failed `cargo xtask check` first, on a lint in a test, unformatted Rust, a lint on a type and a lint on a cast. The gate held all four back, and each went in once it passed.
- Findings 19 and 20 were in captures for some time, and were passed over until the human named them. Reading captures has to mean looking for what's wrong.
- Commit `aaf9474` says the city plays, when its route test was still failing at the time. That test is ignored by the check, as it needs a bake. It passes now, after later commits.
- A crashed script left a game running that held the build's output open, and a script of mine named `bisect.py` hid a Python module. Both cost time, and neither touched the repository.
- The real map suites were first run without `QUAKE_DIR`, and every test failed in no time at all. Run with it, they passed.
- A shell command with a stray `cat` stalled until it was stopped, and the test fix it carried had to be run again.
- A session started for the lints never ran, as its setup didn't finish, so none of that work has happened.

## Left for the human

| What | Where it stands |
|---|---|
| Repeats in pictures | Strong features show at every repeat. The human finds it fine for now, and part of it is a job for the assets, not the engine |
| Layers and masks | 2 of 128 materials use layers, and hex tiling, parallax, triplanar and detail have one each. Vesper Deep has no paint or decals, so nearly every surface is one repeating picture. Paint is held at light sample spacing, so its edges are soft |
| Which light gets what, by order | The first crisp light in a level gets the sharp edge, and the sun is added last, so a crisp lamp would take it from the sun. Past three layers, a light with a new style is dropped without a word. No level has two crisp lights or a second extra style, so neither shows yet |
| Joins between faces at an angle | Sample sharing covers faces on one plane. A shadow crossing a fold or a curve's facets isn't matched |
| Lamp shadows and curved surfaces | Thin things bake as blocks under lamps, and curves show bands and sawtooth by their facets, whether brushes or meshes. See [Shadow rendering](../research/shadow-rendering.md) |
| Moving lights' shadows | Sawtooth and bands still show at a large window size, on any surface near a moving light. Shadow maps at this size can't remove them, and the way forward is the human's call |
| Map loading | 3.7 seconds of Vesper Deep's 5 is compiling the map from its text at each load |
| Vesper Deep's foundry lava | Its two channels are under the foundry's floor, so they can't be seen. A level fault, not fixed yet |
| Enemies that seem broken | The test only covers where they start. Anything wrong once they move, or with the stand-ins in the city, hasn't been found |
| Water's reflections | Screen-space reflections on water show a fine stipple, from their per-pixel jitter. This belongs with the water work |
| Those crates' tests under `src` | 41 files still hold `#[cfg(test)]` code, which reaches private seams. Moving them needs a decision on those seams |
| Real people for the city | Stand-ins now, as the human said they could be |
| Jumps, water and lifts in paths | The human set these for later |
