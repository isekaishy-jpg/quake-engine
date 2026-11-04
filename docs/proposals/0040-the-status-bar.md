# Proposal 0040: the status bar and the view

Status: accepted, 2026-11-04, increment 8.

## Research

- `sbar.c` reads a list of numbers, `cl.stats`, plus the item bits, and draws them. The numbers mean nothing to the rest of the engine.
- `SV_WriteClientdataToMessage` sends those, and damage with where it came from. `dmg_take` and `dmg_save` add up over a frame.
- `view.c` has four colour shifts: contents, damage, bonus and powerup. GLQuake blends them into one colour over the view, in `V_CalcBlend`.
- `V_CalcViewRoll` rolls the view when strafing, kicks it when hit, and rolls a dead player's view to 80 degrees. Review 0010 noted roll as missing.
- `viewsize` shrinks the view around the bar. The catalog drops it. At 16:9, bar pictures 320 wide can't span the screen anyway.
- A status bar is the most game-specific thing there is, and the plan asked how such things are built and scripted.

## Design

- A snapshot carries `stats`, a list of numbers only the game understands, and game time. The game's `ClientData` fills it each tick, in the original's `STAT_` order with the items last.
- `fixture_quake::Hud` is the status bar, as Rust, drawing on a canvas. It's the only place that knows what the numbers are.
- `engine.damage` sends damage. The client's new `ScreenEvent` carries it, and centre text, commands, end of level screens and the level's title to whatever shows them.
- The view is never shrunk: the bar sits over it, centred.
- Roll and kicks are in `engine_client::view`, since any game might use them, and `View` gains a roll.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a modder can't change the bar from Luau yet. Scripting it needs a Luau state on the client, which has no other use today. The engine only sees a canvas, so that can replace `Hud` without touching anything else.
- **Acceptance criteria:** tests measure the faces, warning colours, weapon flashes, powerups, the dead's scores, tints fading, and the numbers arriving from the real scripts. Captures show the bar on real maps.
- **Future extension:** a scripted bar, the mission packs' bars, and the deathmatch scoreboard with online play.

## Review

See [review 0011](../reviews/0011-increment-8.md).
