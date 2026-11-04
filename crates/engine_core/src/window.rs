//! A window's identity, for the systems that draw into it.

/// An operating-system window, as plain numbers. The platform crate keeps the
/// window alive, so this says nothing about how long it stays valid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeWindow {
    /// A Win32 window.
    Win32 {
        /// The `HWND`.
        hwnd: isize,
        /// The `HINSTANCE`, or zero if unknown.
        hinstance: isize,
    },
}
