# Proposal 0143: what meshes left undone

Status: proposed, 2026-11-07, from [what meshes left undone](../research/mesh-limits.md). The human's rule for this: a limit is a research target, and for what we don't have, research decides if it's needed, and what's likely to be wanted later gets done now.

This has a design for each target, and each is reviewed below as it lands.

## Design

### The depth pass

Solid parts of copies and moving meshes go through the depth pass, by the rule faces have, and their main draw then doesn't write depth. Masked parts, parts whose shader moves the surface, and details that fade stay out. `vs_mesh` runs the same maths in both, and `clip` is invariant, so the two agree.

The fixture's models stay out. Their pixels are one texture read, and what a depth pass saves goes by what a pixel costs.

### Fixes with no open question

- **Static copies with rest weights:** applied at compile time to what a copy draws.
- **Parents before children:** `follow_attachments` goes in order, with a loop broken rather than followed.
- **The turn in places and effects:** `point_on`, and an effect on a model.
- **Eight morphs:** only what's set is sent.

### Measure, then decide

A copy's triangles on the CPU, and what posing costs the CPU.

### Building

- **A coarse level moves fewer corners:** corners sorted coarsest level first, so that a level is the first so many, and the compute pass stops there.
- **Hit volumes on joints:** capsules named on joints, tested by `traceline` after the box says it's near. Movement keeps its box.
- **Sorting what's see-through:** with the world's, in its far to near list, which is there already.
- **Vertex colours:** `COLOR_0`, in the stream that doesn't move, multiplying the base colour, and open to surface shaders.
- **UV and colour targets:** that stream moved too, for only the meshes that have them.
- **Hit volumes go by the rewound time,** through what proposal 0048 built.

### Trying

Clips with `net_fake_loss` at 2%, and a client joining part-way through a clip.

## Review

### The depth pass

Built as designed. 24 of the test arm overlapping in the proving ground, against the build before, five runs each in turn, taking the lowest as a GPU's clocks move the rest about:

| | Before | After |
|---|---|---|
| Moving meshes drawn | 0.370 ms | 0.141 ms |
| The whole scene | 1.293 ms | 0.736 ms |

So it nearly halves the scene, depth pass included. Pictures of the arms are as they were. `a_moving_mesh_hides_what_is_drawn_after_it` fails if a mesh is left out of the depth pass but still doesn't write depth, with the model behind showing through.

### Fixes with no open question

- **Static copies show their rest weights.** A baked placement and a copy both do, and with that done they carry no morphs. A collision shape that an import fitted stays fitted to the mesh as made.
- **What's held to what's held no longer lags.** What holds something goes first. A ring of things holding each other has nowhere to start, so it stays a tick behind and doesn't hang.
- **A place turns with its model,** by the same sums as a place on a joint.
- **Eight morphs,** and only what's set goes over the network, as a count and pairs. So this costs a thing with none one byte less than before. The protocol's number goes up.

**One part of this can't land yet.** An effect on a brush model can't turn with it, as a client doesn't know how it's turned: `ModelInstance` has only an offset, as brush models don't turn when drawn or collided with at all. That's what a door that swings would need. It's written into item 151, on this same list, with the rest of what turning needs, and places are ready for it. It isn't research to skip: nothing can show the fault until then.

### A copy's triangles on the CPU

Measured on 101 copies of the test arm's 784 triangles: 160 bytes a triangle for each copy, 12.7 MB in all, in faces, points and leaf lists. So 500 copies of 2000 triangles would hold 160 MB to do what one face each can do. That's worth fixing.

**The bake needs a copy's triangles, and nothing after it does.** `Map::drop_copy_faces` keeps a copy's first face, which stands in for it in each leaf it touched and says where its light is, and puts right everything that numbers faces. `engine-compile` does it after the bake, so that the files shrink too, and writes geometry after that. A level does it when it loads as well, which does nothing to one that's been through this and covers older files.

The proving ground's faces went from 890 to 295, with five copies. Its barrels are drawn and lit as they were, and two shots of the same build differ by more than before and after do, as the fire moves.

**A test proved nothing at first.** With leaves left as they were, the tests still passed, as a level of one leaf can't tell. `leaves_keep_what_they_held_when_a_copy_loses_its_faces` compares what each leaf holds, by what it is and not by number, on a level with several, and fails with that bug.

### What posing costs the CPU

Measured for a clip with every joint keyed: 2 µs for 20 joints, 6.6 µs for 60, and 11.8 µs for 120, sampling, placing and making a palette. So 100 characters of 60 joints are 0.66 ms a frame. That's fine for most scenes and matters in a crowd, so it's taken up with level of detail below.

### Level of detail for what moves

A moving mesh's corners are sorted when it's compiled, so that each level is the first so many, coarsest first. Levels are chosen by the same rule as copies, by the client, which keeps each entity's level from frame to frame, and the compute pass stops at that level's corners. Shadow maps use the same level, as what's past it hasn't moved. It needs no change to any file: the counts come from the indices when a level loads.

Far things also pose less often, 30 times a second at the third level and 15 at the fourth, spread out by who they are so that a crowd doesn't pose all at once. What places them still changes every frame, so only limbs step, and a change of clip poses them at once. That's what comes off the CPU.

96 of the test arm, made small with a wide view, as the proving ground isn't long enough to walk away from them:

| | Near | Small |
|---|---|---|
| Corners worked out | 0.038 ms | 0.027 ms |
| Drawn | 0.510 ms | 0.106 ms |

Most of a coarse level's saving is here in drawing, as the test arm is small to begin with, and the fixed cost of the pass is about 0.011 ms.

- **Three tests were checked by putting their bugs back** properly, on the second try: joints left behind by the sort, morphs left behind, and no sort at all. The first try proved nothing, as the test mesh had no levels and failed either way.
- **How a change of level looks in motion** is for a person, with a real model, at stage 17.

### Hit volumes

A moving mesh gets a capsule on each joint when a level compiles, round the corners that joint carries most, so an author has nothing to do. A line that reaches its box tests those, posed as it stood at the shot's time, and goes by them and not the box. What has size goes by the box as it always did, so moving is unchanged. A mesh with no capsules goes by its box too. `traceline` says which joint, and scripts have it as `trace_joint`, so a game can tell a head from a foot.

Tried in the game, firing from a script at the Blender arm, which has a box 160 tall. Shots at three heights hit `root`, `middle` and `end`, stopping on its surface. Shots above it and beside it, inside the box, went through.

- **Rewinding** goes by what proposal 0048 built. A rewound shot poses the figure at the rewound time. If its clip has changed since, that's taken from the start of the clip that's playing now, as only a clip's start is kept.
- **A bug found by its test:** `end_rewind` left the rewound time set, so that every shot after the first would have gone by the old pose.
- Four other bugs were put back, and each fails a test: joints ignored, things with size going by joints, the pose ignored, and a capsule too thin.
- **A capsule's as wide as the widest of what it holds,** so a shot can hit just beside a thin part of a limb. Better that than a shot through a limb missing.

### Tangents

Not built, as [the research](../research/mesh-limits.md) measured that they aren't worth it. What it found was a bug: `surface_frame` scaled tangent and bitangent together, so a stretched texture's bumps leaned less one way. Each has its own length now. `a_stretched_normal_map_leans_as_far` goes through a matcap, which shows a lean as a colour, and fails on the old shader, where the same map read 34 and 86. Two earlier tries at that test proved nothing: a moving light doesn't go by a normal map, and squeezing a texture also moved its light off the map.

