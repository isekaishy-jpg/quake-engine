//! A map being edited, with its history.

use std::path::{Component, Path, PathBuf};

use engine_format::{Block, Document, Value, from_text, to_text};

/// The field holding a placed thing's hidden ID.
pub(crate) const ID_FIELD: &str = "id";

/// How many steps undo remembers.
const HISTORY_LIMIT: usize = 256;

/// One step of history: a run of blocks that became another run. A map can be large and a
/// step is usually one thing, so whole maps aren't kept.
#[derive(Clone, Debug, PartialEq)]
struct Change {
    /// Where the run starts.
    at: usize,
    /// What was there.
    before: Vec<Block>,
    /// What's there now.
    after: Vec<Block>,
}

impl Change {
    /// What changed from `old` to `new`, if anything, as the part that differs.
    fn between(old: &[Block], new: &[Block]) -> Option<Self> {
        let first = old
            .iter()
            .zip(new)
            .position(|(old, new)| old != new)
            .unwrap_or_else(|| old.len().min(new.len()));
        if old.len() == new.len() && first == old.len() {
            return None;
        }
        let most = old.len().min(new.len()) - first;
        let last = old
            .iter()
            .rev()
            .zip(new.iter().rev())
            .take(most)
            .take_while(|(old, new)| old == new)
            .count();
        Some(Self {
            at: first,
            before: old[first..old.len() - last].to_vec(),
            after: new[first..new.len() - last].to_vec(),
        })
    }

    /// Makes it happen to `blocks`.
    fn apply(&self, blocks: &mut Vec<Block>) {
        let end = self.at + self.before.len();
        let _gone: Vec<Block> = blocks
            .splice(self.at..end, self.after.iter().cloned())
            .collect();
    }

    /// The same step the other way.
    fn reversed(self) -> Self {
        Self {
            at: self.at,
            before: self.after,
            after: self.before,
        }
    }
}

/// A map being edited.
#[derive(Clone, Debug)]
pub struct Session {
    /// The map.
    pub(crate) document: Document,
    /// Where it's saved, if it has been.
    pub(crate) path: Option<PathBuf>,
    /// The content directory, for prefabs, materials and compiling.
    pub(crate) content: PathBuf,
    /// The map as of the last change that was noted.
    settled: Document,
    /// What undo reverses, oldest first.
    undo: Vec<Change>,
    /// What redo goes forward by.
    redo: Vec<Change>,
    /// The next ID to give out.
    next_id: u64,
    /// Whether it's changed since saving.
    pub(crate) dirty: bool,
}

/// `path` from the working folder, if it's relative.
fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_owned())
}

/// `path` with `.` and `..` worked out.
fn plain_path(path: &Path) -> PathBuf {
    let mut plain = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                let _went_up = plain.pop();
            }
            other => plain.push(other),
        }
    }
    plain
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
            settled: Document::new("map"),
            undo: Vec::new(),
            redo: Vec::new(),
            next_id: 1,
            dirty: false,
        }
    }

    /// `path` as it stands: from the content, and inside it.
    ///
    /// # Errors
    ///
    /// Returns a message if it goes outside the content.
    pub fn in_content(&self, path: &Path) -> Result<PathBuf, String> {
        let root = plain_path(&absolute(&self.content));
        let joined = if path.is_absolute() {
            path.to_owned()
        } else {
            root.join(path)
        };
        let plain = plain_path(&joined);
        if plain.starts_with(&root) {
            Ok(plain)
        } else {
            Err(format!("{} isn't in the content", path.display()))
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
        session.settled = session.document.clone();
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

    /// Notes whatever changed since the last time.
    fn settle(&mut self) {
        if let Some(change) = Change::between(&self.settled.blocks, &self.document.blocks) {
            change.apply(&mut self.settled.blocks);
            self.undo.push(change);
            if self.undo.len() > HISTORY_LIMIT {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
    }

    /// Says a change is coming, so that what came before stands as a step of its own.
    pub(crate) fn checkpoint(&mut self) {
        self.settle();
        self.dirty = true;
    }

    /// Goes back a step. Returns whether there was one.
    pub(crate) fn undo(&mut self) -> bool {
        self.settle();
        let Some(change) = self.undo.pop() else {
            return false;
        };
        let change = change.reversed();
        change.apply(&mut self.document.blocks);
        change.apply(&mut self.settled.blocks);
        self.redo.push(change.reversed());
        self.dirty = true;
        true
    }

    /// Goes forward a step. Returns whether there was one.
    pub(crate) fn redo(&mut self) -> bool {
        self.settle();
        let Some(change) = self.redo.pop() else {
            return false;
        };
        change.apply(&mut self.document.blocks);
        change.apply(&mut self.settled.blocks);
        self.undo.push(change);
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
