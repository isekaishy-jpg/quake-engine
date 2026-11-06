# Proposal 0121: level faults

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

Review 0016 left three things about the real levels.

- **The foundry's lava.** Vesper Deep's two channels were inside the floor. One brush ran from 160 below the floor up to it, and another lay over that, so nobody could see them.
- **Enemies once they move.** The tests only held them to where they start.
- **People in the city.** Two guards and a dog stand in, and they slid along facing one way with their legs still.

## Design

- **The channels are open.** The floor is cut round them, with low glowing light above each. They're 16 units down, so a step gets someone out, at a cost.
- **The foundry is dimmer.** Orange light that strong clips to white, and new lights that low showed it at once. The walls had the same look before. Its lights came down to about 70%.
- **Enemies get a test of their own.** For each enemy in both levels, the player is set down where it can see them, 180 units off, in god mode. After six seconds the enemy must have moved or be doing something, must not be in a solid, and no script may have failed.
- **Guards and the dog walk.** They face the way they're going, and step through their walking frames: the soldier's 24 prowling frames and the dog's 8.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** the guards are still the fixture's soldier and dog.
- **Acceptance criteria:** the lava can be seen and burns, every enemy fights, and a guard faces its way and moves its legs.
- **Future extension:** real people need a person.

## Review

Vesper Deep's 9 enemies and Ember Keep's 21 all pass. Zombies stand and throw, so an enemy that hasn't moved only counts as stuck if its frame hasn't changed either. Vesper Deep was worn again after the cut, for its new edges, and still plays from start to end.

**Going through the level tests found a real fault from item 11.** `light_runs_on_across_joins` failed at 9,820 places, and it failed on the unworn level too. It's an ignored test that needs baked levels, so `cargo xtask check` doesn't run it, and nothing had for nine items.

Item 11 made each face rank its layers by how much of each light it gets. `copy_sample`, which makes faces agree where they join, still copied layers by their place in the list. So style 0's light on one face went into a sharp light's layer on the next. It showed as a straight line in the light along joins, which can be seen on Ember Keep's checked floor at 1664 323. Samples are now matched by which light they hold, and a light the next face has no layer for joins its plain light, as `fit_layers` does. That test passes again, and `faces_with_their_layers_in_another_order_still_agree` fails without the fix. The floor at that place was looked at again, and the line is gone. All nine level tests pass on a fresh bake of every level, at bake version 24.

**Needs a person: a real person's model.** The engine has no character art, and it mustn't be commissioned without a spec. What it should be:

- A townsperson, not a soldier, in the city's clothing, about 56 units tall.
- Today's path is the fixture's `.mdl` style of whole-mesh frames, as meshes have no skeletons yet. That's listed for the final report.
- Frames: stand, walk and a turn, at 10 frames a second, facing along x.
- A 512 by 512 picture on the [texture standard](../texture-standard.md).
