# Proposal 0014: the minimized window

Status: accepted, 2026-11-03, increment 1.

## Research

- The human's direction: a minimized window "could just have a preview or just save resources". The preview meant is the Windows one, shown on taskbar hover and in Alt-Tab, and both are wanted: save resources and keep that preview.
- [Review 0003](../reviews/0003-increment-1.md) showed frames kept running at the full cap while minimized, drawing nothing anyone could see, and that ticks don't need frames.
- A minimized window can report a zero-sized surface, as [this NVIDIA forum report](https://forums.developer.nvidia.com/t/problems-with-vk-khr-swapchain/43513) describes, and a swapchain can't be created at zero extent, as [this Khronos thread](https://community.khronos.org/t/about-the-swapchain-image-resizing/7397) notes. The usual answer, as in the [vulkanalia tutorial](https://kylemayes.github.io/vulkanalia/swapchain/recreation.html), is to skip rendering until the size returns.
- That doesn't stop the Windows preview, which is a separate route. With [`DWMWA_FORCE_ICONIC_REPRESENTATION` and `DWMWA_HAS_ICONIC_BITMAP`](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute) set, the system asks the window for a bitmap with [`WM_DWMSENDICONICTHUMBNAIL`](https://learn.microsoft.com/en-us/windows/win32/dwm/wm-dwmsendiconicthumbnail), and the window answers with [`DwmSetIconicThumbnail`](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmseticonicthumbnail). The full-size peek works the same way through [`DwmSetIconicLivePreviewBitmap`](https://learn.microsoft.com/en-us/windows/win32/api/Dwmapi/nf-dwmapi-dwmseticoniclivepreviewbitmap).
- The bitmap is 32-bit and no larger than the size asked for. `DwmInvalidateIconicBitmaps` marks it as out of date. If the window doesn't answer in time, the system falls back to its own default.
- `winit` has no hook for these messages in its public window API, while `solapp` can give the window's `HWND`, so the window could be subclassed inside `engine_platform`.
- `solapp` reports `minimized` in `SADisplayGeometry`, with a zero client size being valid while minimized.

## Design

- The platform checks the window's state on each service visit.
- While minimized, it stops asking for frames. On restore, it asks for one and frames resume.
- Ticks carry on, because a server can't stop for one player's window.
- Input no longer waits for a frame. [Proposal 0015](0015-mouse.md) delivers it as it arrives.

## Proposal

- **Selected approach:** save resources now. The Windows preview waits for a renderer, since there's no image to give it yet.
- **Tradeoffs:** the taskbar shows whatever the system last had, which may be stale. Pausing a single-player game when minimized is a separate choice, left for when there's a game to pause.
- **Acceptance criteria:** frames stop while minimized and resume on restore, and ticks stay at 72 a second throughout. The CPU saving can only be measured once frames draw something.
- **Future extension:** when frames draw, the renderer keeps a copy of the last frame as the window is minimized, and `engine_platform` answers the system's requests with it. That costs one read-back per minimize and no frames. The [open question](../research/open-questions.md) covers the details.

## Review

See [review 0004](../reviews/0004-minimized-and-mouse.md).
