//! The text encoding, for files people and tools read and diff.

use std::fmt::Write as _;
use std::iter::Peekable;
use std::str::Chars;

use crate::document::{Block, CURRENT_VERSION, Document, Value};
use crate::error::FormatError;

/// The deepest blocks may nest, so a bad file can't exhaust the stack.
const MAX_DEPTH: usize = 64;

/// One piece of text.
#[derive(Clone, Debug, PartialEq)]
enum Token {
    /// A bare word.
    Word(String),
    /// A quoted string.
    Quoted(String),
    /// A number.
    Number(f64),
    /// `{`.
    Open,
    /// `}`.
    Close,
    /// `[`.
    ListOpen,
    /// `]`.
    ListClose,
    /// `=`.
    Equals,
    /// The end of a line.
    Line,
}

/// Whether `c` can be part of a bare word.
fn word_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | '+' | ':')
}

/// Turns text into tokens, each with its line.
struct Lexer<'a> {
    /// What's left.
    chars: Peekable<Chars<'a>>,
    /// The line reached.
    line: usize,
}

impl Lexer<'_> {
    /// An error on the current line.
    fn error(&self, message: &str) -> FormatError {
        FormatError::Text {
            line: self.line,
            message: message.to_owned(),
        }
    }

    /// Reads a quoted string, past its opening quote.
    fn quoted(&mut self) -> Result<String, FormatError> {
        let mut out = String::new();
        loop {
            let Some(c) = self.chars.next() else {
                return Err(self.error("a string has no closing quote"));
            };
            match c {
                '"' => return Ok(out),
                '\n' => return Err(self.error("a string runs past the end of its line")),
                '\\' => match self.chars.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    _ => return Err(self.error("unknown escape in a string")),
                },
                _ => out.push(c),
            }
        }
    }

    /// Reads a word or number starting with `first`.
    fn word(&mut self, first: char) -> Token {
        let mut word = String::from(first);
        while let Some(c) = self.chars.peek().copied() {
            if !word_char(c) {
                break;
            }
            word.push(c);
            self.chars.next();
        }
        let numeric = first.is_ascii_digit() || matches!(first, '-' | '+' | '.');
        match word.parse::<f64>() {
            Ok(number) if numeric => Token::Number(number),
            _ => Token::Word(word),
        }
    }

    /// Every token and the line it's on.
    fn run(mut self) -> Result<Vec<(Token, usize)>, FormatError> {
        let mut tokens = Vec::new();
        while let Some(c) = self.chars.next() {
            let token = match c {
                '\n' => {
                    tokens.push((Token::Line, self.line));
                    self.line += 1;
                    continue;
                }
                '#' => {
                    while self.chars.peek().is_some_and(|c| *c != '\n') {
                        self.chars.next();
                    }
                    continue;
                }
                c if c.is_whitespace() => continue,
                '{' => Token::Open,
                '}' => Token::Close,
                '[' => Token::ListOpen,
                ']' => Token::ListClose,
                '=' => Token::Equals,
                '"' => Token::Quoted(self.quoted()?),
                c if word_char(c) => self.word(c),
                c => return Err(self.error(&format!("unexpected character {c:?}"))),
            };
            tokens.push((token, self.line));
        }
        tokens.push((Token::Line, self.line));
        Ok(tokens)
    }
}

/// Reads tokens into a document.
struct Parser {
    /// The tokens, with their lines.
    tokens: Vec<(Token, usize)>,
    /// The next one.
    at: usize,
}

