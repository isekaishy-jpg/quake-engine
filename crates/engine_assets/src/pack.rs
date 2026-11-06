//! Packed containers: a folder of reusable items, written as one file by the
//! compile step (proposal 0068).

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, mpsc};

use crate::archive::Archive;
use crate::error::AssetError;

/// What a pack starts with.
const MAGIC: &[u8; 4] = b"EPAK";

/// The pack version this build writes and reads. Version 2 added squeezing.
const VERSION: u32 = 2;

/// The longest path a pack holds, so a damaged size can't ask for much.
const MAX_PATH_BYTES: usize = 1024;

/// How hard files are squeezed: about the best for its time.
const SQUEEZE_LEVEL: u8 = 6;

/// What a pack's index holds for each file, after its path and its length.
const ENTRY_NUMBERS: usize = 24;

/// What writing a pack came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackSummary {
    /// How many paths it holds.
    pub files: usize,
    /// How many of those use another's bytes, being the same.
    pub shared: usize,
    /// What the files come to.
    pub bytes: u64,
    /// What they take up in the pack.
    pub stored: u64,
}

/// `bytes` as a pack stores them: squeezed, unless that gains nothing.
fn squeezed(bytes: &[u8]) -> Option<Vec<u8>> {
    let small = miniz_oxide::deflate::compress_to_vec(bytes, SQUEEZE_LEVEL);
    (small.len() < bytes.len()).then_some(small)
}

/// Each of `files` as a pack stores it, worked out side by side.
fn all_squeezed(files: &[&[u8]]) -> Vec<Option<Vec<u8>>> {
    let workers = std::thread::available_parallelism().map_or(1, usize::from);
    let next = AtomicUsize::new(0);
    let (sender, receiver) = mpsc::channel();
    std::thread::scope(|scope| {
        for _ in 0..workers {
            let sender = sender.clone();
            let next = &next;
            scope.spawn(move || {
                loop {
                    let at = next.fetch_add(1, Ordering::Relaxed);
                    let Some(bytes) = files.get(at) else {
                        break;
                    };
                    if sender.send((at, squeezed(bytes))).is_err() {
                        break;
                    }
                }
            });
        }
    });
    drop(sender);
    let mut out = vec![None; files.len()];
    for (at, small) in receiver {
        out[at] = small;
    }
    out
}

/// Writes every file of `archive` to `out` as a pack, in path order.
///
/// # Errors
///
/// Returns an error if a file can't be read, or writing fails.
pub fn write_pack(archive: &dyn Archive, out: &mut dyn Write) -> Result<PackSummary, AssetError> {
    write_pack_of(archive, archive.paths(), out)
}

/// Writes the files of `archive` at `paths` to `out` as a pack, in path order.
/// Files that are the same are held once, and each is squeezed if that helps.
///
/// # Errors
///
/// Returns an error if a file can't be read, or writing fails.
pub fn write_pack_of(
    archive: &dyn Archive,
    mut paths: Vec<String>,
    out: &mut dyn Write,
) -> Result<PackSummary, AssetError> {
    let failed = |source| AssetError::Io {
        path: "pack".to_owned(),
        source,
    };
    paths.sort();
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let bytes = archive.read(&path)?;
        files.push((path, bytes));
    }
    let mut first = HashMap::new();
    let mut unique: Vec<&[u8]> = Vec::new();
    let mut held = Vec::with_capacity(files.len());
    for (_, bytes) in &files {
        let number = *first.entry(bytes.as_slice()).or_insert_with(|| {
            unique.push(bytes);
            unique.len() - 1
        });
        held.push(number);
    }
    let small = all_squeezed(&unique);
    let stored: Vec<&[u8]> = unique
        .iter()
        .zip(&small)
        .map(|(bytes, small)| small.as_deref().unwrap_or(bytes))
        .collect();
    let index_size: usize = files
        .iter()
        .map(|(path, _)| path.len() + 2 + ENTRY_NUMBERS)
        .sum();
    let mut offsets = Vec::with_capacity(stored.len());
    let mut offset = 12 + index_size as u64;
    for bytes in &stored {
        offsets.push(offset);
        offset += bytes.len() as u64;
    }
    let mut header = Vec::with_capacity(12 + index_size);
    header.extend_from_slice(MAGIC);
    header.extend_from_slice(&VERSION.to_le_bytes());
    let count = u32::try_from(files.len()).unwrap_or(u32::MAX);
    header.extend_from_slice(&count.to_le_bytes());
    for ((path, bytes), number) in files.iter().zip(&held) {
        let length = u16::try_from(path.len()).unwrap_or(u16::MAX);
        header.extend_from_slice(&length.to_le_bytes());
        header.extend_from_slice(path.as_bytes());
        header.extend_from_slice(&offsets[*number].to_le_bytes());
        header.extend_from_slice(&(stored[*number].len() as u64).to_le_bytes());
        header.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    }
    out.write_all(&header).map_err(failed)?;
    for bytes in &stored {
        out.write_all(bytes).map_err(failed)?;
    }
    Ok(PackSummary {
        files: files.len(),
        shared: files.len() - unique.len(),
        bytes: files.iter().map(|(_, bytes)| bytes.len() as u64).sum(),
        stored: stored.iter().map(|bytes| bytes.len() as u64).sum(),
    })
}

/// A pack's place in its file.
#[derive(Clone, Copy, Debug)]
struct Entry {
    /// Where the file starts.
    offset: u64,
    /// How long it is in the pack.
    stored: u64,
    /// How long it is. If that's more than `stored`, it's squeezed.
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
        if word(4) != VERSION {
            return Err(corrupt("made by another version"));
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
            let mut numbers = [0; ENTRY_NUMBERS];
            file.read_exact(&mut numbers)
                .map_err(|_| corrupt("the index ends early"))?;
            let number = |at: usize| {
                let bytes = numbers[at..at + 8].try_into().unwrap_or([0; 8]);
                u64::from_le_bytes(bytes)
            };
            let (offset, stored, size) = (number(0), number(8), number(16));
            if offset.checked_add(stored).is_none_or(|end| end > file_size) {
                return Err(corrupt("a file runs past the end"));
            }
            if stored > size {
                return Err(corrupt("a file is bigger stored than it is"));
            }
            let entry = Entry {
                offset,
                stored,
                size,
            };
            entries.insert(name, entry);
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
        let corrupt = |reason: &str| AssetError::Corrupt {
            archive: self.label.clone(),
            reason: format!("{path}: {reason}"),
        };
        let stored = usize::try_from(entry.stored).map_err(|_| corrupt("too big"))?;
        let size = usize::try_from(entry.size).map_err(|_| corrupt("too big"))?;
        let mut bytes = vec![0; stored];
        {
            let mut file = self.file.lock().map_err(|_| corrupt("a reader panicked"))?;
            file.seek(SeekFrom::Start(entry.offset)).map_err(failed)?;
            file.read_exact(&mut bytes).map_err(failed)?;
        }
        if stored == size {
            return Ok(bytes);
        }
        let opened = miniz_oxide::inflate::decompress_to_vec_with_limit(&bytes, size);
        opened
            .ok()
            .filter(|opened| opened.len() == size)
            .ok_or_else(|| corrupt("doesn't open"))
    }
}
