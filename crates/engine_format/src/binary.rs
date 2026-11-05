//! The binary encoding, written by compiling and fast to load. It's the same
//! tree as the text, so one holds nothing the other can't.

use crate::document::{Block, CURRENT_VERSION, Document, Value};
use crate::error::FormatError;

/// What a binary document starts with.
const MAGIC: &[u8; 4] = b"EDOC";

/// The deepest blocks may nest, as in text.
const MAX_DEPTH: usize = 64;

/// Marks a number.
const TAG_NUMBER: u8 = 0;
/// Marks text.
const TAG_TEXT: u8 = 1;
/// Marks a flag.
const TAG_FLAG: u8 = 2;
/// Marks a list.
const TAG_LIST: u8 = 3;
/// Marks raw data.
const TAG_BYTES: u8 = 4;

/// Writes a length.
fn put_length(out: &mut Vec<u8>, length: usize) {
    let length = u32::try_from(length).unwrap_or(u32::MAX);
    out.extend_from_slice(&length.to_le_bytes());
}

/// Writes text.
fn put_text(out: &mut Vec<u8>, text: &str) {
    put_length(out, text.len());
    out.extend_from_slice(text.as_bytes());
}

/// Writes a value.
fn put_value(out: &mut Vec<u8>, value: &Value) {
    match value {
        Value::Number(number) => {
            out.push(TAG_NUMBER);
            out.extend_from_slice(&number.to_le_bytes());
        }
        Value::Text(text) => {
            out.push(TAG_TEXT);
            put_text(out, text);
        }
        Value::Flag(flag) => {
            out.push(TAG_FLAG);
            out.push(u8::from(*flag));
        }
        Value::Bytes(bytes) => {
            out.push(TAG_BYTES);
            put_length(out, bytes.len());
            out.extend_from_slice(bytes);
        }
        Value::List(values) => {
            out.push(TAG_LIST);
            put_length(out, values.len());
            for value in values {
                put_value(out, value);
            }
        }
    }
}

/// Writes a block.
fn put_block(out: &mut Vec<u8>, block: &Block) {
    put_text(out, &block.kind);
    match &block.name {
        Some(name) => {
            out.push(1);
            put_text(out, name);
        }
        None => out.push(0),
    }
    put_length(out, block.fields.len());
    for (key, value) in &block.fields {
        put_text(out, key);
        put_value(out, value);
    }
    put_length(out, block.children.len());
    for child in &block.children {
        put_block(out, child);
    }
}

/// Writes a document as binary.
#[must_use]
pub fn to_binary(document: &Document) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&document.version.to_le_bytes());
    put_text(&mut out, &document.kind);
    put_length(&mut out, document.blocks.len());
    for block in &document.blocks {
        put_block(&mut out, block);
    }
    out
}

/// Reads from bytes, refusing anything that runs past their end.
struct Reader<'a> {
    /// What's left.
    bytes: &'a [u8],
}

impl Reader<'_> {
    /// An error.
    fn bad(message: &str) -> FormatError {
        FormatError::Binary(message.to_owned())
    }

    /// Takes `count` bytes.
    fn take(&mut self, count: usize) -> Result<&[u8], FormatError> {
        if count > self.bytes.len() {
            return Err(Self::bad("it ends early"));
        }
        let (taken, rest) = self.bytes.split_at(count);
        self.bytes = rest;
        Ok(taken)
    }

    /// Reads a byte.
    fn byte(&mut self) -> Result<u8, FormatError> {
        Ok(self.take(1)?[0])
    }

    /// Reads a `u32`.
    fn number(&mut self) -> Result<u32, FormatError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Reads a count of things at least `smallest` bytes each, refusing one
    /// that can't fit in what's left.
    fn count(&mut self, smallest: usize) -> Result<usize, FormatError> {
        let count = usize::try_from(self.number()?).map_err(|_| Self::bad("a huge count"))?;
        if count.saturating_mul(smallest) > self.bytes.len() {
            return Err(Self::bad("a count larger than the file"));
        }
        Ok(count)
    }

    /// Reads text.
    fn text(&mut self) -> Result<String, FormatError> {
        let length = self.count(1)?;
        let bytes = self.take(length)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| Self::bad("text that isn't UTF-8"))
    }

    /// Reads a value.
    fn value(&mut self, depth: usize) -> Result<Value, FormatError> {
        if depth >= MAX_DEPTH {
            return Err(Self::bad("lists nest too deeply"));
        }
        match self.byte()? {
            TAG_NUMBER => {
                let bytes = self.take(8)?;
                let mut number = [0; 8];
                number.copy_from_slice(bytes);
                Ok(Value::Number(f64::from_le_bytes(number)))
            }
            TAG_TEXT => Ok(Value::Text(self.text()?)),
            TAG_FLAG => match self.byte()? {
                0 => Ok(Value::Flag(false)),
                1 => Ok(Value::Flag(true)),
                _ => Err(Self::bad("a flag that's neither")),
            },
            TAG_BYTES => {
                let length = self.count(1)?;
                Ok(Value::Bytes(self.take(length)?.to_vec()))
            }
            TAG_LIST => {
                let count = self.count(2)?;
                let mut values = Vec::with_capacity(count);
                for _ in 0..count {
                    values.push(self.value(depth + 1)?);
                }
                Ok(Value::List(values))
            }
            _ => Err(Self::bad("an unknown value")),
        }
    }

    /// Reads a block.
    fn block(&mut self, depth: usize) -> Result<Block, FormatError> {
        if depth >= MAX_DEPTH {
            return Err(Self::bad("blocks nest too deeply"));
        }
        let kind = self.text()?;
        let name = match self.byte()? {
            0 => None,
            1 => Some(self.text()?),
            _ => return Err(Self::bad("a bad name marker")),
        };
        let count = self.count(5)?;
        let mut fields = Vec::with_capacity(count);
        for _ in 0..count {
            let key = self.text()?;
            fields.push((key, self.value(0)?));
        }
        let count = self.count(9)?;
        let mut children = Vec::with_capacity(count);
        for _ in 0..count {
            children.push(self.block(depth + 1)?);
        }
        Ok(Block {
            kind,
            name,
            fields,
            children,
        })
    }
}

/// Reads a document from binary.
///
/// # Errors
///
/// Returns [`FormatError::Binary`] for anything malformed, and
/// [`FormatError::TooNew`] for a version this build doesn't know.
pub fn from_binary(bytes: &[u8]) -> Result<Document, FormatError> {
    let mut reader = Reader { bytes };
    if reader.take(4)? != MAGIC {
        return Err(Reader::bad("not a document"));
    }
    let version = reader.number()?;
    if version > CURRENT_VERSION {
        return Err(FormatError::TooNew(version));
    }
    let kind = reader.text()?;
    let count = reader.count(9)?;
    let mut blocks = Vec::with_capacity(count);
    for _ in 0..count {
        blocks.push(reader.block(0)?);
    }
    if !reader.bytes.is_empty() {
        return Err(Reader::bad("bytes after the end"));
    }
    Ok(Document {
        version,
        kind,
        blocks,
    })
}
