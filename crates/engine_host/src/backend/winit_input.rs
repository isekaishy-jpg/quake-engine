//! Receipt-time native translation. This module never invokes application code.

use std::collections::HashMap;
use std::time::Duration;

use winit::event::{
    DeviceEvent, ElementState, Ime, MouseButton as WinitMouseButton, MouseScrollDelta, WindowEvent,
};
use winit::keyboard::{
    Key, KeyLocation as WinitKeyLocation, NamedKey as WinitNamedKey, NativeKey, NativeKeyCode,
    PhysicalKey as WinitPhysicalKey,
};
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::platform::scancode::PhysicalKeyExtScancode;

use crate::text::TextState;
use crate::{
    ButtonState, Error, InputEvent, InputOrigin, InputState, KeyLocation, LogicalKey, Modifiers,
    MouseButton, NamedKey, PhysicalKey, PhysicalPosition, PhysicalSize, ScrollDelta, WindowTarget,
};

pub(crate) struct NormalizedInput {
    pub(crate) target: WindowTarget,
    pub(crate) event: InputEvent,
    pub(crate) origin: InputOrigin,
    pub(crate) source_time: Option<Duration>,
    pub(crate) device: Option<crate::InputDeviceId>,
}

struct KeyMeaning {
    logical: LogicalKey,
    location: KeyLocation,
}

pub(crate) struct NativeInputAdapter {
    keys: HashMap<(WindowTarget, PhysicalKey), KeyMeaning>,
    #[cfg(test)]
    fail_next_batch_reservation: std::cell::Cell<bool>,
    #[cfg(test)]
    fail_next_key_reservation: bool,
}

impl NativeInputAdapter {
    pub(crate) fn new() -> Self {
        Self {
            keys: HashMap::new(),
            #[cfg(test)]
            fail_next_batch_reservation: std::cell::Cell::new(false),
            #[cfg(test)]
            fail_next_key_reservation: false,
        }
    }

