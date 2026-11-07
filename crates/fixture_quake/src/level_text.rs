//! What the rerelease's levels say, in words.
//!
//! The rerelease's levels hold keys in place of their messages, such as `$map_door_elsewhere`, and
//! its words for them aren't in any file we can read. The original level says the same thing in
//! plain words, on the same entity, so that's where they come from. One key can read differently
//! from one level to the next, so it goes by the entity, and there's no table of them.

/// Where a level's header says its entities are.
const ENTITIES_AT: usize = 4;

/// What marks a key.
const KEY_MARK: char = '$';

/// A level's entities as text, a character to each byte, as they're not all ASCII.
fn entities_of(level: &[u8]) -> Option<String> {
    let number = |at: usize| {
        let bytes = level.get(at..at + 4)?;
        let bytes: [u8; 4] = bytes.try_into().ok()?;
        usize::try_from(u32::from_le_bytes(bytes)).ok()
    };
    let (start, length) = (number(ENTITIES_AT)?, number(ENTITIES_AT + 4)?);
    let text = level.get(start..start.checked_add(length)?)?;
    let end = text
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(text.len());
    Some(text[..end].iter().map(|byte| char::from(*byte)).collect())
}

/// Each entity in `text`, with its braces. Braces in a string don't count.
fn blocks(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut quoted, mut start) = (false, None);
    for (at, letter) in text.char_indices() {
        match letter {
            '"' => quoted = !quoted,
            '{' if !quoted => start = Some(at),
            '}' if !quoted => {
                if let Some(from) = start.take() {
                    out.push(&text[from..=at]);
                }
            }
            _ => {}
        }
    }
    out
}

/// An entity's fields, in order.
fn fields(block: &str) -> Vec<(&str, &str)> {
    let strings: Vec<&str> = block.split('"').skip(1).step_by(2).collect();
    strings
        .as_chunks::<2>()
        .0
        .iter()
        .map(|[key, value]| (*key, *value))
        .collect()
}

/// What a field of an entity is.
fn field<'a>(fields: &[(&'a str, &'a str)], name: &str) -> Option<&'a str> {
    fields
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| *value)
}

/// Whether two entities are the same one of a level, in both its versions.
fn same_entity(one: &[(&str, &str)], other: &[(&str, &str)]) -> bool {
    ["classname", "targetname", "target", "origin"]
        .iter()
        .all(|name| field(one, name) == field(other, name))
}

/// Something to show for a key with no words to be found: what it's called.
fn words_of(key: &str) -> String {
    let name = key.trim_start_matches(KEY_MARK);
    let name = name.strip_prefix("map_").unwrap_or(name);
    let name = name.replace('_', " ");
    let mut letters = name.chars();
    letters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(letters).collect()
    })
}

/// `block` with its keys in words, from `originals` where that entity is there.
fn in_words(block: &str, originals: &[Vec<(&str, &str)>]) -> String {
    let own = fields(block);
    let original = originals.iter().find(|other| same_entity(&own, other));
    let mut out = block.to_owned();
    for (name, value) in &own {
        if !value.starts_with(KEY_MARK) {
            continue;
        }
        let said = original.and_then(|other| field(other, name));
        let said = said.filter(|words| !words.starts_with(KEY_MARK));
        let words = said.map_or_else(|| words_of(value), str::to_owned);
        out = out.replace(&format!("\"{value}\""), &format!("\"{words}\""));
    }
    out
}

/// `level` with its messages in words, from the same level as it first was, if there is one.
/// What's there goes on the end, and the header's told, so nothing else in the level moves.
#[must_use]
pub fn restore_messages(level: &[u8], original: Option<&[u8]>) -> Vec<u8> {
    let Some(text) = entities_of(level).filter(|text| text.contains("\"$")) else {
        return level.to_vec();
    };
    let original = original.and_then(entities_of).unwrap_or_default();
    let originals: Vec<Vec<(&str, &str)>> = blocks(&original).into_iter().map(fields).collect();
    let mut restored = String::new();
    for block in blocks(&text) {
        restored.push_str(&in_words(block, &originals));
        restored.push('\n');
    }
    let mut out = level.to_vec();
    out.resize(out.len().next_multiple_of(4), 0);
    let start = out.len();
    // Each character is a byte, as it was made from them, or came from a level that was.
    out.extend(
        restored
            .chars()
            .map(|letter| u8::try_from(letter).unwrap_or(b'?')),
    );
    out.push(0);
    let length = out.len() - start;
    for (at, value) in [(ENTITIES_AT, start), (ENTITIES_AT + 4, length)] {
        let value = u32::try_from(value).unwrap_or(0).to_le_bytes();
        out[at..at + 4].copy_from_slice(&value);
    }
    out
}
