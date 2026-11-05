# Proposal 0057: the feel of a shot

Status: accepted, 2026-11-04, increment 9.

## Research

- At 80 ms a shot's kick took 7 ticks to appear, because the view kick, the weapon's frames and the muzzle flash all came from the server. Only the sound and the projectile were shown at once.
- `DropPunchAngle` brings the view back at 10 degrees a second.
- `W_Attack` and what it calls give each weapon's sound, pace, kick and frames. Nails and lightning go round for as long as the button is held.
- The blast check took every one of your own straight flyers for a rocket, so a nail at a nearby wall predicted a rocket's knock.

## Design

- The game supplies a `ShotFeel` for each weapon, and `fixture_quake::weapon_rule` has all eight. `ShownShot` plays it from the click.
- While that's showing, the server's kick, frames and flash are the same shot arriving late, so they're left out. It stops once the server has been seen firing and has finished, or when the server hasn't answered by a round trip and a bit, so a refused shot goes away.
- A held weapon goes through the same path as a click. `ShotClock` counts from when each shot was due, as `refire` does, and each shot that isn't part of a loop goes out with a seed.
- The launch event says whether a thing blows up. Only that predicts a blast.
- A blast's strength comes from `BlastRule`, including what the player's carrying, so a quad rocket jump is right too.
- The dead and the intermission show no shots, since a click there isn't one.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a weapon that's refused still kicks for a moment. The axe has random frames, so only its sound is shown. `weapon_rule` restates numbers from the scripts, so a test holds each one against what the scripts really do.
- **Acceptance criteria:** at 80 ms a shot kicks, flashes and moves the weapon on the click, once. Without showing it, the kick takes at least 5 ticks. A shot the server never fires stops being shown. Every weapon's sound, pace, kick, flash and first frame match the scripts. Nails don't knock you. Held and quad rocket jumps stay with the server.
- **Future extension:** none planned.

## Review

See [review 0012](../reviews/0012-increment-9.md).
