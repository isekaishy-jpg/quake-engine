//! The entity lump.

use fixture_quake::parse_entities;

#[test]
fn pairs_are_read_in_order() {
    let entities = parse_entities(
        r#"{
"classname" "worldspawn"
"message" "The Slipgate Complex"
}
{
"classname" "light"
}"#,
    );
    assert_eq!(entities.len(), 2);
    assert_eq!(entities[0].get("message"), Some("The Slipgate Complex"));
    assert_eq!(entities[1].get("classname"), Some("light"));
}

#[test]
fn trailing_spaces_leave_a_key_but_not_a_value() {
    let entities = parse_entities(r#"{ "map " "r2m2" "message" "hello " }"#);
    assert_eq!(entities[0].get("map"), Some("r2m2"));
    assert_eq!(entities[0].get("message"), Some("hello "));
}