    /// `state` is the platform layer before this event, never the delivered layer.
    pub(crate) fn window_event(
        &mut self,
        target: WindowTarget,
        event: &WindowEvent,
        state: &InputState,
        physical_size: PhysicalSize,
        scale: f64,
        text: &mut TextState,
    ) -> Result<Vec<NormalizedInput>, Error> {
        // Keyboard owns its complete key/text batch; do not allocate a discarded
        // window batch before handing it over.
        if let WindowEvent::KeyboardInput {
            event,
            is_synthetic,
            ..
        } = event
        {
            return self.key_event(
                target,
                event.physical_key,
                &event.logical_key,
                event.location,
                event.state,
                event.repeat,
                *is_synthetic,
                event.text_with_all_modifiers(),
                state,
                text,
            );
        }
        let count = match event {
            WindowEvent::Focused(false) => state
                .held_keys
                .len()
                .checked_add(state.held_buttons.len())
                .and_then(|count| count.checked_add(1))
                .ok_or(Error::AllocationFailed)?,
            WindowEvent::MouseCaptureLost => state
                .held_buttons
                .len()
                .checked_add(1)
                .ok_or(Error::AllocationFailed)?,
            WindowEvent::MouseInput {
                state: native_state,
                button,
                ..
            } => usize::from(
                *native_state == ElementState::Pressed
                    || state.held_buttons.contains(&mouse_button(*button)),
            ),
            WindowEvent::Ime(Ime::Enabled) => 0,
            WindowEvent::Ime(Ime::Disabled) => {
                usize::from(text.composing(target) && text.ime_session(target).is_some())
            }
            WindowEvent::Ime(Ime::Preedit(..) | Ime::Commit(_)) => {
                usize::from(text.ime_session(target).is_some())
            }
            WindowEvent::CursorMoved { .. }
            | WindowEvent::MouseWheel { .. }
            | WindowEvent::ModifiersChanged(_)
            | WindowEvent::Focused(true)
            | WindowEvent::Resized(_)
            | WindowEvent::ScaleFactorChanged { .. } => 1,
            _ => return Ok(Vec::new()),
        };
        // Reserve the entire output before key reconciliation or IME mutation.
        let mut batch = self.batch(count)?;
        match event {
            WindowEvent::CursorMoved { position, .. } => push(
                &mut batch,
                target,
                InputEvent::PointerMoved(PhysicalPosition {
                    x: position.x,
                    y: position.y,
                }),
                InputOrigin::NativeWindow,
            ),
            WindowEvent::MouseInput {
                state: button_state,
                button,
                ..
            } => {
                let button = mouse_button(*button);
                let button_state = transition(*button_state);
                // Reconciliation may already have released this button, notably
                // synchronous capture loss during intentional last-button up.
                if button_state == ButtonState::Pressed || state.held_buttons.contains(&button) {
                    push(
                        &mut batch,
                        target,
                        InputEvent::MouseButton {
                            button,
                            state: button_state,
                            position: state.position,
                            scale,
                        },
                        InputOrigin::NativeWindow,
                    );
                }
            }
            WindowEvent::MouseWheel { delta, .. } => push(
                &mut batch,
                target,
                InputEvent::Wheel {
                    delta: scroll_delta(*delta),
                    position: state.position,
                    scale,
                },
                InputOrigin::NativeWindow,
            ),
            WindowEvent::ModifiersChanged(modifiers) => {
                let modifiers = modifiers.state();
                push(
                    &mut batch,
                    target,
                    InputEvent::Modifiers(Modifiers {
                        shift: modifiers.shift_key(),
                        control: modifiers.control_key(),
                        alt: modifiers.alt_key(),
                        super_key: modifiers.super_key(),
                    }),
                    InputOrigin::NativeWindow,
                );
            }
            WindowEvent::Focused(focused) => {
                if !focused {
                    for physical in &state.held_keys {
                        let meaning = self.keys.remove(&(target, *physical));
                        push(
                            &mut batch,
                            target,
                            InputEvent::Key {
                                physical: *physical,
                                logical: meaning
                                    .as_ref()
                                    .map_or(LogicalKey::Unidentified(0), |key| key.logical.clone()),
                                location: meaning.map_or(KeyLocation::Standard, |key| key.location),
                                state: ButtonState::Released,
                                repeat: false,
                                modifiers: Modifiers::default(),
                                synthetic: true,
                            },
                            InputOrigin::Reconciliation,
                        );
                    }
                    release_buttons(&mut batch, target, state, scale);
                }
                push(
                    &mut batch,
                    target,
                    InputEvent::Focus(*focused),
                    InputOrigin::NativeWindow,
                );
            }
            WindowEvent::MouseCaptureLost => {
                release_buttons(&mut batch, target, state, scale);
                push(
                    &mut batch,
                    target,
                    InputEvent::CaptureLost,
                    InputOrigin::NativeWindow,
                );
            }
            WindowEvent::Ime(ime) => match ime {
                Ime::Enabled => text.set_composing(target, true)?,
                Ime::Disabled => {
                    if text.composing(target)
                        && let Some(session) = text.ime_session(target)
                    {
                        push(
                            &mut batch,
                            target,
                            InputEvent::Preedit {
                                session,
                                text: String::new(),
                                selection: None,
                            },
                            InputOrigin::NativeWindow,
                        );
                    }
                    text.set_composing(target, false)?;
                }
                Ime::Preedit(value, selection) => {
                    if let Some(session) = text.ime_session(target) {
                        push(
                            &mut batch,
                            target,
                            InputEvent::Preedit {
                                session,
                                text: value.clone(),
                                selection: *selection,
                            },
                            InputOrigin::NativeWindow,
                        );
                    }
                }
                Ime::Commit(value) => {
                    if let Some(session) = text.ime_session(target) {
                        push(
                            &mut batch,
                            target,
                            InputEvent::Text {
                                session,
                                text: value.clone(),
                            },
                            InputOrigin::NativeWindow,
                        );
                    }
                }
            },
            WindowEvent::Resized(size) => push(
                &mut batch,
                target,
                InputEvent::Geometry {
                    size: PhysicalSize {
                        width: size.width,
                        height: size.height,
                    },
                    scale,
                },
                InputOrigin::NativeWindow,
            ),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => push(
                &mut batch,
                target,
                InputEvent::Geometry {
                    size: physical_size,
                    scale: *scale_factor,
                },
                InputOrigin::NativeWindow,
            ),
            _ => (),
        }
        Ok(batch)
    }

