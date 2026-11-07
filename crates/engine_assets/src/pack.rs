//! Containers: a folder of reusable items, written as one `.pkt` file by the compile step
//! (proposals 0068 and 0131). A short header says how long the index is, the index is a
//! FlatBuffer of [`pkt_generated`](crate::pkt_generated), and the files follow it.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, mpsc};

use planus::ReadAsRoot;
use sha2::{Digest, Sha256};

use crate::archive::Archive;
use crate::error::AssetError;
use crate::file_ids::FileIds;
use crate::pkt_generated::pkt::{Entry as IndexEntry, Manifest, ManifestRef};

/// What a container starts with.
const MAGIC: &[u8; 4] = b"PKT\0";

/// The container format this build writes and reads.
const FORMAT: u32 = 1;

/// The bytes before the index: the magic, the format and the index's length.
const HEADER_BYTES: usize = 16;

/// The longest index a container can have, so a damaged size can't ask for much.
const MAX_INDEX_BYTES: u64 = 1 << 30;

/// How hard files are squeezed: about the best for its time.
const SQUEEZE_LEVEL: u8 = 6;

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

/// Writes every file of `archive` to `out` as a container, with their ids from `ids`.
///
/// # Errors
///
/// Returns an error if a file can't be read, or writing fails.
pub fn write_pack(
    archive: &dyn Archive,
    ids: &mut FileIds,
    out: &mut dyn Write,
) -> Result<PackSummary, AssetError> {
    write_pack_of(archive, archive.paths(), ids, out)
}

/// The SHA-256 of `parts`, in hexadecimal.
fn hash_of<'a>(parts: impl Iterator<Item = &'a [u8]>) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    let mut text = String::with_capacity(64);
    for byte in hasher.finalize() {
        let _written = write!(text, "{byte:02x}");
    }
    text
}

/// Writes the files of `archive` at `paths` to `out` as a container, in id order. New paths
/// get ids in `ids`. Files that are the same are held once, and each is squeezed if that
/// helps.
///
/// # Errors
///
/// Returns an error if a file can't be read, or writing fails.
pub fn write_pack_of(
    archive: &dyn Archive,
    mut paths: Vec<String>,
    ids: &mut FileIds,
    out: &mut dyn Write,
) -> Result<PackSummary, AssetError> {
    let failed = |source| AssetError::Io {
        path: "container".to_owned(),
        source,
    };
    paths.sort();
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let bytes = archive.read(&path)?;
        files.push((ids.assign(&path), path, bytes));
    }
    files.sort_by_key(|(id, _, _)| *id);
    let mut first = HashMap::new();
    let mut unique: Vec<&[u8]> = Vec::new();
    let mut held = Vec::with_capacity(files.len());
    for (_, _, bytes) in &files {
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
    let mut offsets = Vec::with_capacity(stored.len());
    let mut offset = 0;
    for bytes in &stored {
        offsets.push(offset);
        offset += bytes.len() as u64;
    }
    let entries = files
        .iter()
        .zip(&held)
        .map(|((id, path, bytes), number)| IndexEntry {
            fid: *id,
            path: path.clone(),
            offset: offsets[*number],
            stored: stored[*number].len() as u64,
            size: bytes.len() as u64,
        })
        .collect();
    let manifest = Manifest {
        format: FORMAT,
        sha256: hash_of(stored.iter().copied()),
        entries,
    };
    let mut builder = planus::Builder::new();
    let index = builder.finish(&manifest, None);
    out.write_all(MAGIC).map_err(failed)?;
    out.write_all(&FORMAT.to_le_bytes()).map_err(failed)?;
    out.write_all(&(index.len() as u64).to_le_bytes())
        .map_err(failed)?;
    out.write_all(index).map_err(failed)?;
    for bytes in &stored {
        out.write_all(bytes).map_err(failed)?;
    }
    Ok(PackSummary {
        files: files.len(),
        shared: files.len() - unique.len(),
        bytes: files.iter().map(|(_, _, bytes)| bytes.len() as u64).sum(),
        stored: stored.iter().map(|bytes| bytes.len() as u64).sum(),
    })
}

