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

The human's decision: go to a reduced mode while minimized, saving resources while keeping enough for the preview and Alt-Tab. A preview can only show what's rendered, so rendering carries on, smaller and slower.

- The platform checks the window's state on each service visit.
- While minimized, frames drop to `WindowConfig::reduced_frame_rate`, and `Frame::reduced` tells the application a small, cheap frame is enough. On restore, normal pacing returns.
- The engine uses 10 frames a second to start with.
- Ticks carry on at full rate, because a server can't stop for one player's window.
- Input no longer waits for a frame. [Proposal 0015](0015-mouse.md) delivers it as it arrives.

## Proposal

- **Selected approach:** reduced mode, as above. A window that's merely behind others stays at full rate.
- **Tradeoffs:** a minimized game still uses a little GPU time once frames draw. Pausing a single-player game when minimized is a separate choice, left for when there's a game to pause.
- **Acceptance criteria:** frames drop to the reduced rate with `Frame::reduced` set, normal frames return on restore, and ticks stay at 72 a second throughout. CPU use doesn't rise. The GPU saving can only be measured once frames draw.
- **Future extension:** the renderer reads `Frame::reduced` and draws small. What's still unknown is how that output reaches the preview, since a minimized window may have no surface to present to. The [open questions](../research/open-questions.md) list the routes to try.

## Review

See [review 0004](../reviews/0004-minimized-and-mouse.md).