    // A field-based boundary permits deterministic keyboard traces without
    // constructing winit's platform-private KeyEvent payload or inventing WM_CHAR.
    #[expect(
        clippy::too_many_arguments,
        reason = "Preserve separate native keyboard observations."
    )]
    pub(crate) fn key_event(
        &mut self,
        target: WindowTarget,
        physical: WinitPhysicalKey,
        logical: &Key,
        location: WinitKeyLocation,
        native_state: ElementState,
        repeat: bool,
        synthetic: bool,
        committed: Option<&str>,
        state: &InputState,
        text: &TextState,
    ) -> Result<Vec<NormalizedInput>, Error> {
        let physical = physical_key(physical);
        let key_state = transition(native_state);
        if key_state == ButtonState::Released && !state.held_keys.contains(&physical) {
            self.keys.remove(&(target, physical));
            return Ok(Vec::new());
        }
        let committed =
            if key_state == ButtonState::Pressed && !synthetic && !text.composing(target) {
                text.active(target)
                    .zip(committed.filter(|value| !value.is_empty()))
            } else {
                None
            };
        let mut batch = self.batch(1 + usize::from(committed.is_some()))?;
        let logical = logical_key(logical);
        let location = key_location(location);
        if key_state == ButtonState::Pressed {
            #[cfg(test)]
            if std::mem::take(&mut self.fail_next_key_reservation) {
                return Err(Error::AllocationFailed);
            }
            self.keys
                .try_reserve(1)
                .map_err(|_| Error::AllocationFailed)?;
            self.keys.insert(
                (target, physical),
                KeyMeaning {
                    logical: logical.clone(),
                    location,
                },
            );
        } else {
            self.keys.remove(&(target, physical));
        }
        push(
            &mut batch,
            target,
            InputEvent::Key {
                physical,
                logical,
                location,
                state: key_state,
                repeat,
                modifiers: state.modifiers,
                synthetic,
            },
            if synthetic {
                InputOrigin::FocusSynthetic
            } else {
                InputOrigin::NativeWindow
            },
        );
        if let Some((session, committed)) = committed {
            push(
                &mut batch,
                target,
                InputEvent::Text {
                    session,
                    text: committed.to_owned(),
                },
                InputOrigin::NativeWindow,
            );
        }
        Ok(batch)
    }

    /// The caller selects an eligible live target at receipt. Raw keys, buttons,
    /// wheels and companion axes are redundant with the normal window path.
    pub(crate) fn device_event(
        &self,
        selected_target: Option<WindowTarget>,
        event: &DeviceEvent,
        device: crate::InputDeviceId,
    ) -> Result<Vec<NormalizedInput>, Error> {
        let Some(target) = selected_target else {
            return Ok(Vec::new());
        };
        let event = match event {
            DeviceEvent::MouseMotion { delta } => InputEvent::RelativeMotion {
                x: delta.0,
                y: delta.1,
            },
            DeviceEvent::MouseMotionAbsolute {
                position,
                virtual_desktop,
            } => InputEvent::RawAbsoluteMotion {
                x: position.0,
                y: position.1,
                virtual_desktop: *virtual_desktop,
            },
            _ => return Ok(Vec::new()),
        };
        let mut batch = self.batch(1)?;
        push(&mut batch, target, event, InputOrigin::RawDevice);
        batch[0].device = Some(device);
        Ok(batch)
    }

    fn batch(&self, count: usize) -> Result<Vec<NormalizedInput>, Error> {
        let mut batch = Vec::new();
        if count != 0 {
            #[cfg(test)]
            if self.fail_next_batch_reservation.replace(false) {
                return Err(Error::AllocationFailed);
            }
            batch
                .try_reserve_exact(count)
                .map_err(|_| Error::AllocationFailed)?;
        }
        Ok(batch)
    }

    pub(crate) fn retire(&mut self, target: WindowTarget) {
        self.keys.retain(|(window, _), _| *window != target);
    }

    pub(crate) fn clear(&mut self) {
        self.keys.clear();
    }
}

fn push(
    batch: &mut Vec<NormalizedInput>,
    target: WindowTarget,
    event: InputEvent,
    origin: InputOrigin,
) {
    batch.push(NormalizedInput {
        target,
        event,
        origin,
        source_time: None,
        device: None,
    });
}

fn release_buttons(
    batch: &mut Vec<NormalizedInput>,
    target: WindowTarget,
    state: &InputState,
    scale: f64,
) {
    for button in &state.held_buttons {
        push(
            batch,
            target,
            InputEvent::MouseButton {
                button: *button,
                state: ButtonState::Released,
                position: state.position,
                scale,
            },
            InputOrigin::Reconciliation,
        );
    }
}

fn transition(state: ElementState) -> ButtonState {
    match state {
        ElementState::Pressed => ButtonState::Pressed,
        ElementState::Released => ButtonState::Released,
    }
}

fn physical_key(key: WinitPhysicalKey) -> PhysicalKey {
    match key {
        WinitPhysicalKey::Unidentified(NativeKeyCode::Windows(code)) => {
            PhysicalKey::Unidentified(u32::from(code))
        }
        _ => key
            .to_scancode()
            .map_or(PhysicalKey::Unidentified(0), PhysicalKey::ScanCode),
    }
}

