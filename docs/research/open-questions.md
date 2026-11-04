# Open research questions

Questions the research so far has left open. Each names what's needed. Affected work waits for the answer.

| Question | Needed | Affects |
|---|---|---|
| How do doors and lifts work as `box3d` bodies? | Kinematic brush models, tested in motion | Collision |
| How do sweeps against other entities combine with hull rays? | A method and a comparison | Collision |
| Is `qbsp`'s lightmap atlas usable as it stands? | Run it, and check the output | Renderer |
| How do `renet` and its transports behave under loss and latency? | A test with injected loss | Transport |
| What are multiplayer traffic levels? | Measurements once entities exist | Replication |
| What exactly did CS2's sub-tick and Overwatch's command buffer do? | Primary sources. Only third-party accounts were found | Weapon timing, client clock |
| How does Quake Live's `g_lagHax` work? | A reliable source. Only anecdotes were found | Lag compensation |
| How much launch delay and correction feels right? | Testing in play | Projectiles |
| Does one message per raw event keep up with an 8 kHz mouse? | A measurement on real hardware, then perhaps a dedicated input thread | Platform |
| What's the product's real name? | The human's decision | Packaging |
