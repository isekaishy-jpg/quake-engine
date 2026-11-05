//! The tree both encodings share.

/// The newest version this build reads and writes.
pub const CURRENT_VERSION: u32 = 1;

/// A field's value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A number.
    Number(f64),
    /// Text.
    Text(String),
    /// A flag.
    Flag(bool),
    /// Several values.
    List(Vec<Self>),
}

impl Value {
    /// The number, if this is one.
    #[must_use]
    pub const fn number(&self) -> Option<f64> {
        match self {
            Self::Number(number) => Some(*number),
            _ => None,
        }
    }

    /// The text, if this is text.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The flag, if this is one.
    #[must_use]
    pub const fn flag(&self) -> Option<bool> {
        match self {
            Self::Flag(flag) => Some(*flag),
            _ => None,
        }
    }

    /// The values, if this is a list.
    #[must_use]
    pub fn list(&self) -> Option<&[Self]> {
        match self {
            Self::List(values) => Some(values),
            _ => None,
        }
    }

    /// The numbers of a list, if that's all it holds.
    #[must_use]
    pub fn numbers(&self) -> Option<Vec<f64>> {
        self.list()?.iter().map(Self::number).collect()
    }
}

/// One block: a kind, an optional name, fields and child blocks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Block {
    /// What sort of block it is.
    pub kind: String,
    /// Its name. Names may repeat.
    pub name: Option<String>,
    /// Its fields, in order.
    pub fields: Vec<(String, Value)>,
    /// The blocks inside it.
    pub children: Vec<Self>,
}

impl Block {
    /// A block of `kind`.
    #[must_use]
    pub fn new(kind: &str) -> Self {
        Self {
            kind: kind.to_owned(),
            ..Self::default()
        }
    }

    /// A block of `kind` called `name`.
    #[must_use]
    pub fn named(kind: &str, name: &str) -> Self {
        Self {
            name: Some(name.to_owned()),
            ..Self::new(kind)
        }
    }

    /// The first field called `key`.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.fields
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    /// Adds a field.
    pub fn set(&mut self, key: &str, value: Value) {
        self.fields.push((key.to_owned(), value));
    }

    /// The child blocks of `kind`.
    pub fn children_of<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a Self> {
        self.children.iter().filter(move |child| child.kind == kind)
    }
}

/// A whole document.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    /// Its format version.
    pub version: u32,
    /// What it is, such as `map` or `material`.
    pub kind: String,
    /// Its top level blocks.
    pub blocks: Vec<Block>,
}

impl Document {
    /// An empty document of `kind`, at the current version.
    #[must_use]
    pub fn new(kind: &str) -> Self {
        Self {
            version: CURRENT_VERSION,
            kind: kind.to_owned(),
            blocks: Vec::new(),
        }
    }

    /// The top level blocks of `kind`.
    pub fn blocks_of<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a Block> {
        self.blocks.iter().filter(move |block| block.kind == kind)
    }
}