impl Parser {
    /// The next token, without taking it.
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at).map(|(token, _)| token)
    }

    /// The line reached.
    fn line(&self) -> usize {
        self.tokens
            .get(self.at)
            .or_else(|| self.tokens.last())
            .map_or(1, |(_, line)| *line)
    }

    /// An error at the current token.
    fn error(&self, message: &str) -> FormatError {
        FormatError::Text {
            line: self.line(),
            message: message.to_owned(),
        }
    }

    /// Takes the next token.
    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.at).map(|(token, _)| token.clone());
        self.at += 1;
        token
    }

    /// Steps back over the token just taken, for an error to point at it.
    fn back(&mut self) {
        self.at = self.at.saturating_sub(1);
    }

    /// Skips line ends.
    fn skip_lines(&mut self) {
        while self.peek() == Some(&Token::Line) {
            self.at += 1;
        }
    }

    /// Reads one value.
    fn scalar(&mut self, depth: usize) -> Result<Value, FormatError> {
        match self.next() {
            Some(Token::Number(number)) => Ok(Value::Number(number)),
            Some(Token::Quoted(text)) => Ok(Value::Text(text)),
            Some(Token::Word(word)) => match word.as_str() {
                "true" => Ok(Value::Flag(true)),
                "false" => Ok(Value::Flag(false)),
                _ => Ok(Value::Text(word)),
            },
            Some(Token::ListOpen) => {
                if depth >= MAX_DEPTH {
                    return Err(self.error("lists nest too deeply"));
                }
                let mut values = Vec::new();
                loop {
                    self.skip_lines();
                    if self.peek() == Some(&Token::ListClose) {
                        self.at += 1;
                        return Ok(Value::List(values));
                    }
                    if self.peek().is_none() {
                        return Err(self.error("a list has no closing bracket"));
                    }
                    values.push(self.scalar(depth + 1)?);
                }
            }
            _ => {
                self.back();
                Err(self.error("expected a value"))
            }
        }
    }

    /// Whether a new field starts here, as it can on the same line.
    fn at_field(&self) -> bool {
        matches!(self.peek(), Some(Token::Word(_)))
            && matches!(self.tokens.get(self.at + 1), Some((Token::Equals, _)))
    }

    /// Reads a field's value: one value, or several making a list.
    fn value(&mut self) -> Result<Value, FormatError> {
        let first = self.scalar(0)?;
        let mut rest = Vec::new();
        while !matches!(self.peek(), Some(Token::Line | Token::Close) | None) && !self.at_field() {
            rest.push(self.scalar(0)?);
        }
        if rest.is_empty() {
            Ok(first)
        } else {
            rest.insert(0, first);
            Ok(Value::List(rest))
        }
    }

    /// Reads a block's inside, past its `{`.
    fn body(&mut self, block: &mut Block, depth: usize) -> Result<(), FormatError> {
        if depth >= MAX_DEPTH {
            return Err(self.error("blocks nest too deeply"));
        }
        loop {
            self.skip_lines();
            match self.next() {
                Some(Token::Close) => return Ok(()),
                None => return Err(self.error("a block has no closing brace")),
                Some(Token::Word(word)) => {
                    if self.peek() == Some(&Token::Equals) {
                        self.at += 1;
                        let value = self.value()?;
                        block.fields.push((word, value));
                    } else {
                        block.children.push(self.block(word, depth + 1)?);
                    }
                }
                _ => {
                    self.back();
                    return Err(self.error("expected a field or a block"));
                }
            }
        }
    }

    /// Takes the `{` after a block's name.
    fn open(&mut self) -> Result<(), FormatError> {
        if self.next() == Some(Token::Open) {
            Ok(())
        } else {
            self.back();
            Err(self.error("expected {"))
        }
    }

    /// Reads a block, past its kind.
    fn block(&mut self, kind: String, depth: usize) -> Result<Block, FormatError> {
        let mut block = Block {
            kind,
            ..Block::default()
        };
        match self.next() {
            Some(Token::Word(name) | Token::Quoted(name)) => {
                block.name = Some(name);
                self.open()?;
            }
            Some(Token::Number(number)) => {
                block.name = Some(number.to_string());
                self.open()?;
            }
            Some(Token::Open) => {}
            _ => {
                self.back();
                return Err(self.error("expected a name or {"));
            }
        }
        self.body(&mut block, depth)?;
        Ok(block)
    }

    /// Reads the `format` line.
    fn header(&mut self) -> Result<(u32, String), FormatError> {
        self.skip_lines();
        if self.next() != Some(Token::Word("format".to_owned())) {
            self.back();
            return Err(self.error("a document starts with format"));
        }
        let Some(Token::Number(version)) = self.next() else {
            self.back();
            return Err(self.error("format needs a version"));
        };
        if version < 0.0 || version.fract() != 0.0 || version > f64::from(u32::MAX) {
            self.back();
            return Err(self.error("a version is a whole number"));
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "checked to be a whole number in range above"
        )]
        let version = version as u32;
        if version > CURRENT_VERSION {
            return Err(FormatError::TooNew(version));
        }
        let Some(Token::Word(kind)) = self.next() else {
            self.back();
            return Err(self.error("format needs a kind"));
        };
        Ok((version, kind))
    }
}

