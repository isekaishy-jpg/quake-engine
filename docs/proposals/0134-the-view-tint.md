# Proposal 0134: the colour over the view

Status: accepted and implemented, 2026-11-06, the [work list](../research/work-list.md), item 74, and the first step of item 77.

## Research

The underwater tint was three fixed colours in the fixture's `hud.rs`, beside the damage, bonus and powerup flashes, in code that item 6 removes. The human's word: it needs to be set with the water and properly used, as a cheap effect that goes with fog and particles, with the renderer drawing it and game code saying when.

- Blending several colours in turn is general, and nothing about it is Quake's. What each colour means is the game's.
- Levels have pools, but original levels don't, so the eye's contents still have to count.
- A map's `water` block already says the look of its liquids.
- A level that sets a colour for its water and is then tinted Quake's brown looks wrong.

## Design

- **`ViewTint`** in `engine_client` holds what the eye is in, being hurt, a bonus flash, and what the game holds on the player, and blends them as `V_CalcBlend` did. `engine_app` draws it.
- **`PoolLook::tint`** is a colour and a strength. The usual values are Quake's, so the original levels look as they did.
- **`water { tint strength }`:** `tint` and `tint_strength` set it. A level that sets `color` and no `tint` gets that colour, so that it matches its own water.
- **`look_around`** finds the pool the eye is in, and otherwise what such a liquid usually looks like.
- **What's left in `hud.rs`** is the game's: the face, and which powerup is which colour, which is passed on.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** the `water` block is one for a map until item 76.
- **Acceptance criteria:** with the eye in water, a level's tint shows, and original levels are as they were. Hurts and flashes show and fade over it.
- **Future extension:** none planned.

## Review

A copy of the water hall with a strong pink tint was looked at with the eye in the pool, and with no setting. The pink showed over the view and the weapon, and not over the status bar, as it should. The plain one showed Quake's brown over a level whose water is another colour, which is why `color` now leads.
