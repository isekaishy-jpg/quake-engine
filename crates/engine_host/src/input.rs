//! Owned normalized input and Forever-style scoped delivery gating.

use crate::{Error, PhysicalSize, RawTime, WindowTarget};
use std::collections::{HashMap, VecDeque};
use std::time::Duration;

/// Position in physical client pixels, preserved at event receipt.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicalPosition {
    /// Horizontal coordinate in physical client pixels.
    pub x: f64,
    /// Vertical coordinate in physical client pixels.
    pub y: f64,
}

/// Physical keyboard identity; scan codes keep unknown keys bindable.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PhysicalKey {
    /// Native extended scan code; interpretation belongs to the platform adapter.
    ScanCode(u32),
    /// An unidentified native code without invented semantic identity.
    Unidentified(u32),
}

/// Named logical key meanings, independent of physical binding identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NamedKey {
    /// Escape.
    Escape,
    /// Enter or Return.
    Enter,
    /// Tab.
    Tab,
    /// Space.
    Space,
    /// Backspace.
    Backspace,
    /// Delete.
    Delete,
    /// Insert.
    Insert,
    /// Home.
    Home,
    /// End.
    End,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// Left arrow.
    ArrowLeft,
    /// Right arrow.
    ArrowRight,
    /// Up arrow.
    ArrowUp,
    /// Down arrow.
    ArrowDown,
    /// Shift.
    Shift,
    /// Control.
    Control,
    /// Alt.
    Alt,
    /// Alternate graphics modifier.
    AltGraph,
    /// Super/Windows modifier.
    Super,
    /// Caps lock.
    CapsLock,
    /// Num lock.
    NumLock,
    /// Scroll lock.
    ScrollLock,
    /// Context menu.
    ContextMenu,
    /// Print screen.
    PrintScreen,
    /// Pause.
    Pause,
    /// Numbered function key, preserving the backend's observed number.
    Function(u8),
    /// Other backend semantic name; the string is not a stable binding code.
    Other(String),
}

/// Logical keyboard meaning. Character identity is not committed text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LogicalKey {
    /// Unicode character meaning under the current layout.
    Character(String),
    /// Named semantic meaning.
    Named(NamedKey),
    /// Dead key, optionally identifying its combining character.
    Dead(Option<char>),
    /// Unidentified native logical code.
    Unidentified(u32),
}

/// Physical keyboard location as observed by the backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyLocation {
    /// No left/right/numpad distinction.
    Standard,
    /// Left modifier or key.
    Left,
    /// Right modifier or key.
    Right,
    /// Numeric keypad.
    Numpad,
}

/// Modifier state associated with receipt, not a later global query.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Modifiers {
    /// Shift is held.
    pub shift: bool,
    /// Control is held.
    pub control: bool,
    /// Alt is held.
    pub alt: bool,
    /// Super/Windows is held.
    pub super_key: bool,
}

/// A key or button transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ButtonState {
    /// Pressed or repeated press.
    Pressed,
    /// Released.
    Released,
}

/// Mouse button identity; extended unknown buttons retain their native number.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MouseButton {
    /// Left button.
    Left,
    /// Right button.
    Right,
    /// Middle button.
    Middle,
    /// Back button.
    Back,
    /// Forward button.
    Forward,
    /// Other observed native button number.
    Other(u16),
}

/// Scroll axes and their original units, without integer truncation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollDelta {
    /// Fractional line units.
    Lines {
        /// Horizontal amount.
        x: f64,
        /// Vertical amount.
        y: f64,
    },
    /// Physical pixel units.
    Pixels {
        /// Horizontal amount.
        x: f64,
        /// Vertical amount.
        y: f64,
    },
}

/// Origin distinguishes native facts from synthetic reconciliation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputOrigin {
    /// Normal native window event.
    NativeWindow,
    /// Raw device input.
    RawDevice,
    /// Backend-generated focus synthesis.
    FocusSynthetic,
    /// `engine_host`-generated held-state reconciliation.
    Reconciliation,
}

