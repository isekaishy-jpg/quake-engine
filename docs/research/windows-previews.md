# Windows window previews

What Microsoft Learn says about the previews shown on taskbar hover and in Alt-Tab, and what supplying one takes. Read 2026-11-03. Everything here comes from the pages linked, unless marked as inference.

## Two previews, each with its own mechanism

Both appear on taskbar hover and in Alt-Tab.

| | Thumbnail | Peek, also called live preview |
|---|---|---|
| What it is | The small image | A [full-sized view](https://learn.microsoft.com/en-us/windows/win32/dwm/wm-dwmsendiconiclivepreviewbitmap), which appears when the pointer is over the thumbnail in the taskbar, or the thumbnail has focus in Alt-Tab |
| Windows asks with | [`WM_DWMSENDICONICTHUMBNAIL`](https://learn.microsoft.com/en-us/windows/win32/dwm/wm-dwmsendiconicthumbnail) | [`WM_DWMSENDICONICLIVEPREVIEWBITMAP`](https://learn.microsoft.com/en-us/windows/win32/dwm/wm-dwmsendiconiclivepreviewbitmap) |
| The window answers with | [`DwmSetIconicThumbnail`](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmseticonicthumbnail) | [`DwmSetIconicLivePreviewBitmap`](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmseticoniclivepreviewbitmap) |
| Size | At most the size in `lParam`: x in the high word, y in the low word. Anything larger is refused | At most the client area |
| Format | 32-bit | 32-bit |
| Kept by Windows | Cached, and may be dropped at any time | Not cached. Dropped when the preview ends |
| Asked for when | An iconic view is showing, `DWMWA_HAS_ICONIC_BITMAP` is set, no bitmap is cached, and the cache has room | Peek was invoked, `DWMWA_HAS_ICONIC_BITMAP` is set, and an iconic view is the only one that exists |
| No answer in time | Windows uses its default | Windows uses its default |

Setting a peek bitmap while no peek is showing returns success, and the bitmap is thrown away.

## The automatic route

Previews of a window on the desktop need nothing from the application. The [thumbnail overview](https://learn.microsoft.com/en-us/windows/win32/dwm/thumbnail-ovw) calls them "dynamic, constant connections", and the [taskbar page](https://learn.microsoft.com/en-us/windows/win32/shell/taskbar-extensions) says thumbnails are "normally automatic".

[`DwmRegisterThumbnail`](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmregisterthumbnail) is the other direction: it shows another window's live thumbnail inside yours. The engine has no use for it.

## Attributes

[`DwmSetWindowAttribute`](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmsetwindowattribute) sets these, described on the [attribute page](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute):

- `DWMWA_HAS_ICONIC_BITMAP` says the window will supply bitmaps.
- `DWMWA_FORCE_ICONIC_REPRESENTATION` shows the bitmap even if a live view exists.
- Both pages for the iconic calls set the two together before supplying a bitmap.
- Both are normally set at creation, but the page allows that "some scenarios" need them to change over time.

## Refreshing

[`DwmInvalidateIconicBitmaps`](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwminvalidateiconicbitmaps) marks both bitmaps as out of date, and Windows asks again when it next needs them. It "should not be called frequently" because of the cost. If the cache is full, Windows won't ask.

The iconic calls are described as static bitmaps. There's no documented way to stream to a preview, so a moving preview is a series of bitmaps. No refresh rate is documented, and no message says a preview has closed.

## Receiving the messages

- The messages go to the window. `engine_host` gives the `HWND` through `acquire_window`.
- [`SetWindowSubclass`](https://learn.microsoft.com/en-us/windows/win32/api/commctrl/nf-commctrl-setwindowsubclass) adds a handler, and has to run on the window's own thread. `RemoveWindowSubclass` takes it off.
- `winit` has `with_msg_hook`, but it sits on the message loop. Messages sent straight to the window wouldn't pass through it, so subclassing looks safer. That's inference, and how Windows delivers these two messages isn't documented.

## Keeping a window composed

The [attribute page](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute) says `DWMWA_CLOAK` hides a window while it's "still composed by DWM". That makes it a candidate for keeping a live preview through minimizing, if the engine handles minimize itself. Whether a cloaked window keeps its taskbar preview and its place in Alt-Tab isn't documented.

## Not needed

- [`ITaskbarList3`](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-itaskbarlist3) `RegisterTab` and [`ITaskbarList4::SetTabProperties`](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-itaskbarlist4-settabproperties) are for tabs and documents inside one window.
- `SetThumbnailClip` only picks part of the client area for the thumbnail.

## Gaps in the docs

- The [functions](https://learn.microsoft.com/en-us/windows/win32/dwm/functions) and [messages](https://learn.microsoft.com/en-us/windows/win32/dwm/dwm-messages) index pages came back with no lists, so this set was gathered from the pages that link to each other.
- Nothing says what a minimized window shows by default, or whether its preview stays live.
- The thumbnail overview dates from Windows Vista. The Alt-Tab line on the peek pages is the only statement about Alt-Tab that names these calls.
