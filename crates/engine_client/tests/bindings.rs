//! Key bindings, following `keys.c`.

use engine_client::{Bindings, canonical_key};

#[test]
fn config_names_and_platform_names_meet() {
    assert_eq!(canonical_key("UP"), "UPARROW");
    assert_eq!(canonical_key("uparrow"), "UPARROW");
    assert_eq!(canonical_key("RIGHT CTRL"), "CTRL");
    assert_eq!(canonical_key("w"), "W");
}

#[test]
fn a_plus_binding_presses_and_releases() {
    let mut bindings = Bindings::new();
    bindings.bind("uparrow", "+forward");
    assert_eq!(bindings.press("UP"), Some("+forward"));
    assert_eq!(bindings.release("UP").as_deref(), Some("-forward"));
}

#[test]
fn a_plain_binding_runs_on_press_only() {
    let mut bindings = Bindings::new();
    bindings.bind("1", "impulse 1");
    assert_eq!(bindings.press("1"), Some("impulse 1"));
    assert_eq!(bindings.release("1"), None);
}

#[test]
fn bindings_can_be_replaced_and_removed() {
    let mut bindings = Bindings::new();
    bindings.bind("a", "+lookup");
    bindings.bind("A", "+moveleft");
    assert_eq!(bindings.get("a"), Some("+moveleft"));
    bindings.unbind("a");
    assert_eq!(bindings.get("a"), None);
    bindings.bind("b", "x");
    bindings.clear();
    assert_eq!(bindings.get("b"), None);
}
