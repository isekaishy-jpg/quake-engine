# Open research questions

Questions the research so far has left open. Each names what's needed. Affected work waits for the answer.

| Question | Needed | Affects |
|---|---|---|
| How should movement stay out of solid with `box3d` rays, which ignore a start inside a shape? | Surface margin and stuck handling, tested on real maps | Player movement |
| Do rebuilt solids work for BSP2 maps? | Run the rebuild on `mg1` and `mg3` through `qbsp` | Collision |
| How do doors and lifts work as `box3d` bodies? | Kinematic brush models, tested in motion | Collision |
| How do sweeps against other entities combine with hull rays? | A method and a comparison | Collision |
| Is `qbsp`'s lightmap atlas usable as it stands? | Run it, and check the output | Renderer |
| How do `renet` and its transports behave under loss and latency? | A test with injected loss | Transport |
| What are multiplayer traffic levels? | Measurements once entities exist | Replication |
| What exactly did CS2's sub-tick and Overwatch's command buffer do? | Primary sources. Only third-party accounts were found | Weapon timing, client clock |
| How does Quake Live's `g_lagHax` work? | A reliable source. Only anecdotes were found | Lag compensation |
| How much launch delay and correction feels right? | Testing in play | Projectiles |
| How does reduced-mode rendering reach the preview of a minimized window, which may have no surface to present to? | A test once frames draw, by hovering and pressing Alt-Tab. Routes: keep the window composed by handling minimize ourselves, with `DWMWA_CLOAK`, or the [iconic calls](windows-previews.md) | Platform, Renderer |
| Does one message per raw event keep up with an 8 kHz mouse? | A measurement on real hardware, then perhaps a dedicated input thread | Platform |
| What's the product's real name? | The human's decision | Packaging |