/// An owned normalized event. Native translation is a later adapter capability.
#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    /// Physical and logical keyboard identity, separately from text.
    Key {
        /// Physical scan-code identity.
        physical: PhysicalKey,
        /// Current layout meaning.
        logical: LogicalKey,
        /// Physical key location.
        location: KeyLocation,
        /// Press/release state.
        state: ButtonState,
        /// Native repeat flag.
        repeat: bool,
        /// Receipt-time modifiers.
        modifiers: Modifiers,
        /// Whether the backend synthesized this transition.
        synthetic: bool,
    },
    /// Absolute physical client coordinates.
    PointerMoved(PhysicalPosition),
    /// Raw relative device movement, not a client coordinate or camera policy.
    RelativeMotion {
        /// Horizontal relative movement.
        x: f64,
        /// Vertical relative movement.
        y: f64,
    },
    /// Raw Windows absolute normalized coordinates, not pixel coordinates.
    RawAbsoluteMotion {
        /// Observed horizontal normalized units, unclamped.
        x: i32,
        /// Observed vertical normalized units, unclamped.
        y: i32,
        /// Whether coordinates refer to the virtual desktop.
        virtual_desktop: bool,
    },
    /// Button transition retaining its event-associated pointer position.
    MouseButton {
        /// Observed button identity.
        button: MouseButton,
        /// Press/release state.
        state: ButtonState,
        /// Last received client position, absent before an absolute observation.
        position: Option<PhysicalPosition>,
        /// Receipt-time native scale.
        scale: f64,
    },
    /// Scroll transition retaining axes, units and event-associated position.
    Wheel {
        /// Original fractional scroll axes and units.
        delta: ScrollDelta,
        /// Last received client position when available.
        position: Option<PhysicalPosition>,
        /// Receipt-time native scale.
        scale: f64,
    },
    /// Native window focus observation.
    Focus(bool),
    /// Actual native capture loss, distinct from focus loss.
    CaptureLost,
    /// Receipt-time modifier observation.
    Modifiers(Modifiers),
    /// Owned committed Unicode text for an exact text-target incarnation.
    Text {
        /// Session active at native receipt; never retargeted during delivery.
        session: crate::TextSessionId,
        /// Committed Unicode text.
        text: String,
    },
    /// Owned preedit Unicode and checked UTF-8 byte selection, separate from commit.
    Preedit {
        /// Session owning this native composition.
        session: crate::TextSessionId,
        /// Current preedit contents.
        text: String,
        /// Optional UTF-8 byte selection, on character boundaries.
        selection: Option<(usize, usize)>,
    },
    /// Native geometry observation; scale must be positive and finite.
    Geometry {
        /// Physical client size.
        size: PhysicalSize,
        /// Native scale observed for associated geometry/input.
        scale: f64,
    },
}

/// Honest receipt/delivery timestamps; no native occurrence time is fabricated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InputStamp {
    /// Host-monotonic receipt order.
    pub sequence: u64,
    /// Host raw time when `engine_host` received the record.
    pub receipt: RawTime,
    /// Host raw time when application delivery began, absent while deferred.
    pub delivery: Option<RawTime>,
    /// Native source time only when the backend actually supplies one.
    pub source_time: Option<Duration>,
}

/// An opaque raw-device observation within one host. Native device identity may
/// repeat across reconnects; this token makes no nonreuse or hardware guarantee.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct InputDeviceId {
    pub(crate) host: crate::HostId,
    pub(crate) native: winit::event::DeviceId,
}
impl InputDeviceId {
    /// The host at which this native identity was observed.
    pub fn host(self) -> crate::HostId {
        self.host
    }
}

/// Fully owned deferred input. Position and text belong to this record.
#[derive(Clone, Debug, PartialEq)]
pub struct InputRecord {
    /// Observed native raw-device identity; ordinary window records have none.
    pub device: Option<InputDeviceId>,
    /// Exact native window generation at receipt.
    pub target: WindowTarget,
    /// Separate receipt and delivery stamps.
    pub stamp: InputStamp,
    /// Native/synthetic origin.
    pub origin: InputOrigin,
    /// Owned normalized payload.
    pub event: InputEvent,
}

/// Explicit state layer; platform receipt can lead application delivery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputStateLayer {
    /// Facts already received from the backend.
    Platform,
    /// Facts applied immediately before application handlers.
    Delivered,
}

/// A window generation's input state in one declared layer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputState {
    /// Currently held physical keys.
    pub held_keys: Vec<PhysicalKey>,
    /// Currently held mouse buttons.
    pub held_buttons: Vec<MouseButton>,
    /// Last absolute client position.
    pub position: Option<PhysicalPosition>,
    /// Latest native focus observation.
    pub focused: bool,
    /// Latest observed modifiers.
    pub modifiers: Modifiers,
    /// Most recent native scale, absent before geometry/associated input.
    pub scale: Option<f64>,
}

impl InputState {
    fn apply(&mut self, event: &InputEvent) -> Result<(), Error> {
        match event {
            InputEvent::Key {
                physical,
                state,
                modifiers,
                ..
            } => {
                set_held(&mut self.held_keys, *physical, *state)?;
                self.modifiers = *modifiers;
            }
            InputEvent::MouseButton {
                button,
                state,
                position,
                scale,
            } => {
                set_held(&mut self.held_buttons, *button, *state)?;
                self.position = *position;
                self.scale = Some(*scale);
            }
            InputEvent::PointerMoved(position) => self.position = Some(*position),
            InputEvent::Wheel {
                position, scale, ..
            } => {
                self.position = *position;
                self.scale = Some(*scale);
            }
            InputEvent::Focus(focused) => {
                self.focused = *focused;
                if !focused {
                    self.held_keys.clear();
                    self.held_buttons.clear();
                    self.modifiers = Modifiers::default();
                }
            }
            InputEvent::CaptureLost => self.held_buttons.clear(),
            InputEvent::Modifiers(modifiers) => self.modifiers = *modifiers,
            InputEvent::Geometry { scale, .. } => self.scale = Some(*scale),
            _ => (),
        }
        Ok(())
    }
}
fn set_held<T: Copy + PartialEq>(
    values: &mut Vec<T>,
    value: T,
    state: ButtonState,
) -> Result<(), Error> {
    match state {
        ButtonState::Pressed if !values.contains(&value) => {
            values.try_reserve(1).map_err(|_| Error::AllocationFailed)?;
            values.push(value);
        }
        ButtonState::Released => values.retain(|held| *held != value),
        _ => (),
    }
    Ok(())
}