/// Reads a document from text.
///
/// # Errors
///
/// Returns [`FormatError::Text`] for what doesn't parse, and
/// [`FormatError::TooNew`] for a version this build doesn't know.
pub fn from_text(text: &str) -> Result<Document, FormatError> {
    let lexer = Lexer {
        chars: text.chars().peekable(),
        line: 1,
    };
    let mut parser = Parser {
        tokens: lexer.run()?,
        at: 0,
    };
    let (version, kind) = parser.header()?;
    let mut blocks = Vec::new();
    loop {
        parser.skip_lines();
        match parser.next() {
            None => break,
            Some(Token::Word(word)) => blocks.push(parser.block(word, 0)?),
            _ => {
                parser.back();
                return Err(parser.error("expected a block"));
            }
        }
    }
    Ok(Document {
        version,
        kind,
        blocks,
    })
}

/// Whether `word` can be written without quotes and read back as itself.
fn bare(word: &str) -> bool {
    !word.is_empty()
        && word.chars().all(word_char)
        && !matches!(word, "true" | "false")
        && word.parse::<f64>().is_err()
}

/// Writes `text` quoted.
fn write_quoted(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Writes a number so that it reads back exactly. Rust's float formatting
/// gives the shortest text that does.
fn write_number(out: &mut String, number: f64) {
    if number.is_finite() {
        let _ = write!(out, "{number}");
    } else {
        // Nothing here makes these, and they have no text form to read back.
        out.push('0');
    }
}

/// Writes one value.
fn write_value(out: &mut String, value: &Value) {
    match value {
        Value::Number(number) => write_number(out, *number),
        Value::Text(text) => write_quoted(out, text),
        Value::Flag(flag) => out.push_str(if *flag { "true" } else { "false" }),
        Value::List(values) => {
            out.push('[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    out.push(' ');
                }
                write_value(out, value);
            }
            out.push(']');
        }
    }
}

/// Writes a block.
fn write_block(out: &mut String, block: &Block, depth: usize) {
    let indent = "    ".repeat(depth);
    out.push_str(&indent);
    out.push_str(&block.kind);
    if let Some(name) = &block.name {
        out.push(' ');
        if bare(name) {
            out.push_str(name);
        } else {
            write_quoted(out, name);
        }
    }
    out.push_str(" {\n");
    for (key, value) in &block.fields {
        out.push_str(&indent);
        out.push_str("    ");
        out.push_str(key);
        out.push_str(" = ");
        write_value(out, value);
        out.push('\n');
    }
    for child in &block.children {
        write_block(out, child, depth + 1);
    }
    out.push_str(&indent);
    out.push_str("}\n");
}

/// Writes a document as text.
#[must_use]
pub fn to_text(document: &Document) -> String {
    let mut out = format!("format {} {}\n", document.version, document.kind);
    for block in &document.blocks {
        out.push('\n');
        write_block(&mut out, block, 0);
    }
    out
}
