//! Quake's pak archive, as read by `COM_LoadPackFile` in `common.c`.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use engine_assets::{Archive, AssetError};

/// The size of the file header.
const HEADER_SIZE: usize = 12;
/// The size of one directory entry.
const ENTRY_SIZE: usize = 64;
/// The size of an entry's name field.
const NAME_SIZE: usize = 56;

/// One file's place in the pak.
#[derive(Clone, Copy, Debug)]
struct Entry {
    /// Its offset from the start of the pak.
    offset: u64,
    /// Its length in bytes.
    length: usize,
}

/// A pak file. The directory is read at open, and contents on demand.
#[derive(Clone, Debug)]
pub struct PakArchive {
    /// The pak on disk.
    path: PathBuf,
    /// The path as text, for display.
    label: String,
    /// Files by lower-case name.
    entries: BTreeMap<String, Entry>,
}

/// Reads a little-endian `u32` at `offset`.
fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let field = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes(field.try_into().ok()?))
}

/// Reads a whole file's directory from `file`.
fn read_directory(file: &mut File, label: &str) -> Result<BTreeMap<String, Entry>, AssetError> {
    let corrupt = |reason: &str| AssetError::Corrupt {
        archive: label.to_owned(),
        reason: reason.to_owned(),
    };
    let io = |source| AssetError::Io {
        path: label.to_owned(),
        source,
    };
    let size = file.metadata().map_err(io)?.len();
    let mut header = [0; HEADER_SIZE];
    file.read_exact(&mut header)
        .map_err(|_short| corrupt("too short"))?;
    if &header[..4] != b"PACK" {
        return Err(corrupt("no PACK header"));
    }
    let offset = read_u32(&header, 4).ok_or_else(|| corrupt("bad header"))?;
    let length = read_u32(&header, 8).ok_or_else(|| corrupt("bad header"))?;
    let length_bytes = usize::try_from(length).map_err(|_large| corrupt("bad directory"))?;
    if length_bytes % ENTRY_SIZE != 0 || u64::from(offset) + u64::from(length) > size {
        return Err(corrupt("bad directory"));
    }
    file.seek(SeekFrom::Start(u64::from(offset))).map_err(io)?;
    let mut directory = vec![0; length_bytes];
    file.read_exact(&mut directory).map_err(io)?;
    let mut entries = BTreeMap::new();
    let (records, _remainder) = directory.as_chunks::<ENTRY_SIZE>();
    for record in records {
        let name = &record[..NAME_SIZE];
        let end = name.iter().position(|&byte| byte == 0).unwrap_or(NAME_SIZE);
        let name = String::from_utf8_lossy(&name[..end]).to_lowercase();
        let start = read_u32(record, NAME_SIZE).ok_or_else(|| corrupt("bad entry"))?;
        let bytes = read_u32(record, NAME_SIZE + 4).ok_or_else(|| corrupt("bad entry"))?;
        if u64::from(start) + u64::from(bytes) > size {
            return Err(corrupt("entry past end of file"));
        }
        let length = usize::try_from(bytes).map_err(|_large| corrupt("bad entry"))?;
        let _previous = entries.insert(
            name,
            Entry {
                offset: u64::from(start),
                length,
            },
        );
    }
    Ok(entries)
}

impl PakArchive {
    /// Opens the pak at `path` and reads its directory.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::Io`] if it can't be read, and
    /// [`AssetError::Corrupt`] if it isn't a valid pak.
    pub fn open(path: &Path) -> Result<Self, AssetError> {
        let label = path.display().to_string();
        let mut file = File::open(path).map_err(|source| AssetError::Io {
            path: label.clone(),
            source,
        })?;
        let entries = read_directory(&mut file, &label)?;
        Ok(Self {
            path: path.to_path_buf(),
            label,
            entries,
        })
    }
}

impl Archive for PakArchive {
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
        let io = |source| AssetError::Io {
            path: format!("{}#{path}", self.label),
            source,
        };
        let mut file = File::open(&self.path).map_err(io)?;
        file.seek(SeekFrom::Start(entry.offset)).map_err(io)?;
        let mut bytes = vec![0; entry.length];
        file.read_exact(&mut bytes).map_err(io)?;
        Ok(bytes)
    }
}
