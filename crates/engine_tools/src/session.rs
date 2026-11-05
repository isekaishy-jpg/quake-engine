//! A map being edited, with its history.

use std::path::{Path, PathBuf};

use engine_format::{Block, Document, Value, from_text, to_text};

/// The field holding a placed thing's hidden ID.
pub(crate) const ID_FIELD: &str = "id";

/// How many steps undo remembers.
const HISTORY_LIMIT: usize = 256;

/// A map being edited.
#[derive(Clone, Debug)]
pub struct Session {
    /// The map.
    pub(crate) document: Document,
    /// Where it's saved, if it has been.
    pub(crate) path: Option<PathBuf>,
    /// The content directory, for prefabs, materials and compiling.
    pub(crate) content: PathBuf,
    /// What undo goes back to, oldest first.
    undo: Vec<Document>,
    /// What redo goes forward to.
    redo: Vec<Document>,
    /// The next ID to give out.
    next_id: u64,
    /// Whether it's changed since saving.
    pub(crate) dirty: bool,
}

/// A block's ID.
pub(crate) fn id_of(block: &Block) -> Option<u64> {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "IDs are whole numbers written here"
    )]
    block
        .get(ID_FIELD)
        .and_then(Value::number)
        .map(|id| id as u64)
}

impl Session {
    /// A new empty map, working in `content`.
    #[must_use]
    pub fn new(content: &Path) -> Self {
        Self {
            document: Document::new("map"),
            path: None,
            content: content.to_owned(),
            undo: Vec::new(),
            redo: Vec::new(),
            next_id: 1,
            dirty: false,
        }
    }

    /// Opens the map at `path`, giving IDs to anything without one.
    ///
    /// # Errors
    ///
    /// Returns a message if it can't be read.
    pub fn open(path: &Path, content: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        let document = from_text(&text).map_err(|error| error.to_string())?;
        let mut session = Self::new(content);
        session.document = document;
        session.path = Some(path.to_owned());
        session.next_id = session
            .document
            .blocks
            .iter()
            .filter_map(id_of)
            .max()
            .map_or(1, |highest| highest + 1);
        let mut blocks = std::mem::take(&mut session.document.blocks);
        for block in &mut blocks {
            if id_of(block).is_none() {
                session.give_id(block);
            }
        }
        session.document.blocks = blocks;
        Ok(session)
    }

    /// Saves to `path`, or where it was last saved.
    ///
    /// # Errors
    ///
    /// Returns a message if there's nowhere to save or writing fails.
    pub fn save(&mut self, path: Option<&Path>) -> Result<PathBuf, String> {
        let target = path
            .map(Path::to_owned)
            .or_else(|| self.path.clone())
            .ok_or("nowhere to save: give a path")?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        std::fs::write(&target, to_text(&self.document)).map_err(|error| error.to_string())?;
        self.path = Some(target.clone());
        self.dirty = false;
        Ok(target)
    }

    /// Where it's saved, if it has been.
    #[must_use]
    pub fn saved_path(&self) -> Option<PathBuf> {
        self.path.clone()
    }

    /// The map as text.
    #[must_use]
    pub fn text(&self) -> String {
        to_text(&self.document)
    }

    /// Gives `block` a new ID.
    pub(crate) fn give_id(&mut self, block: &mut Block) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        #[expect(clippy::cast_precision_loss, reason = "IDs stay far below 2^53")]
        block.set(ID_FIELD, Value::Number(id as f64));
        id
    }

    /// Remembers the map before a change.
    pub(crate) fn checkpoint(&mut self) {
        self.undo.push(self.document.clone());
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    /// Goes back a step. Returns whether there was one.
    pub(crate) fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo
            .push(std::mem::replace(&mut self.document, previous));
        self.dirty = true;
        true
    }

    /// Goes forward a step. Returns whether there was one.
    pub(crate) fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(std::mem::replace(&mut self.document, next));
        self.dirty = true;
        true
    }

    /// The block with `id`.
    pub(crate) fn find_mut(&mut self, id: u64) -> Option<&mut Block> {
        self.document
            .blocks
            .iter_mut()
            .find(|block| id_of(block) == Some(id))
    }
}
