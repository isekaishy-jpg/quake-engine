//! Key names.

use engine_platform::key_name;

#[test]
fn keys_are_named_by_position() {
    assert_eq!(key_name(0x01), Some("ESCAPE"));
    assert_eq!(key_name(0x1E), Some("A"));
    assert_eq!(key_name(0x1C), Some("ENTER"));
}
