# Review 0004: the minimized window and mouse look

Status: review record, 2026-11-03. Reviewed: proposals 0014 and 0015, and the changes to `engine_platform`, `engine_client` and `engine_app`.

| # | Finding | Handling |
|---|---|---|
| 1 | Not asking for the next frame didn't stop frames, because `Forever` pacing schedules them by itself | Pacing is now disabled while minimized, and put back on restore |
| 2 | Input queued for frames would wait for as long as the window was minimized, and tied mouse look to frame timing | Input now goes to the application as it arrives |
| 3 | Proposal 0014 expected a measurable fall in CPU time, but empty frames cost too little to measure | Criterion changed to CPU use not rising, with the GPU saving measured once frames draw |
| 4 | Stopping frames would freeze the preview, because a preview only shows what's rendered | Replaced with a reduced mode, by the human's decision |

## Verified

These ran against the Steam `id1` data, read in place.

| Check | Result |
|---|---|
| Reduced frames while minimized | 42 in 4.21 s, 10.0 a second, all marked reduced |
| Ticks while minimized | 303 in 4.21 s, 72.0 a second |
| Frames after restore | Full rate again, 116 in 1.17 s |
| CPU, three rounds of 6 s | Visible 31, 16 and 31 ms. Minimized 0, 16 and 0 ms, at a timer step of about 16 ms. One earlier 4 s reading of 203 ms minimized didn't repeat |
| Mouse look tests | 9 pass, including exact return and the per-event clamp |
| Raw motion with the mouse captured | Reached the view. Each change in angle was a whole number of counts at 0.066 degrees a count |

`cargo xtask check` passes.

## Remaining

- The raw motion run isn't a clean count check. Hand movement on the same desktop mixed with the injected motion, so the 300 and 100 counts sent couldn't be told apart from it. A second run was skipped because the desktop wasn't idle.
- Releasing capture with `m_capture 0` wasn't observed on its own.
- Mouse buttons and the wheel remain unexercised.
- How reduced-mode rendering reaches the preview, and 8 kHz mice, are [open questions](../research/open-questions.md).
