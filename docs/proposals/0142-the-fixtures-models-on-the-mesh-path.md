# Proposal 0142: the fixture's models on the mesh path

Status: accepted and implemented, 2026-11-07. The human's decision, on [what meshes left undone](../research/mesh-limits.md): "just move it onto our path from the old path". This reverses item 12 of [proposal 0140](0140-meshes-at-run-time.md).

## Research

- **Why item 12 said no:** it was slower, and the pictures would differ. The first was mostly a poor vertex format, now fixed. What's left of it is small next to what pixels cost.
- **What the old path does:**
  - Two poses are blended in the vertex shader, in each pass that draws the model, from bytes scaled and offset.
  - Light is worked out per corner, by Quake's rule, from an ambient and a shade the client gives.
  - A skin's alpha says what's fullbright, and the fragment shader goes by that.
  - The view's weapon has its depth squeezed, to stay in front.
  - Triangles wind the other way from a mesh's.
  - It has a hull pipeline, and shadow maps have a pipeline of their own.
  - Models belong to the renderer and last across levels, where moving meshes belong to a level.
- **Corners are stored unshared,** three to a triangle, so a corner on six triangles is kept six times. Moving those through a compute pass would be paying for that.
- **What moving meshes do wrong for this:** a moving mesh out of view is left out of the frame's list, so it can't cast a shadow. Alias models cast theirs from anywhere, so that can't come across as it is.

## Design

**The look is kept, and the way there isn't.** From the corners on there's one path: the compute pass writes the same 16-byte corners, and they're drawn with the same three streams as a mesh, by the same outline and the same shadow pipeline. All that stays of alias models is how they're lit: a fragment stage of their own, and a short vertex stage for Quake's light rule. Pictures of the fixture are what it's for, so they mustn't change.

- **Poses are a kind of job.** `cs_poses` blends two poses into moved corners, in the model's own space, so scale and offset are done with once. The model's matrix goes in the instance stream, as it does for a mesh.
- **A dispatch for each model, not each copy.** Poses belong to models, which last across levels, so each has a small set of its own. Copies of a model are next to each other in the frame's list.
- **Corners are welded** when a model is added, by vertex and UV, and drawn by index, with the winding turned to match a mesh's.
- **What's left of `EntityDraw`:** nothing changes for the app. It still says model, poses, blend, skin, place and light.
- **Light rides in the instance stream,** where a mesh keeps where its light is: ambient, shade, and whether it's kept in front.
- **Moved corners don't depend on being seen.** Everything is moved, and what's in view is a flag on the job. Only what's in view is drawn in the scene, and shadow maps take any of it. That fixes moving meshes too. Models get culled against the view for the first time, by the reach shadows already use, and what's kept in front never is.
- **Room:** a frame's corners go to 1,048,576, which at 16 bytes is less memory per corner than before, and its meshes to 4096. Past that a model draws in its first pose, as a mesh draws at rest.
- **Going:** `vs_main` and `vs_hull`'s pose blending, the alias hull pipeline, the figure shadow pipeline, `vs_figure`, the push constants, and the pose and UV bytes as they were.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** alias models get the compute pass's cost in exchange for welding, culling and paying once for all passes. Normals go through bytes once more, which can move light by about one step in 255.
- **Rejected:**
  - Lighting alias models as meshes, per pixel. It's simpler, but the fixture would stop looking like itself.
  - A vertex stage that blends poses on the mesh path. That's the old path under a new name.
  - One big buffer for all poses. Models arrive one at a time, so it would have to grow, and a set each costs a few dispatches.
- **Acceptance criteria:**
  - All 79 of the game's models, in fixed poses, compared with the old path: plain, turned, first pose, kept in front, with an outline, and with a shadow. Differences stay within a step or two of light, and edges.
  - No worse in `gpu_times` in a real level.
  - A moving mesh out of view still casts a shadow.
  - The old pipelines are gone.

## Review

Built as designed. There's one way of moving and drawing a model now, and the old pipelines are gone. What stays of alias models is a short vertex stage for Quake's light rule, and a fragment stage for the skin's fullbright.

### The pictures

All 79 of the game's models, in 474 fixed pictures, drawn on the old path first and again on the new. The old path's pictures came out byte for byte the same on two runs, so any difference was the move's.

| Picture | Count | Pixels off by more than 1 | Most any pixel was off |
|---|---|---|---|
| Plain | 79 | 0 | 1 |
| Turned, with more shade | 79 | 0 | 1 |
| First pose, no blend | 79 | 0 | 0 |
| Kept in front | 79 | 0 | 1 |
| With a shadow | 79 | 0 | 1 |
| With an outline | 79 | 7 | 255 |

386 of the 474 are identical. The rest are a step of light in 255, from normals going through bytes once more. The seven pixels are one each in seven outline pictures, on the line's edge.

### What it costs

Eight of the game's monsters at 1280 by 720, in a crowd that animates, on the GPU, on both builds. These are the deform, shadow and scene spans added together, in ms.

| Models | Old, no shadows | New, no shadows | Old, one shadow light | New, one shadow light |
|---|---|---|---|---|
| 32 | 0.023 | 0.045 | 0.117 | 0.124 |
| 128 | 0.069 | 0.099 | 0.715 | 0.438 |
| 512 | 0.833 | 0.645 | 3.154 | 1.603 |

**It isn't free.** A small crowd with no shadows is 0.02 to 0.03 ms slower, which is what the compute pass costs to run at all. With a lot on screen it's quicker, and with a shadow light it's up to twice as quick, as poses are blended once and not for each of a light's six faces. Some of this is welding and culling, which the old path could have had, so it's not all down to the compute pass.

The waving arms read the same on both builds, at 1.31 and 1.32 ms, so meshes didn't pay for it.

### Found on the way

- **A moving mesh out of view cast no shadow.** It was left out of the frame's list. The new test fails with that put back, with the same light with and without the mesh.
- **The first comparison of speed was of a build against itself.** `cargo test -p engine_app` rebuilds the game, so the copy kept as the old one was already new. It showed up as two sets of numbers too alike. The numbers here are from the commit before, built again by stashing the change.
- **The GPU's clocks matter.** One build read 1.3 ms and 2.1 ms on the same scene, depending on what state its clocks were in. So only runs in the same state compare, and one run proves nothing.
- **A test passed with its bug put back.** A figure dropped from the frame still cast a shadow, from its first pose. Its first pose is a point now, so only moved corners can pass.

### What's still to do

Looking at real levels in play showed items and the view's weapon as they should be, but a person should still play some with monsters fighting. That's the one thing the fixed pictures can't say.

