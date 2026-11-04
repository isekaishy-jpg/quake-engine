# Proposal 0028: moving brushes on the client

Status: accepted, 2026-11-04, increment 4.

## Research

- [The pre-research](../pre-research/stack.md#online-feel) says other entities stay interpolated one tick behind, while the local player is predicted.
- World faces are baked into static buffers with each face's vertices in place, so a moving model needs its position applied at draw time.
- `vulkane` has push constants, which fit a per-draw offset.

## Design

- `Snapshot` carries the brush instances, in full each tick. Delta encoding arrives with online play.
- New messages: `LightStyle`, `Print` and `SetView`, the last for a teleport turning the player.
- `Client` keeps the last two lists, interpolates for drawing, and predicts against the latest.
- `build_world` no longer places brush models. Each model keeps its own index ranges in a fixed part of the index buffer, and the world's visible set sits before them.
- `Renderer::draw` takes the instances, and draws each model with its offset as a push constant.
- `engine_client::LightStyles` turns patterns into brightness.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** all visible brush models are drawn, with no PVS check, which costs little at this count. A button's alternate texture isn't shown yet.
- **Acceptance criteria:** tests cover the new messages, interpolation, styles and per-model ranges. A GPU test draws a model at an offset. On a real map a door moves smoothly.
- **Future extension:** alias models take the same instance path.

## Review

See [review 0007](../reviews/0007-increment-4.md).
