//! Packed containers: a folder of reusable items, written as one file by the
//! compile step (proposal 0068).

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Mutex;

use crate::archive::Archive;
use crate::error::AssetError;

/// What a pack starts with.
const MAGIC: &[u8; 4] = b"EPAK";

/// The pack version this build writes and reads.
const VERSION: u32 = 1;

/// The longest path a pack holds, so a damaged size can't ask for much.
const MAX_PATH_BYTES: usize = 1024;

/// Writes every file of `archive` to `out` as a pack, in path order.
///
/// # Errors
///
/// Returns an error if a file can't be read, or writing fails.
pub fn write_pack(archive: &dyn Archive, out: &mut dyn Write) -> Result<(), AssetError> {
    let failed = |source| AssetError::Io {
        path: "pack".to_owned(),
        source,
    };
    let mut paths = archive.paths();
    paths.sort();
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let bytes = archive.read(&path)?;
        files.push((path, bytes));
    }
    let index_size: usize = files.iter().map(|(path, _)| path.len() + 18).sum();
    let mut offset = 12 + index_size as u64;
    let mut header = Vec::with_capacity(12 + index_size);
    header.extend_from_slice(MAGIC);
    header.extend_from_slice(&VERSION.to_le_bytes());
    let count = u32::try_from(files.len()).unwrap_or(u32::MAX);
    header.extend_from_slice(&count.to_le_bytes());
    for (path, bytes) in &files {
        let length = u16::try_from(path.len()).unwrap_or(u16::MAX);
        header.extend_from_slice(&length.to_le_bytes());
        header.extend_from_slice(path.as_bytes());
        header.extend_from_slice(&offset.to_le_bytes());
        header.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        offset += bytes.len() as u64;
    }
    out.write_all(&header).map_err(failed)?;
    for (_, bytes) in &files {
        out.write_all(bytes).map_err(failed)?;
    }
    Ok(())
}

/// A pack's place in its file.
#[derive(Clone, Copy, Debug)]
struct Entry {
    /// Where the file starts.
    offset: u64,
    /// How long it is.
    size: u64,
}

/// A packed container, read from one file.
#[derive(Debug)]
pub struct PackArchive {
    /// The file's name.
    label: String,
    /// The file.
    file: Mutex<File>,
    /// What it holds, by path.
    entries: BTreeMap<String, Entry>,
}

impl PackArchive {
    /// Opens the pack at `path`.
    ///
    /// # Errors
    ///
    /// Returns an error if it can't be read, or isn't a whole pack.
    pub fn open(path: &Path) -> Result<Self, AssetError> {
        let label = path.display().to_string();
        let failed = |source| AssetError::Io {
            path: label.clone(),
            source,
        };
        let corrupt = |reason: &str| AssetError::Corrupt {
            archive: label.clone(),
            reason: reason.to_owned(),
        };
        let mut file = File::open(path).map_err(failed)?;
        let file_size = file.metadata().map_err(failed)?.len();
        let mut header = [0; 12];
        file.read_exact(&mut header)
            .map_err(|_| corrupt("too short"))?;
        if &header[..4] != MAGIC {
            return Err(corrupt("not a pack"));
        }
        let word = |at: usize| {
            u32::from_le_bytes([header[at], header[at + 1], header[at + 2], header[at + 3]])
        };
        if word(4) > VERSION {
            return Err(corrupt("made by a newer version"));
        }
        let mut entries = BTreeMap::new();
        for _ in 0..word(8) {
            let mut length = [0; 2];
            file.read_exact(&mut length)
                .map_err(|_| corrupt("the index ends early"))?;
            let length = usize::from(u16::from_le_bytes(length));
            if length > MAX_PATH_BYTES {
                return Err(corrupt("a path is too long"));
            }
            let mut name = vec![0; length];
            file.read_exact(&mut name)
                .map_err(|_| corrupt("the index ends early"))?;
            let name = String::from_utf8(name).map_err(|_| corrupt("a path isn't text"))?;
            let mut numbers = [0; 16];
            file.read_exact(&mut numbers)
                .map_err(|_| corrupt("the index ends early"))?;
            let (offset, size) = numbers.split_at(8);
            let offset = u64::from_le_bytes(offset.try_into().unwrap_or([0; 8]));
            let size = u64::from_le_bytes(size.try_into().unwrap_or([0; 8]));
            if offset.checked_add(size).is_none_or(|end| end > file_size) {
                return Err(corrupt("a file runs past the end"));
            }
            entries.insert(name, Entry { offset, size });
        }
        Ok(Self {
            label,
            file: Mutex::new(file),
            entries,
        })
    }
}

impl Archive for PackArchive {
    fn label(&self) -> &str {
        &self.label
    }

    fn paths(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }

    fn contains(&self, path: &str) -> bool {
        self.entries.contains_key(path)
    }

    fn read(&self, path: &str) -> Result<Vec<u8>, AssetError> {
        let entry = self
            .entries
            .get(path)
            .ok_or_else(|| AssetError::NotFound(path.to_owned()))?;
        let failed = |source| AssetError::Io {
            path: path.to_owned(),
            source,
        };
        let mut file = self.file.lock().map_err(|_| AssetError::Corrupt {
            archive: self.label.clone(),
            reason: "a reader panicked".to_owned(),
        })?;
        file.seek(SeekFrom::Start(entry.offset)).map_err(failed)?;
        let size = usize::try_from(entry.size).map_err(|_| AssetError::Corrupt {
            archive: self.label.clone(),
            reason: "a file is too big".to_owned(),
        })?;
        let mut bytes = vec![0; size];
        file.read_exact(&mut bytes).map_err(failed)?;
        Ok(bytes)
    }
}