/// A file's place in its container.
#[derive(Clone, Copy, Debug)]
struct Entry {
    /// Where the file starts.
    offset: u64,
    /// How long it is in the pack.
    stored: u64,
    /// How long it is. If that's more than `stored`, it's squeezed.
    size: u64,
}

/// A container, read from one file.
#[derive(Debug)]
pub struct PackArchive {
    /// The file's name.
    label: String,
    /// The file.
    file: Mutex<File>,
    /// What it holds, by path.
    entries: BTreeMap<String, Entry>,
    /// Each file's path, by id.
    paths: BTreeMap<u32, String>,
    /// Where the files start.
    body: u64,
    /// The SHA-256 the index says they have.
    sha256: String,
}

impl PackArchive {
    /// Opens the container at `path`.
    ///
    /// # Errors
    ///
    /// Returns an error if it can't be read, or isn't a whole container.
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
        let mut header = [0; HEADER_BYTES];
        file.read_exact(&mut header)
            .map_err(|_| corrupt("too short"))?;
        if &header[..4] != MAGIC {
            return Err(corrupt("not a container"));
        }
        if header[4..8] != FORMAT.to_le_bytes() {
            return Err(corrupt("made by another version"));
        }
        let length = u64::from_le_bytes(header[8..].try_into().unwrap_or([0xFF; 8]));
        let body = (HEADER_BYTES as u64).saturating_add(length);
        if length > MAX_INDEX_BYTES || body > file_size {
            return Err(corrupt("the index runs past the end"));
        }
        let mut index = vec![0; usize::try_from(length).map_err(|_| corrupt("too big"))?];
        file.read_exact(&mut index)
            .map_err(|_| corrupt("the index ends early"))?;
        let bad_index = |_| corrupt("the index doesn't read");
        let manifest = ManifestRef::read_as_root(&index).map_err(bad_index)?;
        let mut entries = BTreeMap::new();
        let mut paths = BTreeMap::new();
        for entry in manifest.entries().map_err(bad_index)? {
            let entry = entry.map_err(bad_index)?;
            let (offset, stored, size) = (
                entry.offset().map_err(bad_index)?,
                entry.stored().map_err(bad_index)?,
                entry.size().map_err(bad_index)?,
            );
            let end = offset
                .checked_add(stored)
                .and_then(|end| end.checked_add(body));
            if end.is_none_or(|end| end > file_size) {
                return Err(corrupt("a file runs past the end"));
            }
            if stored > size {
                return Err(corrupt("a file is bigger stored than it is"));
            }
            let path = entry.path().map_err(bad_index)?.to_owned();
            paths.insert(entry.fid().map_err(bad_index)?, path.clone());
            let held = Entry {
                offset,
                stored,
                size,
            };
            entries.insert(path, held);
        }
        let sha256 = manifest.sha256().map_err(bad_index)?.to_owned();
        Ok(Self {
            label,
            file: Mutex::new(file),
            entries,
            paths,
            body,
            sha256,
        })
    }

    /// Reads every file's bytes and checks them against the index's hash, which a load
    /// leaves out for the time it takes.
    ///
    /// # Errors
    ///
    /// Returns an error if it can't be read, or it doesn't match.
    pub fn verify(&self) -> Result<(), AssetError> {
        let corrupt = |reason: &str| AssetError::Corrupt {
            archive: self.label.clone(),
            reason: reason.to_owned(),
        };
        let failed = |source| AssetError::Io {
            path: self.label.clone(),
            source,
        };
        let mut body = Vec::new();
        {
            let mut file = self.file.lock().map_err(|_| corrupt("a reader panicked"))?;
            file.seek(SeekFrom::Start(self.body)).map_err(failed)?;
            file.read_to_end(&mut body).map_err(failed)?;
        }
        if hash_of(std::iter::once(body.as_slice())) == self.sha256 {
            Ok(())
        } else {
            Err(corrupt("its hash doesn't match"))
        }
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

    fn path_of_id(&self, id: u32) -> Option<String> {
        self.paths.get(&id).cloned()
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
            file.seek(SeekFrom::Start(self.body + entry.offset))
                .map_err(failed)?;
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
