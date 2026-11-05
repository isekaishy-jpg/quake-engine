# Open research questions

Questions the research so far has left open. Each names what's needed. Affected work waits for the answer.

| Question | Needed | Affects |
|---|---|---|
| How much launch delay and correction feels right? | Testing in play | Projectiles |
| Does one message per raw event keep up with an 8 kHz mouse? | A measurement on real hardware, then perhaps a dedicated input thread | Platform |
| How should door logic be revamped? | The human's design, after the port plays through. The port matches `doors.qc`, and it may feel better with sound | Game scripts |
| Should the original levels use the rerelease's coloured `.lit` light? | The human's decision. `load_level` already reads a `.lit` it can find, and `mg1` draws in colour. Every original level has one in the rerelease pak, which isn't mounted for `id1` | Game layout |
| Should the status bar and menus become scripts? | The human's decision. They're Rust in the fixture today, and only a canvas reaches the engine | UI |
| How do online play's numbers feel on a real network? | Play over the internet. Interpolation delay, the rewind limit and the 5% loss design limit are starting points | Replication, lag compensation |
| Are the Sol crates held to the same bar? | A bug and quality review of their 31,000 lines, and bringing them under the workspace lints, as proposal 0009 left open | `solapp`, `solworker`, `solcache` |
| What's the product's real name? | The human's decision | Packaging |
