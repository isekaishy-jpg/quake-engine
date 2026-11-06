//! Reading and writing documents.

use engine_format::{
    Block, CURRENT_VERSION, Document, FormatError, Value, from_binary, from_text, to_binary,
    to_text,
};

/// A document using every kind of value.
fn sample() -> Document {
    let mut entity = Block::named("entity", "elevator_1");
    entity.set("type", Value::Text("elevator".to_owned()));
    entity.set(
        "origin",
        Value::List(vec![
            Value::Number(512.0),
            Value::Number(-256.5),
            Value::Number(0.1),
        ]),
    );
    entity.set("awake", Value::Flag(true));
    entity.set("note", Value::Text("a \"quoted\"\nline\\".to_owned()));
    entity.set("tiny", Value::Number(1.000_000_000_000_000_2));
    entity.set(
        "nested",
        Value::List(vec![
            Value::List(vec![Value::Number(1.0)]),
            Value::Flag(false),
        ]),
    );
    entity.set("data", Value::Bytes(vec![0, 1, 254, 255]));
    entity.children.push(Block::new("part"));
    let mut document = Document::new("map");
    document.blocks.push(entity);
    document.blocks.push(Block::named("entity", "two words"));
    document
}

#[test]
fn text_reads_back_exactly() {
    let document = sample();
    assert_eq!(from_text(&to_text(&document)).expect("parses"), document);
}

#[test]
fn binary_reads_back_exactly() {
    let document = sample();
    assert_eq!(
        from_binary(&to_binary(&document)).expect("parses"),
        document
    );
}

#[test]
fn text_and_binary_of_one_document_agree() {
    let document = sample();
    let text = from_text(&to_text(&document)).expect("parses");
    let binary = from_binary(&to_binary(&document)).expect("parses");
    assert_eq!(text, binary);
}

#[test]
fn hand_written_text_reads_as_expected() {
    let text = "# a comment\nformat 1 map\n\nentity elevator_1 {\n    origin = 512 256 64\n    speed = 150 # fast\n    kind = lift\n}\nsky {\n}\n";
    let document = from_text(text).expect("parses");
    assert_eq!(document.kind, "map");
    let entity = document.blocks_of("entity").next().expect("an entity");
    assert_eq!(entity.name.as_deref(), Some("elevator_1"));
    assert_eq!(
        entity.get("origin").and_then(Value::numbers),
        Some(vec![512.0, 256.0, 64.0])
    );
    assert_eq!(entity.get("speed").and_then(Value::number), Some(150.0));
    assert_eq!(entity.get("kind").and_then(Value::text), Some("lift"));
    assert_eq!(document.blocks_of("sky").count(), 1);
}

#[test]
fn fields_can_share_a_line() {
    let document = from_text(
        "format 1 map
brush { material = floor cuboid = 0 0 0 1 1 1 }
",
    );
    let document = document.expect("parses");
    let brush = document.blocks_of("brush").next().expect("a brush");
    assert_eq!(brush.get("material").and_then(Value::text), Some("floor"));
    assert_eq!(
        brush
            .get("cuboid")
            .and_then(Value::numbers)
            .map(|n| n.len()),
        Some(6)
    );
}

#[test]
fn names_can_repeat() {
    let text = "format 1 map\nentity door { }\nentity door { }\n";
    let document = from_text(text).expect("parses");
    assert_eq!(document.blocks_of("entity").count(), 2);
}

#[test]
fn bad_text_says_which_line() {
    let cases = [
        ("format 1 map\nentity {\n", 3),
        ("format 1 map\n\n\nentity a b {\n}\n", 4),
        ("format 1 map\nentity {\n  x =\n}\n", 3),
        ("format 1 map\nentity {\n  x = \"open\n}\n", 3),
    ];
    for (text, line) in cases {
        match from_text(text) {
            Err(FormatError::Text { line: found, .. }) => assert_eq!(found, line, "{text}"),
            other => panic!("{text}: {other:?}"),
        }
    }
    assert!(from_text("").is_err(), "no header");
    assert!(from_text("entity { }").is_err(), "no format line");
}

#[test]
fn a_newer_version_is_refused_clearly() {
    let newer = CURRENT_VERSION + 1;
    let text = format!("format {newer} map\n");
    assert_eq!(from_text(&text), Err(FormatError::TooNew(newer)));
    let mut document = sample();
    document.version = newer;
    assert_eq!(
        from_binary(&to_binary(&document)),
        Err(FormatError::TooNew(newer))
    );
}

#[test]
fn every_truncation_of_binary_is_refused() {
    let bytes = to_binary(&sample());
    for length in 0..bytes.len() {
        assert!(from_binary(&bytes[..length]).is_err(), "at {length}");
    }
}

#[test]
fn a_huge_count_is_refused_without_allocating() {
    let mut bytes = to_binary(&Document::new("map"));
    let at = bytes.len() - 4;
    bytes[at..].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(from_binary(&bytes).is_err());
}

#[test]
fn deep_nesting_is_refused() {
    let text = format!("format 1 map\n{}", "a {\n".repeat(200));
    assert!(from_text(&text).is_err());
    let list = format!("format 1 map\na {{\n x = {}\n}}\n", "[".repeat(200));
    assert!(from_text(&list).is_err());
    let mut block = Block::new("a");
    for _ in 0..200 {
        let mut outer = Block::new("a");
        outer.children.push(block);
        block = outer;
    }
    let mut document = Document::new("map");
    document.blocks.push(block);
    assert!(from_binary(&to_binary(&document)).is_err());
}

/// A list ends where a block starts, on one line as on many.
#[test]
fn a_block_can_follow_a_list_on_the_same_line() {
    let one_line = "format 1 map
entity x { light = [1 0 0] brush { size = 2 } }
";
    let many_lines = "format 1 map
entity x {
    light = [1 0 0]
    brush {
        size = 2
    }
}
";
    let (one, many) = (from_text(one_line), from_text(many_lines));
    assert_eq!(one.expect("one line"), many.expect("many lines"));
}

#[test]
fn a_named_block_can_follow_several_values_on_the_same_line() {
    let parsed = from_text(
        "format 1 map
entity x { spot = 1 2 3 part wheel { size = 2 } }
",
    );
    let document = parsed.expect("parses");
    let block = &document.blocks[0];
    assert_eq!(block.fields.len(), 1, "{block:?}");
    assert_eq!(block.children[0].name.as_deref(), Some("wheel"));
}

/// A word is still a value where no block follows it.
#[test]
fn words_in_a_list_stay_words() {
    let parsed = from_text(
        "format 1 map
entity x { tags = one two three }
",
    );
    let document = parsed.expect("parses");
    assert_eq!(document.blocks[0].fields.len(), 1);
}
