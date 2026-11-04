# Proposal 0052: projectiles

Status: accepted, 2026-11-04, increment 9.

## Research

- The [networking record](../pre-research/networking.md) decided that a projectile is one launch event, not a stream of positions, because its path is fixed once it's fired.
- `SV_Physics_Toss` gives the rules: rockets and nails fly straight, grenades take gravity and bounce by 1.5, and stop under 60 units a second upward on a floor.
- A rocket's path has to be traced as a point. The player hull stops it short of what it really hits.

## Design

- `engine.launch` raises `Effect::Launch` with the entity, owner, origin, velocity, time and whether it bounces. `announce_launch` in the scripts calls it for each thing fired.
- `engine_client::projectiles` works out a `Path` from a launch, a tick at a time, with the rules above. Only the server's projectile hits anything. This decides where it's drawn and when a rocket goes off.
- Your own shot appears as a guess the moment you click, from `fixture_quake::shot_launch`, and the server's launch replaces it. A guess the server never confirms goes away.
- Your own projectile leaves at once and eases towards what's on your screen. An enemy's starts at the muzzle and catches up with the real one within 0.1 s.
- Random spread comes from the seed sent with the shot, so the guess and the server agree.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a door that moves after the launch isn't in the path, so a drawn projectile can pass through it until the server says it's gone. The launch delay and correction are guesses until someone plays them.
- **Acceptance criteria:** a client's path matches the server's rocket and grenade to within a unit, for their whole flight. A refused shot's guess disappears. Nothing about a hit changes.
- **Future extension:** tuning `sv_` values for how own shots ease, from play.

## Review

See [review 0012](../reviews/0012-increment-9.md).
