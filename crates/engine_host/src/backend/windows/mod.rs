//! Narrow platform helpers; native input and window mechanics remain in winit.

#![expect(
    unsafe_code,
    reason = "shell and display calls are only reached through Win32"
)]

pub(crate) mod shell;
pub(crate) mod topology;
