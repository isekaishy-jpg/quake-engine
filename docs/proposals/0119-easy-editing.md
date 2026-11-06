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
- **No hard hands needed.** Everything is a single click, with no drag, no held key, no right button, and no time limit. Arrows, Space, the number keys, U and P do it all without a mouse. Walking round in 3D is four buttons.
- **Eyes.** Bigger and Smaller scale everything up to three times. Plain colours gives white on black.
- **Play is one press:** it saves, naming a new level itself, lights it, and runs it.
- **It's the same map.** `plan_paint`, `plan_thing`, `plan_look` and `plan_read` are ordinary operations, so an agent can do all of this, and the plan makes ordinary brushes and entities in the map file. What someone builds by hand beside it is left alone. Advanced mode has everything it had, and is one button away in both directions.
- A new level opens easy, and a level made another way opens advanced.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a plan is one floor, with square rooms of one height. That's a real limit on what can be made here. Going further means Advanced, where the plan's brushes can be worked on, though painting again remakes what the plan made.
- **Acceptance criteria:** someone with no training can paint a room, add a start and a light, and play it, with only a keyboard or only a mouse. Advanced and agent workflows work as they did.
- **Future extension:** for going over with the human. Stairs and upper floors on the plan. A screen reader, which needs `egui`'s AccessKit joined to our own window, and spoken hints. Pictures on the tools. A prefab shelf on the plan. Reaching into the plan from Advanced.

## Review

A level of two rooms, a door, a pool, a fire, an enemy and three lights was made through the operations, baked in 0.3 s and played. The plan and the 3D view were looked at in the editor. No person with a disability has tried it, and only they can say whether it works for them. That needs a person.
