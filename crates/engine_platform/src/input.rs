//! Keyboard and mouse input, in this crate's own terms.
//!
//! Keys are named by physical position, so bindings work on any layout.

use solapp::{SAButtonState, SAInputEvent, SAMouseButton, SAPhysicalKey, SAScrollDelta};

/// A mouse button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton {
    /// The left button.
    Left,
    /// The right button.
    Right,
    /// The middle button.
    Middle,
}

/// One input event, in the order received.
#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    /// A key went down or up, by its name ([`key_name`]).
    Key {
        /// The key's name, such as `ENTER` or `A`.
        name: &'static str,
        /// Down or up.
        pressed: bool,
        /// Whether this is a held key's repeat.
        repeat: bool,
    },
    /// A mouse button went down or up.
    Button {
        /// The button.
        button: MouseButton,
        /// Down or up.
        pressed: bool,
    },
    /// Raw mouse movement in device counts, while the mouse is captured.
    Motion {
        /// Horizontal counts.
        x: f64,
        /// Vertical counts.
        y: f64,
    },
    /// The wheel turned, in lines. Positive is away from the user.
    Wheel {
        /// Lines turned.
        lines: f64,
    },
    /// The window lost focus, so held keys should be released.
    FocusLost,
}

/// Pixels per wheel line, for devices that report pixels.
const WHEEL_PIXELS_PER_LINE: f64 = 40.0;

/// The name of the key at a scan code, by position.
#[must_use]
pub fn key_name(scan: u32) -> Option<&'static str> {
    const ROWS: [(u32, &str); 4] = [
        (0x02, "1234567890-="),
        (0x10, "QWERTYUIOP[]"),
        (0x1E, "ASDFGHJKL;'`"),
        (0x2C, "ZXCVBNM,./"),
    ];
    const NAMES: [&str; 46] = [
        "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "=", "Q", "W", "E", "R", "T", "Y",
        "U", "I", "O", "P", "[", "]", "A", "S", "D", "F", "G", "H", "J", "K", "L", ";", "'", "`",
        "Z", "X", "C", "V", "B", "N", "M", ",", ".", "/",
    ];
    let mut index = 0;
    for (first, keys) in ROWS {
        let count = u32::try_from(keys.len()).unwrap_or(0);
        if (first..first + count).contains(&scan) {
            let at = usize::try_from(scan - first).unwrap_or(0);
            return NAMES.get(index + at).copied();
        }
        index += keys.len();
    }
    Some(match scan {
        0x01 => "ESCAPE",
        0x0E => "BACKSPACE",
        0x0F => "TAB",
        0x1C | 0xE01C => "ENTER",
        0x1D => "LEFT CTRL",
        0xE01D => "RIGHT CTRL",
        0x2A => "LEFT SHIFT",
        0x36 => "RIGHT SHIFT",
        0x38 => "LEFT ALT",
        0xE038 => "RIGHT ALT",
        0x39 => "SPACE",
        0x2B => "\\",
        0x3B => "F1",
        0x3C => "F2",
        0x3D => "F3",
        0x3E => "F4",
        0x3F => "F5",
        0x40 => "F6",
        0x41 => "F7",
        0x42 => "F8",
        0x43 => "F9",
        0x44 => "F10",
        0x57 => "F11",
        0x58 => "F12",
        0xE048 => "UP",
        0xE050 => "DOWN",
        0xE04B => "LEFT",
        0xE04D => "RIGHT",
        0xE047 => "HOME",
        0xE04F => "END",
        0xE049 => "PAGE UP",
        0xE051 => "PAGE DOWN",
        0xE052 => "INSERT",
        0xE053 => "DELETE",
        0x45 | 0xE11D => "PAUSE",
        0x52 => "KEYPAD 0",
        0x4F => "KEYPAD 1",
        0x50 => "KEYPAD 2",
        0x51 => "KEYPAD 3",
        0x4B => "KEYPAD 4",
        0x4C => "KEYPAD 5",
        0x4D => "KEYPAD 6",
        0x47 => "KEYPAD 7",
        0x48 => "KEYPAD 8",
        0x49 => "KEYPAD 9",
        0x53 => "KEYPAD .",
        0x37 => "KEYPAD *",
        0x4A => "KEYPAD -",
        0x4E => "KEYPAD +",
        0xE035 => "KEYPAD /",
        _ => return None,
    })
}

/// The engine's events for one host event.
pub(crate) fn translate(event: &SAInputEvent) -> Option<InputEvent> {
    match event {
        SAInputEvent::Key {
            physical: SAPhysicalKey::ScanCode(scan),
            state,
            repeat,
            ..
        } => Some(InputEvent::Key {
            name: key_name(*scan)?,
            pressed: *state == SAButtonState::Pressed,
            repeat: *repeat,
        }),
        SAInputEvent::MouseButton { button, state, .. } => {
            let button = match button {
                SAMouseButton::Left => MouseButton::Left,
                SAMouseButton::Right => MouseButton::Right,
                SAMouseButton::Middle => MouseButton::Middle,
                _ => return None,
            };
            Some(InputEvent::Button {
                button,
                pressed: *state == SAButtonState::Pressed,
            })
        }
        SAInputEvent::RelativeMotion { x, y } => Some(InputEvent::Motion { x: *x, y: *y }),
        SAInputEvent::Wheel { delta, .. } => Some(InputEvent::Wheel {
            lines: match *delta {
                SAScrollDelta::Lines { y, .. } => y,
                SAScrollDelta::Pixels { y, .. } => y / WHEEL_PIXELS_PER_LINE,
            },
        }),
        SAInputEvent::Focus(false) => Some(InputEvent::FocusLost),
        _ => None,
    }
}
