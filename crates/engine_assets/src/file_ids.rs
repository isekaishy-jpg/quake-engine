//! File ids: a number for each path, which stays when a file is renamed or changes format
//! (proposal 0131).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::error::AssetError;

/// The first id given out. 0 means no file.
const FIRST_ID: u32 = 1;

/// The table of file ids, kept as text so that it can be read and merged like code.
///
/// An id is never given to anything else, even once its file has gone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileIds {
    /// The id the next new path gets.
    next: u32,
    /// Each path's id.
    ids: BTreeMap<String, u32>,
}

impl Default for FileIds {
    fn default() -> Self {
        Self {
            next: FIRST_ID,
            ids: BTreeMap::new(),
        }
    }
}

impl FileIds {
    /// Reads a table written by [`Self::to_text`].
    ///
    /// # Errors
    ///
    /// Returns an error if a line isn't right, or two paths share an id.
    pub fn parse(text: &str) -> Result<Self, AssetError> {
        let corrupt = |reason: String| AssetError::Corrupt {
            archive: "file id table".to_owned(),
            reason,
        };
        let mut table = Self::default();
        let mut taken = BTreeMap::new();
        for line in text.lines().filter(|line| !line.is_empty()) {
            let (id, path) = line
                .split_once(';')
                .ok_or_else(|| corrupt(format!("no `;` in `{line}`")))?;
            if id == "next" {
                table.next = path
                    .parse()
                    .map_err(|_| corrupt(format!("a bad next id in `{line}`")))?;
                continue;
            }
            let id: u32 = id
                .parse()
                .map_err(|_| corrupt(format!("a bad id in `{line}`")))?;
            if let Some(other) = taken.insert(id, path) {
                return Err(corrupt(format!("{id} is both {other} and {path}")));
            }
            table.ids.insert(path.to_owned(), id);
        }
        let highest = table.ids.values().max().copied().unwrap_or(0);
        if table.next <= highest {
            return Err(corrupt(format!(
                "next is {} with {highest} used",
                table.next
            )));
        }
        Ok(table)
    }

    /// The table as text, with what's next first and then each file by id.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut by_id: Vec<(u32, &str)> = self
            .ids
            .iter()
            .map(|(path, id)| (*id, path.as_str()))
            .collect();
        by_id.sort_unstable();
        let mut text = format!("next;{}\n", self.next);
        for (id, path) in by_id {
            let _written = writeln!(text, "{id};{path}");
        }
        text
    }

    /// The id of `path`, if it has one.
    #[must_use]
    pub fn id_of(&self, path: &str) -> Option<u32> {
        self.ids.get(path).copied()
    }

    /// The id of `path`, giving it the next free one if it has none.
    pub fn assign(&mut self, path: &str) -> u32 {
        if let Some(id) = self.id_of(path) {
            return id;
        }
        let id = self.next;
        self.next += 1;
        self.ids.insert(path.to_owned(), id);
        id
    }
}
