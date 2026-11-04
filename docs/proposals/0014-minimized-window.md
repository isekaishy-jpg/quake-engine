# Proposal 0014: the minimized window

Status: accepted, 2026-11-03, increment 1.

## Research

- The human's direction: a minimized window "could just have a preview or just save resources".
- [Review 0003](../reviews/0003-increment-1.md) showed frames kept running at the full cap while minimized, drawing nothing anyone could see, and that ticks don't need frames.
- A minimized window can report a zero-sized surface, as [this NVIDIA forum report](https://forums.developer.nvidia.com/t/problems-with-vk-khr-swapchain/43513) describes, and a swapchain can't be created at zero extent, as [this Khronos thread](https://community.khronos.org/t/about-the-swapchain-image-resizing/7397) notes. The usual answer, as in the [vulkanalia tutorial](https://kylemayes.github.io/vulkanalia/swapchain/recreation.html), is to skip rendering until the size returns.
- So a live preview can't come from the ordinary present path. It would need a separate route, and there's no renderer to try one with yet.
- `solapp` reports `minimized` in `SADisplayGeometry`, with a zero client size being valid while minimized.

## Design

- The platform checks the window's state on each service visit.
- While minimized, it stops asking for frames. On restore, it asks for one and frames resume.
- Ticks carry on, because a server can't stop for one player's window.
- Input no longer waits for a frame. [Proposal 0015](0015-mouse.md) delivers it as it arrives.

## Proposal

- **Selected approach:** save resources now. A preview waits for a renderer.
- **Tradeoffs:** the taskbar shows whatever the system last had, which may be stale. Pausing a single-player game when minimized is a separate choice, left for when there's a game to pause.
- **Acceptance criteria:** frames stop while minimized and resume on restore, and ticks stay at 72 a second throughout. The CPU saving can only be measured once frames draw something.
- **Future extension:** a preview is an [open question](../research/open-questions.md) for the renderer.

## Review

See [review 0004](../reviews/0004-minimized-and-mouse.md).
