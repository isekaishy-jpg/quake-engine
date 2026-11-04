# Proposal 0014: the minimized window

Status: accepted, 2026-11-03, increment 1.

## Research

- The human's direction: a minimized window "could just have a preview or just save resources". The preview meant is the Windows one, shown on taskbar hover and in Alt-Tab, which the human describes as live at around 30 frames a second, with a mechanism of its own for each. Both are wanted: save resources and keep that preview.
- [Review 0003](../reviews/0003-increment-1.md) showed frames kept running at the full cap while minimized, drawing nothing anyone could see, and that ticks don't need frames.
- A minimized window can report a zero-sized surface, as [this NVIDIA forum report](https://forums.developer.nvidia.com/t/problems-with-vk-khr-swapchain/43513) describes, and a swapchain can't be created at zero extent, as [this Khronos thread](https://community.khronos.org/t/about-the-swapchain-image-resizing/7397) notes. The usual answer, as in the [vulkanalia tutorial](https://kylemayes.github.io/vulkanalia/swapchain/recreation.html), is to skip rendering until the size returns.
- The Windows preview doesn't go through the application at all while the window is on the desktop. Microsoft's [thumbnail overview](https://learn.microsoft.com/en-us/windows/win32/dwm/thumbnail-ovw) calls these "dynamic, constant connections", not snapshots, and names both taskbar hover and Alt-Tab. So they stay live for as long as frames run, at a rate the system sets. That rate isn't documented, and neither is how the shell builds each one.
- A minimized window is different. A [third-party developer](https://github.com/Yun-yyskk/windowpeeking) reports that Windows stops composing it, and [a clock app's issue](https://github.com/danielchalmers/DesktopClock/issues/126) reports the preview staying as it was at minimize. Both are secondary sources. If they hold, the last frame stays in the preview with no work from the engine.
- An application can supply its own images. [Windows previews](../research/windows-previews.md) records the APIs from Microsoft Learn. The thumbnail and the peek view each have their own message and call, and Alt-Tab asks for peek through the same message as taskbar hover.
- Those calls take static bitmaps, so a moving preview is a series of them. Microsoft warns against refreshing too often because of the cost, so the rate has to be measured.
- `solapp` reports `minimized` in `SADisplayGeometry`, with a zero client size being valid while minimized.

## Design

- The platform checks the window's state on each service visit.
- While minimized, it stops asking for frames. On restore, it asks for one and frames resume.
- Ticks carry on, because a server can't stop for one player's window.
- Input no longer waits for a frame. [Proposal 0015](0015-mouse.md) delivers it as it arrives.

## Proposal

- **Selected approach:** save resources when minimized, and only then. A window that's merely behind others keeps its frames, so its preview stays live. A minimized window supplies its preview through the APIs above once there's an image to supply.
- **Tradeoffs:** the taskbar shows whatever the system last had, which may be stale. Pausing a single-player game when minimized is a separate choice, left for when there's a game to pause.
- **Acceptance criteria:** frames stop while minimized and resume on restore, and ticks stay at 72 a second throughout. The CPU saving can only be measured once frames draw something.
- **Future extension:** with a renderer:
  - `engine_platform` subclasses the window and sets `DWMWA_HAS_ICONIC_BITMAP`.
  - It sets `DWMWA_FORCE_ICONIC_REPRESENTATION` when the window is minimized and clears it on restore, so a visible window keeps the automatic live view.
  - A request means a preview is showing. The renderer draws off-screen at the size asked for, with no swapchain, and the platform answers with the matching call.
  - Invalidating asks Windows to request again. If no request comes, the preview has closed and drawing stops, so nothing is drawn between previews.
  - The [open questions](../research/open-questions.md) cover what needs measuring.

## Review

See [review 0004](../reviews/0004-minimized-and-mouse.md).
