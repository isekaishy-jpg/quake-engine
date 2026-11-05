use super::*;
use crate::identity::{HostId, WindowGeneration, WindowSlots};

fn target() -> WindowTarget {
    let mut slots = WindowSlots::<()>::new(HostId::allocate().unwrap());
    WindowTarget {
        id: slots.reserve().unwrap(),
        generation: WindowGeneration::INITIAL,
    }
}

fn caret() -> TextCaret {
    TextCaret {
        position: PhysicalPosition { x: 12.5, y: -2.0 },
        size: PhysicalSize {
            width: 1,
            height: 20,
        },
    }
}

#[test]
fn replacement_and_end_invalidate_exact_session_without_reusing_identity() {
    let mut text = TextState::new();
    let window = target();
    let first = text.begin(window, caret()).unwrap();
    let second = text.begin(window, caret()).unwrap();
    assert_ne!(first, second);
    assert_eq!(second.target(), window);
    assert!(!text.is_live(first));
    assert!(text.is_live(second));
    assert_eq!(text.end(first), Err(Error::StaleIdentity));
    text.end(second).unwrap();
    assert!(!text.is_live(second));
    assert!(text.active(window).is_none());
    assert_ne!(text.begin(window, caret()).unwrap(), second);
}

#[test]
fn validation_and_exhaustion_preserve_existing_session_and_caret() {
    let mut text = TextState::new();
    let window = target();
    let first = text.begin(window, caret()).unwrap();
    let invalid = TextCaret {
        position: PhysicalPosition {
            x: f64::NAN,
            y: 0.0,
        },
        ..caret()
    };
    assert!(text.begin(window, invalid).is_err());
    assert!(text.update(first, invalid).is_err());
    assert_eq!(text.caret(first).unwrap(), caret());
    assert_eq!(text.active(window), Some(first));
    text.next_serial = u64::MAX;
    assert_eq!(
        text.begin(window, caret()),
        Err(Error::IdentityExhausted(IdentityKind::TextSession))
    );
    assert!(text.is_live(first));
}

#[test]
fn native_composition_keeps_retired_owner_until_disabled() {
    let mut text = TextState::new();
    let window = target();
    let first = text.begin(window, caret()).unwrap();
    text.set_composing(window, true).unwrap();
    text.end(first).unwrap();
    let second = text.begin(window, caret()).unwrap();
    assert_eq!(text.ime_session(window), Some(first));
    assert!(text.composing(window));
    assert!(!text.is_live(text.ime_session(window).unwrap()));
    text.set_composing(window, false).unwrap();
    assert_eq!(text.ime_session(window), Some(second));
    text.retire(window);
    assert!(!text.is_live(second));
    assert!(!text.composing(window));
}

#[test]
fn composition_without_text_owner_cannot_acquire_a_later_session() {
    let mut text = TextState::new();
    let window = target();
    text.set_composing(window, true).unwrap();
    let session = text.begin(window, caret()).unwrap();
    assert!(text.ime_session(window).is_none());
    text.set_composing(window, false).unwrap();
    assert_eq!(text.ime_session(window), Some(session));
    text.clear();
    assert!(!text.is_live(session));
}

#[test]
fn native_caret_rectangle_narrowing_and_endpoint_overflow_are_rejected() {
    for invalid in [
        TextCaret {
            position: PhysicalPosition {
                x: f64::from(i32::MAX),
                y: 0.0,
            },
            ..caret()
        },
        TextCaret {
            position: PhysicalPosition {
                x: f64::from(i32::MIN) - 1.0,
                y: 0.0,
            },
            ..caret()
        },
        TextCaret {
            size: PhysicalSize {
                width: u32::MAX,
                height: 0,
            },
            ..caret()
        },
        TextCaret {
            position: PhysicalPosition {
                x: 0.0,
                y: f64::INFINITY,
            },
            ..caret()
        },
    ] {
        assert!(invalid.validate().is_err());
    }
    assert!(
        TextCaret {
            position: PhysicalPosition {
                x: f64::from(i32::MIN),
                y: f64::from(i32::MAX)
            },
            size: PhysicalSize {
                width: 0,
                height: 0
            },
        }
        .validate()
        .is_ok()
    );
}
