# Proposal 0015: mouse look

Status: accepted, 2026-11-03, increment 1.

## Research

- The human's direction: "Mousing can be modern quality(but should be deterministic)", and anything that modifies how the mouse behaves happens only when a user changes a setting.
- Microsoft's [high-definition mouse article](https://docs.microsoft.com/windows/win32/dxtecharts/taking-advantage-of-high-dpi-mouse-movement) recommends `WM_INPUT`, because `WM_MOUSEMOVE` only reports whole pixels and carries pointer ballistics. `winit` uses `WM_INPUT` for `DeviceEvent::MouseMotion`, passing `lLastX` and `lLastY` as `f64`, so counts arrive as exact whole numbers.
- `solapp` gates relative motion on focus, drains input at receipt, and stamps each record with a receipt time. It reports no native event time on this path, and its queue has no drop limit.
- From `in_win.c` and `common.c`, the original's mouse look has these faults for this purpose:
  - It adds to a `float` once a frame, so rounding depends on how motion is split into frames.
  - It clamps pitch once a frame, so a flick past the limit and back gives a different view at a different frame rate.
  - `m_filter` averages two frames, so its result depends on frame rate. [DirectQ](https://forums.insideqc.com/viewtopic.php?p=37148) removed it.
  - `MSG_WriteAngle` sends a view angle in one byte, after truncating it to whole degrees.
- Settings: `sensitivity`, `m_yaw` and `m_pitch` keep their meaning, so old configs still feel the same. Pitch stays from -70 to 80 for the fixture.
- High polling rates are a known problem for engines that handle one message at a time, going by [community reports](https://afkgaming.com/esports/guide/what-is-raw-input-buffer-in-valorant) rather than primary sources. A dedicated thread with `GetRawInputBuffer` is a possible answer. It needs measuring on real 8 kHz hardware.

## Design

**Deterministic** here means the view is a pure function of the ordered raw counts and the settings. Frame rate and tick rate have no effect.

- `engine_client::Look` holds the view. It keeps a count total per axis, and works out the angle from the total. Totals of whole numbers are exact in `f64`, so moving out and back returns to exactly the same angle.
- Each raw event is applied on arrival, by itself. Pitch is clamped per event, in counts. Reading the view changes nothing.
- A settings change folds the current angle into a new origin, so the view doesn't jump.
- Nothing changes how the mouse behaves unless the user sets it. The default is raw counts through `sensitivity`, `m_yaw` and `m_pitch`, and nothing else.
- There's no smoothing, filter or acceleration yet. Any added later is a setting, off by default, and works from the raw event sequence, not frame time, so the view stays deterministic.
- `engine_platform` delivers input as it arrives, with its receipt time, through `PlatformApp::input`, and no longer queues it for frames. It captures the mouse when `PlatformApp::mouse_capture` says so, and reports a failure through `PlatformApp::notice`.
- `m_capture` is off by default, so the cursor is left alone until there's a game to play.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** input arrives on the same thread as drawing, so a blocking present delays receipt, though not the totals. Receipt times are therefore honest but not hardware times.
- **Acceptance criteria:** tests show identical views for any split of reads, exact return after moving out and back, a per-event clamp and no jump when settings change. Real raw motion sent to the window reaches the view with the expected angle.
- **Future extension:**
  - Commands in increment 3 carry full-precision angles, never one byte, and take motion by receipt time. The recorded command then decides replays.
  - Absolute devices, such as remote desktop sessions, aren't handled yet.
  - The 8 kHz question is [open](../research/open-questions.md).

## Review

See [review 0004](../reviews/0004-minimized-and-mouse.md).
