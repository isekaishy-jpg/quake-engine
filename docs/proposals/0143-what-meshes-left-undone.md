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
- **Real tangents:** MikkTSpace, from the file or worked out when a mesh is imported, turned with its joints. Faces keep what they have.
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

