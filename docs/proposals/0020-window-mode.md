# Proposal 0020: the window mode

Status: accepted, 2026-11-03, increment 2.

## Research

- The human's direction: the default should be borderless fullscreen.
- `solapp` offers `SADisplayRequest::{Windowed, Borderless, Exclusive}` through `request_display`, and `monitors` lists the choices. A request needs a monitor from that list.
- A request is a transition. After the native change, `solapp` waits for the application to report that presentation is ready at the new size, through `report_presentation`.
- A borderless window stays composed by the desktop, so Alt-Tab and taskbar previews keep working, and switching away is quick.

## Design

- `WindowConfig::mode` is `WindowMode::Borderless` or `WindowMode::Windowed`.
- For borderless, the platform requests the monitor the window opened on, falling back to the first it finds.
- The platform reports presentation ready as soon as the change is applied, because the renderer rebuilds its swapchain from the resize that follows.
- The `-window` argument opens a normal window instead.
- The renderer rebuilds its swapchain when told of a resize, not by comparing sizes, so a size it can't match can't cause a rebuild every frame.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** exclusive fullscreen isn't offered, since nothing needs it yet. A menu setting comes with the menus.
- **Acceptance criteria:** with no arguments the client area fills the monitor and frames draw at that size, and `-window` opens a normal window.
- **Future extension:** a saved setting and a monitor choice arrive with the menus.

## Review

See [review 0005](../reviews/0005-increment-2.md).
