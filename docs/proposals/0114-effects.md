# Proposal 0114: effects

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

[The note](../research/effects-water-shaders.md) has the human's direction: what an author, or Claude, can make matters most. A mix of graph and modules, with scripts attaching. The CPU and GPU, chosen by need. A particle can be a sprite, ribbon, beam, mesh, light or decal. Attach points for maps and prefabs.

- Particles were a port of Quake's: a position and a colour, in seven built-in motions, and a level couldn't define its own.
- [Niagara](https://dev.epicgames.com/documentation/en-us/unreal-engine/key-concepts-in-niagara-effects-for-unreal-engine) is a stack of modules on each emitter, with emitters that can talk to each other, and [its events only work on the CPU](https://dev.epicgames.com/documentation/unreal-engine/events-and-event-handlers-in-niagara-effects-for-unreal-engine?lang=en-US).
- GPU particles usually mean compute shaders and buffers of state. Dust, snow and rain don't need state: where one is follows from its number and the time, in a vertex shader, with nothing kept and nothing to synchronise.
- `Effect` is `Copy`, so it can't carry a name.

## Design

- **Effects are files.** `effects/<name>.eeffect` is a document of emitters, each a stack of settings: how they start, what moves them, how they change, what they are. Text is what Claude writes best.
- **A graph of emitters.** `on_death` starts another emitter where a particle ends.
- **The CPU** looks after what depends on what came before: curves, drag, turbulence, ribbons, deaths, up to 8192 at once.
- **The GPU** works out `gpu = true` emitters in the vertex shader from a particle's number, the time and one push of numbers. There's no buffer, and the CPU does nothing per particle.
- **Kinds:** soft sprites, ribbons as lines through where one has been, beams, flat marks, and lights, which join the moving lights.
- **Blending:** over, or adding, which `vulkane` needed a small patch for. Colours past 1 glow.
- **Attaching:** `type = effect` entities in a level, and `engine.effect(name, origin, entity)` in scripts, which follows that entity. A server names an effect by a hash, so `Effect` stays `Copy`.
- The original's particles stay as they are, in the same list and draw.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a visual graph editor wasn't built. A file and a table of its settings is what an author and Claude both use now, and the editor pass (item 18) is where a picture of it belongs. GPU emitters can't do what needs history. Compute would allow that, and nothing needs it yet.
- **Acceptance criteria:** an author can make fire, smoke and dust with no code, emitters can start each other, a script can start one on something moving, and great numbers cost the CPU nothing.
- **Future extension:** mesh particles, once the mesh path can draw instances that move. Bones to attach to, with skeletal animation.

## Review

A test level with `campfire`, `smoke` and 15,000 motes of `dust` was looked at. The first version's motes bunched at the middle of their volume, as a ball needs a cube root, and its flames added up to peach, so `campfire` was cooled.