/// One bounded input drain's observation; leftover records retain their order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InputDrainReport {
    /// Records consumed, including stale-generation discards.
    pub consumed: usize,
    /// Records still queued after this visit.
    pub remaining: usize,
}

pub(crate) struct Input {
    pub(crate) enabled: bool,
    pub(crate) queue: VecDeque<InputRecord>,
    next_sequence: u64,
    #[cfg(test)]
    pub(crate) fail_next_queue_reservation: bool,
    #[cfg(test)]
    pub(crate) fail_next_delivery_reservation: bool,
    platform: HashMap<WindowTarget, InputState>,
    delivered: HashMap<WindowTarget, InputState>,
}
impl Input {
    pub(crate) fn new() -> Self {
        Self {
            enabled: true,
            queue: VecDeque::new(),
            next_sequence: 1,
            #[cfg(test)]
            fail_next_queue_reservation: false,
            #[cfg(test)]
            fail_next_delivery_reservation: false,
            platform: HashMap::new(),
            delivered: HashMap::new(),
        }
    }
    pub(crate) fn state(
        &self,
        target: WindowTarget,
        layer: InputStateLayer,
    ) -> Option<&InputState> {
        match layer {
            InputStateLayer::Platform => self.platform.get(&target),
            InputStateLayer::Delivered => self.delivered.get(&target),
        }
    }
    pub(crate) fn receive(
        &mut self,
        target: WindowTarget,
        event: InputEvent,
        origin: InputOrigin,
        receipt: RawTime,
        source_time: Option<Duration>,
        device: Option<InputDeviceId>,
    ) -> Result<(), Error> {
        validate(&event)?;
        let sequence = self.next_sequence;
        self.next_sequence = sequence
            .checked_add(1)
            .ok_or(Error::IdentityExhausted(crate::IdentityKind::Input))?;
        self.platform
            .try_reserve(1)
            .map_err(|_| Error::AllocationFailed)?;
        self.platform.entry(target).or_default().apply(&event)?;
        #[cfg(test)]
        if std::mem::take(&mut self.fail_next_queue_reservation) {
            return Err(Error::AllocationFailed);
        }
        self.queue
            .try_reserve(1)
            .map_err(|_| Error::AllocationFailed)?;
        self.queue.push_back(InputRecord {
            device,
            target,
            stamp: InputStamp {
                sequence,
                receipt,
                delivery: None,
                source_time,
            },
            origin,
            event,
        });
        Ok(())
    }
    pub(crate) fn deliver(&mut self, record: &mut InputRecord, time: RawTime) -> Result<(), Error> {
        #[cfg(test)]
        if std::mem::take(&mut self.fail_next_delivery_reservation) {
            return Err(Error::AllocationFailed);
        }
        self.delivered
            .try_reserve(1)
            .map_err(|_| Error::AllocationFailed)?;
        self.delivered
            .entry(record.target)
            .or_default()
            .apply(&record.event)?;
        record.stamp.delivery = Some(time);
        Ok(())
    }
    pub(crate) fn retire(&mut self, target: WindowTarget) {
        self.queue.retain(|record| record.target != target);
        self.platform.remove(&target);
        self.delivered.remove(&target);
    }

    pub(crate) fn retire_all(&mut self) {
        self.queue.clear();
        self.platform.clear();
        self.delivered.clear();
    }
}
fn validate(event: &InputEvent) -> Result<(), Error> {
    let finite = |x: f64, y: f64| x.is_finite() && y.is_finite();
    let position_valid = |position: Option<PhysicalPosition>| {
        position.is_none_or(|position| finite(position.x, position.y))
    };
    let scale_valid = |scale: f64| scale.is_finite() && scale > 0.0;
    let valid = match event {
        InputEvent::PointerMoved(position) => finite(position.x, position.y),
        InputEvent::RelativeMotion { x, y } => finite(*x, *y),
        InputEvent::MouseButton {
            position, scale, ..
        } => position_valid(*position) && scale_valid(*scale),
        InputEvent::Wheel {
            delta,
            position,
            scale,
        } => {
            let (ScrollDelta::Lines { x, y } | ScrollDelta::Pixels { x, y }) = delta;
            finite(*x, *y) && position_valid(*position) && scale_valid(*scale)
        }
        InputEvent::Geometry { scale, .. } => scale_valid(*scale),
        InputEvent::Preedit {
            text, selection, ..
        } => selection.is_none_or(|(start, end)| {
            start <= end && text.is_char_boundary(start) && text.is_char_boundary(end)
        }),
        _ => true,
    };
    if valid {
        Ok(())
    } else {
        Err(Error::InvalidInput(
            "invalid normalized input coordinates, scale or text selection",
        ))
    }
}
