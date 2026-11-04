# Proposal 0014: the minimized window

Status: accepted, 2026-11-03, increment 1.

## Research

- The human's direction: a minimized window "could just have a preview or just save resources". The preview meant is the Windows one, shown on taskbar hover and in Alt-Tab, which the human describes as live at around 30 frames a second, with a mechanism of its own for each. Both are wanted: save resources and keep that preview.
- [Review 0003](../reviews/0003-increment-1.md) showed frames kept running at the full cap while minimized, drawing nothing anyone could see, and that ticks don't need frames.
- A minimized window can report a zero-sized surface, as [this NVIDIA forum report](https://forums.developer.nvidia.com/t/problems-with-vk-khr-swapchain/43513) describes, and a swapchain can't be created at zero extent, as [this Khronos thread](https://community.khronos.org/t/about-the-swapchain-image-resizing/7397) notes. The usual answer, as in the [vulkanalia tutorial](https://kylemayes.github.io/vulkanalia/swapchain/recreation.html), is to skip rendering until the size returns.
- The Windows preview doesn't go through the application at all while the window is on the desktop. Microsoft's [thumbnail overview](https://learn.microsoft.com/en-us/windows/win32/dwm/thumbnail-ovw) calls these "dynamic, constant connections", not snapshots, and names both taskbar hover and Alt-Tab. So they stay live for as long as frames run, at a rate the system sets. That rate isn't documented, and neither is how the shell builds each one.
- A minimized window is different. A [third-party developer](https://github.com/Yun-yyskk/windowpeeking) reports that Windows stops composing it, and [a clock app's issue](https://github.com/danielchalmers/DesktopClock/issues/126) reports the preview staying as it was at minimize. Both are secondary sources. If they hold, the last frame stays in the preview with no work from the engine.
- An application can supply its own image. With [`DWMWA_HAS_ICONIC_BITMAP`](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute) set, the system asks through [`WM_DWMSENDICONICTHUMBNAIL`](https://learn.microsoft.com/en-us/windows/win32/dwm/wm-dwmsendiconicthumbnail), and the window answers with [`DwmSetIconicThumbnail`](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmseticonicthumbnail). The full-size peek uses [`DwmSetIconicLivePreviewBitmap`](https://learn.microsoft.com/en-us/windows/win32/api/Dwmapi/nf-dwmapi-dwmseticoniclivepreviewbitmap).
- These are the dedicated APIs for this job, as the human pointed out. Two details matter. `DWMWA_FORCE_ICONIC_REPRESENTATION` shows the bitmap even when a live view exists, so it isn't wanted on a visible window. And [`DwmInvalidateIconicBitmaps`](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwminvalidateiconicbitmaps) asks for a fresh image. Microsoft warns against calling it frequently because of the cost, so the refresh rate has to be measured, and it should only run while a preview is showing.
- `winit` has no hook for these messages in its public window API, while `solapp` can give the window's `HWND`, so the window could be subclassed inside `engine_platform`.
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
- **Future extension:** with a renderer, `engine_platform` sets `DWMWA_HAS_ICONIC_BITMAP` without forcing it, and answers the system's requests. A request means a preview is showing, so the renderer then draws off-screen at the size asked for, with no swapchain, and the platform keeps refreshing until requests stop. Between previews, nothing is drawn. The [open questions](../research/open-questions.md) cover what needs measuring.

## Review

See [review 0004](../reviews/0004-minimized-and-mouse.md).
