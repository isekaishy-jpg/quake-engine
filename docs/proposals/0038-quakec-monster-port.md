# Proposal 0038: the QuakeC port of monsters

Status: accepted, 2026-11-04, increment 7.

## Research

- `ai.qc`, `fight.qc`, `monsters.qc` and fifteen monster files come to about 7300 lines, most of them one-line frame functions like `[ $stand1, ogre_stand2 ] {ai_stand();};`. Typing that out would invite mistakes that nothing would catch.
- QuakeC's own rules matter in a translation:
  - `&` and `|` bind as tightly as `*`, and `=` tighter than `&&`, from `pr_comp.c`.
  - Unset fields read as zero, an empty string or the world, and the world is false. Zero is true in Luau.
  - `self` and `other` are globals, and some functions swap them. As arguments they can still be reassigned.
  - `*` between two vectors is a dot product.
  - `x_y` is a part of a vector, and writing one has to rebuild it, since Luau's vectors don't change.
  - `$frame` lists number a model's frames in order.
- Some QuakeC is only right by luck. `ogre_smash12` calls `ai_charge` with no argument, taking what was last passed.
- A monster's `Killed` and `T_Damage` have parts that increment 6 left out: counting kills, firing its targets, getting angry at what hit it, and pain in nightmare.

## Design

- A one-off translator turns the files into Luau of this repository's style, and isn't kept: the Luau is the source from now on. Each `.qc` is one `.luau` with the same function names.
- It knows the type of every field, global and local from `defs.qc`, so a truth test becomes `~= 0`, `has_text`, `exists` or nothing.
- `animate` stands for what a `[frame, next]` function does.
- Field reads have defaults, through `set_field_defaults`, so a number never reads as nil.
- `ent` stands in for reading through an empty entity, and `exists` for testing one. Doors keep treating `enemy` as nil, since they were ported that way.
- Network writes become `temp_entity`, and the closing text `finale`.
- Gibs, backpacks, `SpawnMeatSpray`, `aim`, telefrags and `trigger_monsterjump` are ported by hand into the files they belong with.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** translated comments that followed a statement end up on the line after it. A monster's frame functions stay as long as the original's, rather than becoming tables, which keeps them easy to check against the source and to change one at a time.
- **Acceptance criteria:** nothing the scripts use is undefined. Every class on every map has a function. Tests on the real scripts cover seeing, not seeing, attacking, dying, gibbing, backpacks and paths. Every monster runs on every map with no script errors.
- **Future extension:** the intermission and the end of the game, which need the status bar's screens. `ClientObituary` waits for somewhere to show it. Coop and deathmatch come with online play.

## Review

See [review 0010](../reviews/0010-increment-7.md).
