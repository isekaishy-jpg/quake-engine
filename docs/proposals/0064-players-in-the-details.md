# Proposal 0064: the rest of the player

Status: accepted, 2026-11-04, increment 10.

## Research

- `sv_user.c` won't let a player back up for 0.7 s after a teleport, through `teleport_time`.
- `DeathBubbles` sends bubbles up from drowning pain and from dying under water.
- `PainSound` and `GibPlayer` play `player/teledth1.wav` for a telefrag.
- `PlayerDie` throws the body up a little, by up to 300, if it wasn't already going up. `GibPlayer` makes the player dead at once.
- `killed_self` has a rare second line, when `random()` is exactly 0.
- `powerup_touch` brings powerups back in any deathmatch, where armour and ammo only come back in deathmatch 1.
- `misc_teleporttrain` sets its model. The port left that out, so the teleporter was invisible. `avelocity` is set too, though pushers don't rotate in this build of the original.

## Design

- `PlayerState.no_backing` counts down, so prediction sees it too. Forward movement can't go below 0 while it lasts, out of the water.
- The dead leave the ground on any upward speed, so the hop isn't lost to the rule that keeps the living on it.
- The rest follow `player.qc`, `client.qc`, `items.qc` and `plats.qc`.
- `weapon_rule` guesses nails, so `worldspawn` now names their models, as it does for rockets and grenades.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** the suicide line says "their" for "his", as the other messages do.
- **Acceptance criteria:** each has a test with a control: nobody can back up until it wears off, bubbles rise under water and not on land, the telefrag sound plays for that and nothing else, the body hops and lands, and powerups and armour come back by the right rules.
- **Future extension:** none planned.

## Review

See [review 0013](../reviews/0013-increment-10.md).
