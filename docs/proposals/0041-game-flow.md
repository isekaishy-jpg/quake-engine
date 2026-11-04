# Proposal 0041: from the start of the game to the end

Status: accepted, 2026-11-04, increment 8.

## Research

- Reading `client.qc` against the port found what stood between it and a whole game:
  - Levels changed at once, with no intermission or episode text.
  - `trigger_setskill` did nothing, and `skill` was fixed at 1, so the start map's choice had no effect.
  - `serverflags` was a script global, and each level is a new Luau state, so every rune was lost at the next level. Shub-Niggurath's gate could never have opened.
  - `SelectSpawnPoint` wasn't ported. Coming back to `start` with a rune has to use `info_player_start2`, or the player lands back at the very beginning.
  - `ClientObituary` and `ClientKill` waited for somewhere to show them.
- The original keeps `serverflags` in `svs`, saves it at a level change, and clears it on `map`. A death's restart goes back to what the level started with.
- `PF_changelevel` ignores every request after the first, and a `NO_INTERMISSION` exit touches every tick.

## Design

- The rest of `client.qc` is ported: `FindIntermission`, `execute_changelevel`, `ExitIntermission`, `IntermissionThink`, `GotoNextMap`, `respawn`, `ClientKill`, `ClientObituary` and the spawn choice. `hold_at_camera` is the part `execute_changelevel` and `finale_1` share.
- `Level` carries `flags` and `settings`. `start_level` hands them to the game before anything spawns, and `cvar` reads them. `engine.changelevel` takes the flags to carry.
- `localcmd` reaches the host's console, and `stuffcmd` the client's, so `skill 2` and `bf` work as they did.
- `PlayerState.frozen` holds the player still at their origin, which is what `MOVETYPE_NONE` and an empty `view_ofs` come to.
- `god`, `notarget`, `give`, `kill` and `changelevel` go through the server to `ClientCommand`.
- A menu or console stops the game, as `_Host_Frame` does, and so does `pause`.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** another player's kills in `ClientObituary` wait for online play. "his death" reads "their death".
- **Acceptance criteria:** tests on the real scripts cover the camera, waiting for a button, episode text, the four rune text, carrying runes, skill, and each exit's level. Every level is reachable from `start` by the maps' own exits. Coming back with a rune lands somewhere else. Captures show the results and the typed text on real maps.
- **Future extension:** coop and deathmatch rules with online play.

## Review

See [review 0011](../reviews/0011-increment-8.md).
