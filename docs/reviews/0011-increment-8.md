# Review 0011: increment 8

Status: review record, 2026-11-04. Reviewed: proposals 0039 to 0044, and the changes to every crate they touch.

| # | Finding | Handling |
|---|---|---|
| 1 | `serverflags` lived in a script global, and each level is a new Luau state, so runes were lost at every level change | `Level` carries the flags, with a test |
| 2 | Coming back to `start` with a rune landed at the very beginning, since the engine only knows `info_player_start` | `SelectSpawnPoint`'s rule is ported, and a test on the real `start` shows the two places |
| 3 | `trigger_setskill` did nothing | It sends `skill` to the host, and the next level reads it |
| 4 | A `NO_INTERMISSION` exit asked for its level every tick | `changelevel` only goes once, as `PF_changelevel` does |
| 5 | Five closures in `think` fields couldn't be saved | They're named functions, with their state in fields |
| 6 | The canvas used a size kept from window events, which aren't promised before the first frame, so it could have been drawn for the wrong size in borderless | The renderer reports the size it draws to |
| 7 | A level change waited for frames that a minimized window doesn't draw | The wait counts every frame |
| 8 | "Reset to defaults" went back to the original's keys, where `a` looks up | It sets the modern movement keys again too |
| 9 | `changelevel` from the console started the player with nothing | It goes through the game, as an exit does |
| 10 | `;` is a key called `SEMICOLON`, so it couldn't be typed | It types |
| 11 | With no game, closing the menus left nothing to type at | The console opens |
| 12 | Turning keys could move a view held by an intermission | They're ignored, as the mouse is |
| 13 | Menu text was hard to read over a bright view | The view dims, as `Draw_FadeScreen` does |
| 14 | The first save test passed everywhere at once, which proves nothing alone | Restoring the wrong random seed on purpose made it fail, then the code went back |

## Verified

These ran against the Steam data, read in place. Captures used a throwaway user folder.

| Check | Result |
|---|---|
| `cargo xtask check` | Passes |
| A game with every monster awake, saved through JSON and resumed, against the original for five seconds | 38 maps, identical on every tick |
| Every level reachable from `start` by the maps' own exits | All 32 |
| Every script on every map, and every monster fought | 117 maps and 1888 monsters, no script errors |
| Every picture the screens name | All found |
| `start` to `e1m1`, results, a button, `e1m2` with gear kept and keys gone | Captured |
| `e1m7` to the results, the typed text, then back to `start` | Captured |
| Saving on `e1m1`, leaving, and loading | Captured, with the view, gear and keys back |
| Main, Options, Keys, Load, Quit, console, scores and pause | Captured |

## Not verified

- Nobody has played it. Captures go through the same drawing as the window, but at a fixed size, and no one has seen the bar in the real borderless window.
- Keys were only driven through tests with a pretend host, not on a keyboard.
- The whole game hasn't been played through in order. Each link is tested, and every exit is known to exist, but a monster or door that blocks the way in practice wouldn't show.
- The original wasn't run beside it, so the screens were checked against its source.

## Limitations

- The first run reads Steam's `config.cfg`, if there is one, and prints unknown commands for 1996's settings. They don't return once this engine writes its own.
- Typing follows a US layout. Window mode can't be changed from the menus.
- Particles and other things only the client knows aren't saved.
- Mission pack status bars aren't done, and the Multiplayer row waits for online play.
