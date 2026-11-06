# Proposal 0102: frames that keep up

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

The human saw 126 to 143 FPS at 2560 by 1440 on a GTX 1070, expected hundreds, and saw hitches moving quickly through Vesper Deep. Everything here was measured on Vesper Deep at about that size, with timers taken out afterwards.

- The CPU spent 4 to 6.5 ms of each frame waiting for the GPU, and only then started the next. One frame was in flight, so the two took turns, and a frame cost the GPU's time plus about 1.3 ms of the CPU's.
- GPU time goes up with pixels, which explains the gap the earlier measure left: about 4 ms at 2000 by 1125 is 6 to 7 ms at 2560 by 1440.
- The final pass took 1.1 to 3.4 ms. Switching its parts off one at a time showed stencil rays were 2.6 ms of that in the market, where the view is inside a beam. Fog was 0.4 and pools 0.3.
- Surfaces took 3 to 3.8 ms over a flat colour. A flat colour and a light map alone cost the same, so it wasn't light samples. Anisotropy and moving lights together were under 0.3 ms.
- `light()` ran twice a pixel on anything with specular, which is nearly everything: 0.55 ms.
- One shader does everything a surface can do. Compiling out what plain surfaces don't use saved 0.65 ms, though none of it runs on them.
- Every pixel looped over up to 16 capsules, though a capsule shades only what's close behind it: 0.45 ms.
- A hitch came with each tile of light. A 1024 tile took 8.5 to 11.8 ms to write, two a frame: 12 MB was gathered into one vector, copied again, and the GPU was waited on. Packing it was only 2.7 ms. The first second after a load had ten frames over 12 ms, the worst 35 ms.

## Design

- **Two frames in flight.** Each has its own fence, semaphore and command buffer, and its own copy of what the CPU writes each frame: particles, canvas vertices and visible indices. A frame's indices are rewritten only if they're behind the view.
- **Uniforms are copied by the GPU.** Descriptors point at one uniform buffer, in device memory, and many sets share it. Doubling it would mean doubling the sets. Instead each frame writes a host buffer, and the first thing its commands do is copy that in.
- **Frames don't overlap on what they share.** They draw to the same targets, so a frame starts with a barrier on the one before.
- **Light in the air at half size.** Stencil rays are soft, and they already read a blurred mip. They draw into their own target, on the post shader and set, and the final pass adds them with a bilinear read.
- **`light()` once.**
- **A plain surface shader.** `material.wgsl` has `const PLAIN: bool`, and the same source compiled with it true drops parallax, triplanar, hex tiles, flow, layers, detail, emissive and shading models. A batch with none of those uses it.
- **Capsules in the clusters.** Moving lights go from 64 to 48, the last 16 bits of a cluster's mask are capsules, and a pixel looks only at those whose shade may reach it. A sphere round the shade stands in, as it does for lights.
- **Tiles without stopping.** A tile's write is queued, and the next frame records it first. What it reads is held until that frame finishes. Parts go straight into the staging buffer, an atlas with nothing to say is skipped, and one tile shows a frame.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** input is up to a frame older with two in flight. Moving lights stop at 48, which no level comes near. The plain shader adds a compile at startup until shaders are cached (sprint item 6).
- **Acceptance criteria:** at about 2560 by 1400 on the GTX 1070, Vesper Deep's square holds over 200 FPS and its market, quay and terrace over 165. No frame in the first second after a load passes 15 ms. Capsules still shade what's behind them. Beams look as they did.
- **Future extension:** frame rate is looked at again in the final performance sweep.

## Review

| Where | Before | After |
|---|---|---|
| Square | 165 FPS | 216 to 224 |
| Market | 128 | 170 to 175 |
| Quay | 140 | 176 to 186 |
| Terrace | 132 | 174 to 186 |
| Worst frame after a load | 35 ms | 13.4 ms |

Beams in the minster were looked at, and are as soft as they were.
