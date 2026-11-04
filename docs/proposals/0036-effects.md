# Proposal 0036: sprites, particles, lights and beams

Status: accepted, 2026-11-04, increment 6.

## Research

- `spritegn.h` and `Mod_LoadSpriteModel`: a header, then frames that are one picture or a timed group. Index 255 is a hole. Three sprites ship: `s_explod`, `s_bubble` and `s_light`, all flat against the view, but `R_DrawSprite` has five ways to turn.
- `r_part.c`: a particle is a position, a velocity, a palette colour, a place on a colour ramp and a way of moving. `R_RocketTrail` steps along by 3 units, but moves its start by only 1, so a trail bunches up at the start of each frame's path and depends on the frame rate. QuakeWorld fixes this.
- `D_DrawParticle` draws squares: 256 / depth pixels on a 320 wide screen, kept from 1 to 4. `GLQuake`'s are small round dots.
- `cl_tent.c`: each temporary entity is particles, a sound, and sometimes a light. Beams last 0.2 seconds, a new one from the same entity replaces the old, and they're drawn as model pieces every 30 units with a random roll. A beam from the player follows the player.
- `CL_RelinkEntities`: effect bits give a muzzle flash, bright and dim lights. Model flags give trails, and a rocket also a light.
- `R_AddDynamicLights` adds `radius - height - distance across` to each light map sample of a marked surface, with `minlight` cutting off the edge. `R_DrawAliasModel` adds `radius - distance` to a model's light.
- `R_TextureAnimation` swaps to `alternate_anims` when an entity's frame isn't 0. `Mod_LoadTextures` makes `+0` and `+a` frames two cycles, each pointing at the other.

## Design

- `engine_map::Sprite` holds pictures and frames, and `fixture_quake::parse_spr` reads them.
- `engine_client::Particles` ports the particle system, with no knowledge of the game. `engine_client::Effects` holds everything else: lights, beams, trails and the table of temporary entities, so one file knows the original's numbers.
- Trails carry their distance from one frame to the next, so spacing doesn't depend on the frame rate.
- The renderer takes `Drawables` for each frame, so adding to a frame doesn't change every call. A `Recorder` records a frame, for the window and for captures alike.
- Sprites and particles build their corners in the shader. Particles are one instanced draw.
- Lights go in the frame's uniforms, and the world shader adds them per pixel, working out the plane from derivatives. Models add them on the CPU.
- A snapshot carries the player's entity, effects and punch, because the player isn't among the entities.
- `ModelInstance` carries a frame, and the renderer swaps to `Texture::alternate`.

## Proposal

- **Selected approach:** the above, keeping software Quake's square particles, which suit the pixelled textures.
- **Tradeoffs:** lights per pixel can't reproduce the 16 unit steps of light maps. They're smoother, so their edge is faded over 16 units to stand in for the blur of those samples. Lights reach through walls, as the original's do, and adding a check would be easy. Up to 16384 particles live at once, where 2048 run out after two rockets. Colour-mapped explosions and the grappling beam belong to the mission packs and aren't here.
- **Acceptance criteria:** tests cover sprite files, a sprite's facing and holes, particle colour and size, dynamic light near and far, alternate textures, every temporary entity's effects, trail spacing at any frame rate, beams following the player, the view's kick, barrels, traps, bubbles and fireballs. Captures in `e1m1` show each weapon's effects.
- **Future extension:** real light on models, coloured lights, and blended particles, each behind a setting. Screen tints for damage, water and powerups arrive with the status bar.

## Review

See [review 0009](../reviews/0009-increment-6.md).
