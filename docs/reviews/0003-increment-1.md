# Review 0003: increment 1

Status: review record, 2026-11-03. Reviewed: proposals 0010 to 0013, and the changes to `engine_core`, `engine_platform`, `engine_console`, `engine_assets`, `fixture_quake` and `engine_app`.

| # | Finding | Handling |
|---|---|---|
| 1 | `COM_Parse` also splits on `{ } ( ) ' :`, which the tokenizer doesn't do, while proposal 0012 claimed an exact match | Kept the simpler rule so `host:port` stays one token. The shipped configs don't depend on it. Recorded in proposal 0012, with a test |
| 2 | `DirectoryArchive` joined lower-case paths directly, so upper-case loose files would be missed on a case-sensitive system | Paths now resolve one part at a time, ignoring case, with a test. The test can't fail on Windows, so this isn't verified on a case-sensitive system |
| 3 | Text piped from Windows PowerShell starts with a byte order mark, which broke the first command typed | Removed from standard input lines |
| 4 | The command line had no tests | Added tests that run the executable with refused command lines |
| 5 | Proposal 0011's list of public items was incomplete | Corrected |

## Verified

These ran against the Steam `id1` data, read in place. Nothing was copied.

| Check | Result |
|---|---|
| Window opens and `quit` ends the run | Exit code 0 |
| Window closed by the system | Exit code 0 |
| `exec quake.rc` | Runs `default.cfg` and `config.cfg`. Commands from later increments, such as `bind`, are reported as unknown |
| `path` | `PAK1.PAK`, `PAK0.PAK`, then the `id1` directory |
| `dir maps/e1m` | 8 files |
| Keys sent to the window | `A` and `ENTER` arrive by name, pressed and released |
| Ticks, minimized, frames running | 362 in 5.03 s, 72.0 a second |
| Ticks, minimized, no frames requested, renderer-managed pacing | 377 in 5.23 s, 72.1 a second, with 1 frame in the whole run |

The last row used a temporary change to the host, since reverted. It answers the open question: the tick doesn't depend on frames.

`cargo xtask check` passes.

## Remaining

- Mouse buttons, relative motion and the wheel are translated but weren't exercised, because relative mouse mode has no user yet. The first use comes with player look.
- Tick timing was measured on one Windows 10 machine under light load.
- `assets_manager` and file ids move to increment 2, where typed assets first appear.
- Increment 2 is next: the map importer and rendering.
