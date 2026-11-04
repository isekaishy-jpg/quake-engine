# Review 0004: the minimized window and mouse look

Status: review record, 2026-11-03. Reviewed: proposals 0014 and 0015, and the changes to `engine_platform`, `engine_client` and `engine_app`.

| # | Finding | Handling |
|---|---|---|
| 1 | Not asking for the next frame didn't stop frames, because `Forever` pacing schedules them by itself | Pacing is now disabled while minimized, and put back on restore |
| 2 | Input queued for frames would wait for as long as the window was minimized, and tied mouse look to frame timing | Input now goes to the application as it arrives |
| 3 | Proposal 0014 expected a measurable fall in CPU time, but empty frames cost too little to measure: 31 ms visible against 47 ms minimized over 4 s, at a timer step of about 16 ms | Criterion moved to when frames draw something |

## Verified

These ran against the Steam `id1` data, read in place.

| Check | Result |
|---|---|
| Frames while minimized | 0 in 4.19 s (715 before and after) |
| Ticks while minimized | 302 in 4.19 s, 72.1 a second |
| Frames after restore | Resumed, 114 in 1.17 s |
| Mouse look tests | 9 pass, including exact return and the per-event clamp |
| Raw motion with the mouse captured | Reached the view. Each change in angle was a whole number of counts at 0.066 degrees a count |

`cargo xtask check` passes.

## Remaining

- The raw motion run isn't a clean count check. Hand movement on the same desktop mixed with the injected motion, so the 300 and 100 counts sent couldn't be told apart from it. A second run was skipped because the desktop wasn't idle.
- Releasing capture with `m_capture 0` wasn't observed on its own.
- Mouse buttons and the wheel remain unexercised.
- A preview and 8 kHz mice are [open questions](../research/open-questions.md).
