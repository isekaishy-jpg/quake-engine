# Proposal 0011: the platform host boundary

Status: accepted, 2026-11-03, increment 1.

## Research

- [Proposal 0001](0001-architecture.md) requires `solapp` to stay private to `engine_platform`.
- populous-reborn's `reborn_platform` wraps `solapp` in 914 lines with its own `PlatformApp` trait, frame record and input events. It's proven in a running game, and it's the closest working example.
- `solapp` must run on the main thread, and requires `started`, `frame`, `service` and `stopping` to be handled on the owner thread.
- Keys arrive as scan codes. Naming keys by physical position makes bindings independent of keyboard layout. populous-reborn already has a scan code table.
- Pacing: `RendererManaged` suits a renderer that blocks on present. With no renderer yet, unlimited frames would spin a CPU core. `Forever` gives a nanosecond interval, while `Stock` rounds to whole milliseconds.

## Design

`engine_platform` exposes `PlatformApp`, `run`, `WindowConfig`, `Frame`, `FrameOutcome`, `ServiceOutcome`, `InputEvent`, `MouseButton`, `key_name`, `AppError` and `PlatformError`. No `solapp` type appears in that API.

- `PlatformApp::started` runs once, `service` runs for the simulation, `frame` runs for presentation, and `stopping` runs at shutdown.
- `service` receives time since start and returns how long until it wants to run again, or a stop.
- Input is delivered as engine-owned events named by physical key. Proposal 0015 changed this from queueing for frames to delivery on arrival.
- `WindowConfig` carries title, size, the mode from proposal 0020, an optional frame cap and the reduced frame rate from proposal 0014. The cap uses `Forever`, and no cap uses `RendererManaged`.

## Proposal

- **Selected approach:** adapt `reborn_platform`, adding a service for the tick. Cursor images, text sessions and display modes wait until something needs them.
- **Tradeoffs:** a thin wrapper now means more API work later, which is the cost of keeping `solapp` private.
- **Acceptance criteria:** the crate-graph test passes, no public item names a `solapp` type, a window opens and closes cleanly, and keys reach the application by name.
- **Future extension:** the renderer gets a window handle in increment 2. A different host replaces this crate alone.

## Review

See [review 0003](../reviews/0003-increment-1.md).
