# Proposal 0031: sounds in the game

Status: accepted, 2026-11-04, increment 5.

## Research

- `SV_StartSound` sends an entity, a channel, a sound, a volume and an attenuation, from the middle of the entity's box. It refuses values out of range. WinQuake then keeps that origin, so a sound doesn't follow what made it.
- A sound from the view entity plays at full volume, whatever the distance.
- `ambientsound` sends a looping sound at a fixed place. Maps use it through `ambient_*`, and through torches and fluorescent lights, which the port doesn't have yet.
- Each leaf stores four ambient levels, for water, sky, slime and lava. `S_UpdateAmbientSounds` fades `water1.wav` and `wind2.wav` towards them, at 100 a second, using 0.3 of the level and nothing under 8. `qbsp` exposes the levels.
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
- `engine_app` passes what the client received to `Audio`, with the listener and the ambient levels of the leaf it's in. Audio is muted while minimized, as the original did.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a name in each message costs bytes that a precache table would save, which matters at online play, not on loopback. Sounds keep the origin they started at, as WinQuake does.
- **Acceptance criteria:** the sound calls match the QuakeC. Tests cover the messages, refused values, a door's sounds through the real scripts, statics, music and player sounds. A real map plays with no script errors.
- **Future extension:** a precache table at connect. Sounds following entities needs entity numbers in snapshots, which increment 6 brings.

## Review

See [review 0008](../reviews/0008-increment-5.md).
