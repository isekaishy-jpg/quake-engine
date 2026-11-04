# Proposal 0031: sounds in the game

Status: accepted, 2026-11-04, increment 5.

## Research

- `SV_StartSound` sends an entity, a channel, a sound, a volume and an attenuation, from the middle of the entity's box. It refuses values out of range. WinQuake then keeps that origin, so a sound doesn't follow what made it.
- A sound from the view entity plays at full volume, whatever the distance.
- `ambientsound` sends a looping sound at a fixed place. Maps use it through `ambient_*`, and through torches and fluorescent lights, which the port doesn't have yet.
- Each leaf stores four ambient levels, for water, sky, slime and lava. `S_UpdateAmbientSounds` fades `water1.wav` and `wind2.wav` towards them, at 100 a second, using 0.3 of the level and nothing under 8. `qbsp` exposes the levels.
- `CalcAmbientSounds` in `vis` works out each leaf's distance to water and sky, then overwrites it with `maxd = 0.25` before using it, so every shipped level is 0 or 255. `e1m1` has water at full volume in 59% of its open leaves, with no sense of distance.
- The port's `sound` stub dropped the channel, volume and attenuation, so calls need them back from the QuakeC.
- `PlayerPreThink` and `PlayerPostThink` in `client.qc` make the jump, landing and water sounds. They need the player's water level, ground and jump button.
- `spawn_tfog` plays a random teleport sound 0.2 s later.

## Design

- `engine_core::sound` holds `SoundStart` and `StaticSound`, which pass from script to mixer unchanged.
- The host gives scripts `sound`, `ambientsound`, `random` and `engine.music`. It refuses values out of range, as the server does.
- `Outbox` carries sounds, and new messages `Sound`, `StaticSound` and `Music` carry them to the client. Names travel as text for now.
- The server marks a sound as the client's own, so a client needs no entity numbers.
- The server runs `PlayerPreThink` and `PlayerPostThink` around each command, when a game defines them.
- `random` is seeded the same each run, so it stays deterministic.
- `engine_client::AmbientSounds` does what `vis` meant to, from the listener rather than the leaf: full within 100 units of the nearest water or sky in sight of its leaf, fading to nothing at 500. The original jumps from 1 to 0.8 at 100, and this ramps instead. A level the map stores stays a limit, so a map from a fixed `vis` still works.
- `engine_app` passes what the client received to `Audio`, with the listener and those levels. Audio is muted while minimized, as the original did.

## Proposal

- **Selected approach:** the above.
- **Direct upgrades:** ambient sound fades with distance as `vis` intended.
- **Tradeoffs:** a name in each message costs bytes that a precache table would save, which matters at online play, not on loopback. Sounds keep the origin they started at, as WinQuake does.
- **Acceptance criteria:** the sound calls match the QuakeC. Tests cover the messages, refused values, a door's sounds through the real scripts, statics, music and player sounds. A real map plays with no script errors.
- **Future extension:** a precache table at connect. Sounds following entities needs entity numbers in snapshots, which increment 6 brings.

## Review

See [review 0008](../reviews/0008-increment-5.md).