fn logical_key(key: &Key) -> LogicalKey {
    match key {
        Key::Character(value) => LogicalKey::Character(value.to_string()),
        Key::Dead(value) => LogicalKey::Dead(*value),
        Key::Named(value) => LogicalKey::Named(named_key(*value)),
        Key::Unidentified(NativeKey::Windows(value)) => LogicalKey::Unidentified(u32::from(*value)),
        Key::Unidentified(_) => LogicalKey::Unidentified(0),
    }
}

fn key_location(location: WinitKeyLocation) -> KeyLocation {
    match location {
        WinitKeyLocation::Standard => KeyLocation::Standard,
        WinitKeyLocation::Left => KeyLocation::Left,
        WinitKeyLocation::Right => KeyLocation::Right,
        WinitKeyLocation::Numpad => KeyLocation::Numpad,
    }
}

fn named_key(key: WinitNamedKey) -> NamedKey {
    use NamedKey as S;
    use WinitNamedKey as N;
    match key {
        N::Escape => S::Escape,
        N::Enter => S::Enter,
        N::Tab => S::Tab,
        N::Space => S::Space,
        N::Backspace => S::Backspace,
        N::Delete => S::Delete,
        N::Insert => S::Insert,
        N::Home => S::Home,
        N::End => S::End,
        N::PageUp => S::PageUp,
        N::PageDown => S::PageDown,
        N::ArrowLeft => S::ArrowLeft,
        N::ArrowRight => S::ArrowRight,
        N::ArrowUp => S::ArrowUp,
        N::ArrowDown => S::ArrowDown,
        N::Shift => S::Shift,
        N::Control => S::Control,
        N::Alt => S::Alt,
        N::AltGraph => S::AltGraph,
        N::Super => S::Super,
        N::CapsLock => S::CapsLock,
        N::NumLock => S::NumLock,
        N::ScrollLock => S::ScrollLock,
        N::ContextMenu => S::ContextMenu,
        N::PrintScreen => S::PrintScreen,
        N::Pause => S::Pause,
        N::F1 => S::Function(1),
        N::F2 => S::Function(2),
        N::F3 => S::Function(3),
        N::F4 => S::Function(4),
        N::F5 => S::Function(5),
        N::F6 => S::Function(6),
        N::F7 => S::Function(7),
        N::F8 => S::Function(8),
        N::F9 => S::Function(9),
        N::F10 => S::Function(10),
        N::F11 => S::Function(11),
        N::F12 => S::Function(12),
        N::F13 => S::Function(13),
        N::F14 => S::Function(14),
        N::F15 => S::Function(15),
        N::F16 => S::Function(16),
        N::F17 => S::Function(17),
        N::F18 => S::Function(18),
        N::F19 => S::Function(19),
        N::F20 => S::Function(20),
        N::F21 => S::Function(21),
        N::F22 => S::Function(22),
        N::F23 => S::Function(23),
        N::F24 => S::Function(24),
        N::F25 => S::Function(25),
        N::F26 => S::Function(26),
        N::F27 => S::Function(27),
        N::F28 => S::Function(28),
        N::F29 => S::Function(29),
        N::F30 => S::Function(30),
        N::F31 => S::Function(31),
        N::F32 => S::Function(32),
        N::F33 => S::Function(33),
        N::F34 => S::Function(34),
        N::F35 => S::Function(35),
        _ => S::Other(format!("{key:?}")),
    }
}

fn mouse_button(button: WinitMouseButton) -> MouseButton {
    match button {
        WinitMouseButton::Left => MouseButton::Left,
        WinitMouseButton::Right => MouseButton::Right,
        WinitMouseButton::Middle => MouseButton::Middle,
        WinitMouseButton::Back => MouseButton::Back,
        WinitMouseButton::Forward => MouseButton::Forward,
        WinitMouseButton::Other(value) => MouseButton::Other(value),
    }
}

fn scroll_delta(delta: MouseScrollDelta) -> ScrollDelta {
    match delta {
        MouseScrollDelta::LineDelta(x, y) => ScrollDelta::Lines {
            x: f64::from(x),
            y: f64::from(y),
        },
        MouseScrollDelta::PixelDelta(position) => ScrollDelta::Pixels {
            x: position.x,
            y: position.y,
        },
    }
}

#[cfg(test)]
#[path = "../../tests/unit/input_translation.rs"]
mod tests;
