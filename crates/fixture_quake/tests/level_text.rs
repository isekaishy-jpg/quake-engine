//! What the rerelease's levels say, in words.

use fixture_quake::{parse_entities, restore_messages};

/// A level holding only `entities`, with something after them that has to stay put.
fn level(entities: &str) -> Vec<u8> {
    let mut out = vec![0; 124];
    out[..4].copy_from_slice(&29_u32.to_le_bytes());
    out[4..8].copy_from_slice(&124_u32.to_le_bytes());
    let length = u32::try_from(entities.len() + 1).expect("short");
    out[8..12].copy_from_slice(&length.to_le_bytes());
    out.extend_from_slice(entities.as_bytes());
    out.push(0);
    out.extend_from_slice(b"the rest");
    out
}

/// A level's entities as text.
fn text_of(level: &[u8]) -> String {
    let word = |at: usize| {
        let bytes: [u8; 4] = level[at..at + 4].try_into().expect("four");
        usize::try_from(u32::from_le_bytes(bytes)).expect("fits")
    };
    let (start, length) = (word(4), word(8));
    String::from_utf8_lossy(&level[start..start + length - 1]).into_owned()
}

/// A door that says `message`.
fn door(target: &str, message: &str) -> String {
    format!(
        "{{\n\"classname\" \"func_door\"\n\"targetname\" \"{target}\"\n\"message\" \"{message}\"\n}}\n"
    )
}

/// A key takes the words of the same entity in the original.
#[test]
fn a_key_takes_the_original_words() {
    let new = level(&door("a", "$map_door_elsewhere"));
    let old = level(&door("a", "This door is opened elsewhere."));
    let text = text_of(&restore_messages(&new, Some(&old)));
    assert!(
        text.contains("\"This door is opened elsewhere.\""),
        "{text}"
    );
    assert!(!text.contains('$'), "{text}");
}

/// Two entities with the same key can say different things.
#[test]
fn it_goes_by_the_entity() {
    let new = level(&(door("a", "$map_door") + &door("b", "$map_door")));
    let old = level(&(door("b", "Second") + &door("a", "First")));
    let text = text_of(&restore_messages(&new, Some(&old)));
    let (first, second) = (text.find("First"), text.find("Second"));
    assert!(first.is_some() && second.is_some(), "{text}");
    assert!(first < second, "{text}");
}

/// A level with no keys is left as it is, to the byte.
#[test]
fn a_level_with_no_keys_is_left_alone() {
    let new = level(&door("a", "Plain words"));
    assert_eq!(restore_messages(&new, None), new);
}

/// Only what's a key changes, and other fields stay as they were.
#[test]
fn other_fields_are_kept() {
    let new = level(&door("a", "$map_door"));
    let old = level(&door("a", "Words"));
    let text = text_of(&restore_messages(&new, Some(&old)));
    assert!(text.contains("\"classname\" \"func_door\""), "{text}");
    assert!(text.contains("\"targetname\" \"a\""), "{text}");
}

/// A field that isn't a key stays as the rerelease has it, even if the original differs.
#[test]
fn only_keys_come_from_the_original() {
    let with_wait = |wait: &str, message: &str| {
        format!(
            "{{\n\"classname\" \"func_door\"\n\"wait\" \"{wait}\"\n\"message\" \"{message}\"\n}}\n"
        )
    };
    let new = level(&with_wait("2", "$map_door"));
    let old = level(&with_wait("9", "Words"));
    let text = text_of(&restore_messages(&new, Some(&old)));
    assert!(text.contains("\"wait\" \"2\""), "{text}");
    assert!(text.contains("\"Words\""), "{text}");
}

/// With no original to go by, it says what the key's called, and not a key.
#[test]
fn a_key_with_no_words_is_spelled_out() {
    let new = level(&door("a", "$map_door_elsewhere"));
    let text = text_of(&restore_messages(&new, None));
    assert!(text.contains("\"Door elsewhere\""), "{text}");
    let elsewhere = level(&door("z", "Not this one"));
    let text = text_of(&restore_messages(&new, Some(&elsewhere)));
    assert!(text.contains("\"Door elsewhere\""), "{text}");
}

/// An original that's a key itself is no help.
#[test]
fn an_original_key_is_not_words() {
    let new = level(&door("a", "$map_door_here"));
    let old = level(&door("a", "$map_other"));
    let text = text_of(&restore_messages(&new, Some(&old)));
    assert!(text.contains("\"Door here\""), "{text}");
}

/// What's after the entities stays where it is, as the rest of a level goes by the header.
#[test]
fn the_rest_of_a_level_stays_put() {
    let new = level(&door("a", "$map_door"));
    let old = level(&door(
        "a",
        "A much longer message than the key it stands in for",
    ));
    let out = restore_messages(&new, Some(&old));
    assert_eq!(&out[..4], &new[..4]);
    assert_eq!(&out[12..new.len()], &new[12..]);
}

/// What comes out is still a list of entities.
#[test]
fn it_is_still_entities() {
    let new = level(&(door("a", "$map_door") + &door("b", "Plain")));
    let old = level(&door("a", "Words"));
    let text = text_of(&restore_messages(&new, Some(&old)));
    let entities = parse_entities(&text);
    assert_eq!(entities.len(), 2);
}

/// A level too short to be one is left as it is.
#[test]
fn what_is_not_a_level_is_left_alone() {
    assert_eq!(restore_messages(&[1, 2, 3], None), [1, 2, 3]);
}
