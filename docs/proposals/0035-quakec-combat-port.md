# Proposal 0035: the QuakeC port of combat

Status: accepted, 2026-11-04, increment 6.

## Research

- `combat.qc`, `weapons.qc`, `items.qc`, `player.qc`, and the rest of `client.qc`, `misc.qc` and `subs.qc`, come from `v101qc`, the single player progs. Increment 4's logic differed from `qw-qc` only in licences and networking, so the port's source doesn't change its behaviour.
- Luau caches any global a chunk doesn't assign, unless compiled at optimization level 0. That froze `trace_fraction`, and made `world` nil everywhere but its own script, so `worldtype` silently read as medieval on every map.
- A `__newindex` only fires for absent keys, so `model` and `owner` keep a hidden field to go on being seen.
- QuakeC writes `self.punchangle_x` and the engine reads it. `SV_ClientThink` runs `DropPunchAngle` at 10 degrees a second.
- `v_angle`, `button0` and `impulse` come from the player's command.
- A player carries `parm1` to `parm16` between levels. `SetChangeParms` drops keys and powerups, and `SetNewParms` starts a new game.
- `barrel_explode`, `trap_spikeshooter`, `misc_fireball` and the bubbles need nothing more than weapons do. `LaunchLaser` lives in `enforcer.qc`, but a trap fires it too.

## Design

- Each `.qc` becomes one `.luau`, with its function names kept, and `self` and `other` as arguments.
- Scripts are compiled at level 0, so every global is read fresh.
- The engine gains `traceline`, `pointcontents`, `droptofloor`, `set_owner`, `temp_entity`, `particle`, `set_parms` and `restart`.
- `Server` keeps the level it started with and its parms. Dying restarts the level with them.
- The script's `punchangle` is the simulation's, which drops it and sends it in each snapshot.
- `LaunchLaser` sits with the weapons, ready for the enforcer.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** level 0 gives up Luau's global caching and inlining. A game's scripts run a few times a tick, and nothing measures slow. `aim` returns straight ahead until monsters give it something to aim at, and a backpack waits for coop and deathmatch.
- **Acceptance criteria:** all eight weapons, ammo, pickups, armour, powerups, keys, damage, drowning, lava and slime, death and restart, and shootable buttons and doors have logic tests. Every touch and target on all 117 maps runs without a script error.
- **Future extension:** monsters in increment 7, and the status bar's bonus flash in increment 8.

## Review

See [review 0009](../reviews/0009-increment-6.md).
