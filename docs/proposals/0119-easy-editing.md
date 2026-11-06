# Proposal 0119: making levels the easy way

Status: accepted and implemented, 2026-11-06, the sprint through the list. This is the one full pass the human asked for, as a starting point to go over together.

## Research

The human's bar: without an agent, anyone can quickly make maps, including people with disabilities or of poor mental faculty, without removing advanced options or agent workflows. Judged against that, not against other level editors, the editor fails at the first step. It opens on a 3D view of nothing, with a JSON box, and flying needs a right button held and four keys.

- [WCAG 2.2](https://www.w3.org/TR/WCAG22/) asks that nothing needs dragging, that targets are big enough, and that everything works by keyboard.
- [Making Content Usable for People with Cognitive and Learning Disabilities](https://www.w3.org/TR/coga-usable/) asks for one thing at a time, plain words, a clear next step, and that a mistake is easy to undo. Numbers and hidden modes are what get in the way.
- [Game Accessibility Guidelines](https://gameaccessibilityguidelines.com/) add text size, contrast, no timing, and no need for simultaneous buttons.
- What's easiest for making a place is older than computers: graph paper. A floor plan is flat, so there's no camera to steer, and each square either is or isn't room. Level makers for non-experts mostly work this way.

## Design

- **A floor plan.** Click squares to make rooms. Walls, floors and ceilings follow. Nothing to place, line up or size.
- **Plain things.** Start, Light, Door, Health, Ammo, Enemy, Fire and Water, each one click, in a list of ten big buttons with a word each.
- **What's next.** A panel says, in words, what's still missing, and what the chosen tool does. Play can't be pressed until the level can be played, and then it says so.
- **What it's made of.** Six named looks change the whole level at once.
- **No hard hands needed.** Everything is a single click, with no drag, no held key, no right button, and no time limit. Arrows, Space, the number keys, U and P work the plan without a mouse, and Tab and Esc reach every other button. Walking round in 3D is four buttons.
- **Eyes.** Bigger and Smaller scale everything up to three times. Plain colours gives white on black.
- **Play is one press:** it saves, naming a new level itself, lights it, and opens the game, which stays open until it's closed. Nothing has to be set up: the game is looked for beside the editor, and its data in the usual Steam folders. `ENGINE_GAME` and `ENGINE_BASEDIR` still say otherwise.
- **It's the same map.** `plan_paint`, `plan_thing`, `plan_look` and `plan_read` are ordinary operations, so an agent can do all of this, and the plan makes ordinary brushes and entities in the map file. What someone builds by hand beside it is left alone. Advanced mode has everything it had, and is one button away in both directions.
- A new level opens easy, and a level made another way opens advanced.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a plan is one floor, with square rooms of one height. That's a real limit on what can be made here. Going further means Advanced, where the plan's brushes can be worked on, though painting again remakes what the plan made.
- **Acceptance criteria:** someone with no training can paint a room, add a start and a light, and play it, with only a keyboard or only a mouse. Advanced and agent workflows work as they did.
- **Future extension:** for going over with the human. Stairs and upper floors on the plan. A screen reader, which needs `egui`'s AccessKit joined to our own window, and spoken hints. Pictures on the tools. A prefab shelf on the plan. Reaching into the plan from Advanced.

## Review

The first commit of this work, 26c7282, went in before its checks were done, and was marked finished too early. Going through the real window afterwards found five faults, all fixed in the commit after it:

- **Play didn't play.** It used `playtest`, which ran the game for five seconds, closed it, and froze the editor meanwhile. Without `ENGINE_GAME` and `ENGINE_BASEDIR` set, it did nothing at all. A new `play` operation opens the game and leaves it, and finds it unaided. The Play button in Advanced had the same fault and uses it too.
- **Doors never opened.** A named entity is a target, and a door that's a target waits to be fired. What a plan puts down has no name now. The test called `a_door_blocks_the_way_through_and_opens` only checked that a door compiled, and is renamed for what it does.
- **Bigger did nothing.** The bridge to `egui` treated points as pixels, so the scale never reached the screen. Sizes, the pointer, clipping and the 3D view all follow it now.
- **Walls narrowed openings,** by reaching past their square at both ends, which showed as pillars at a door. They only reach past at outer corners now.
- **Advanced opened inside a wall** on a level with no start. It looks from the first square now.

Checked in the real window, with posted keys and clicks: painting and things by keyboard alone, keys after clicks, Tab to a button and Space to press it, Esc back to the plan, P opening the game with nothing set, the game still open after the editor closed, Bigger with clicks landing true, Plain colours, Advanced, and the 3D view at a bigger size. In the game: a level of two rooms, a door, a pool, a fire, an enemy and three lights baked in 0.3 s, and the door rose and let the player through into the pool.

The bright green pool is the Sewer look's own liquid, which glows by its material. It's the art, not a fault.

The wall and target tests fail with their fixes taken out, as do the size tests. No person with a disability has tried any of this, and only they can say whether it works for them. That needs a person.
