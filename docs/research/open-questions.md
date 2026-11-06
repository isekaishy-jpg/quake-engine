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
| Are the host, worker and cache crates held to the same bar? | A bug and quality review of their 31,000 lines, and moving their tests out of `src`. They are under the workspace lints now (proposal 0096) | `engine_host`, `engine_worker`, `engine_cache` |
| How should particles, water and shaders be built? | The workflow, once other decisions are made. [The direction so far](effects-water-shaders.md) is set, and nothing is designed | Rendering, editor, content |
| How should full mesh support work? | The workflow, from [the direction so far](mesh-support.md): glTF import, generated light map UVs and generated collision shapes are chosen, and the rest is open | Meshes, baked light, rendering, collision, prefabs |
| What's the texture standard, and how is it packed? | Wide research, then the workflow, from [the direction so far](texture-standard.md). Linear colour, block compression and compiling ahead are chosen. The next assets wait on this | Textures, materials, shaders, assets |
| How should regions, areas, zones and sub zones work? | Claude's research into how to make it work, then the workflow. A named part of a map that things can hang from: showing its name as text, music and ambient sound, ambient light and so on, with sub zones inside zones | Maps, the editor, audio, light, UI |
| What should the editor be like to work with? | A full workflow pass by Claude when the list reaches it, as a better starting point, then going over it in detail with the human. Without an agent, anyone should be able to make maps quickly and easily, including people with disabilities or cognitive impairments. That doesn't mean removing advanced options or cutting agent workflows. Today it works, but its UX is poor | The editor |
| How should the engine's own UI be built? | The human's design, then the workflow. A real text renderer, nine-slice frames, menus and animated pictures, for the art in `content/ui` and `content/fonts`. Cultist Sans is proportional, so the console needs its own answer | UI, the console, menus |
