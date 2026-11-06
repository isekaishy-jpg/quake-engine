# Proposal 0124: coloured light for the original levels

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

The rerelease's pak has a `.lit` for 37 levels, and `load_level` already reads one. The question was left to the human, and the sprint makes the call and records it here.

The first idea was to mount the `.lit` files beside the original levels. That can't work. A `.lit` holds three bytes for each byte of a level's light, in the same order, and **none of the 37 match an original level.** `e3m6.lit` needs 195,506 bytes of light, and the original has 191,084. It does match the rerelease's own `e3m6.bsp`.

So the rerelease's levels are different files. E1M1 has 6,605 faces to the original's 5,516, 86 textures to 81, and 428 entities to 369. They're the same levels, rebuilt, with some walls textured differently. Coloured light means playing those.

Mounting the whole rerelease over the game isn't an option, as it would change `progs`, models and pictures too.

## Design

- **`ColouredLevels`** wraps an archive and shows only levels with a `.lit`, and those `.lit` files. A level and its light come as a pair, and everything else stays as the game has it.
- **It's on when the rerelease is there,** mounted for each game directory over what's already mounted. Without it levels are white, as they were.
- **`-whitelight`** leaves it off, for the original levels exactly. The fixture is a baseline, so comparing against the original has to stay one flag away.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** by default the fixture's levels are no longer byte for byte the 1996 ones. The tests of the port's behaviour read the original pak directly, so they still hold it to that.
- **Acceptance criteria:** the levels show colour, and every one loads, runs and fights without an error.
- **Future extension:** none planned.

## Review

`every_coloured_level_loads_runs_and_fights` loads each of the 41 levels with a `.lit` in the rerelease, checks its light isn't white, runs its scripts, and wakes, hurts and kills every monster. None failed. E1M1 was looked at with and without colour.
