//! Pipelines kept between runs, so a driver needn't build them again.
//!
//! Drivers check the header of what they're given, but not always well, and a
//! file cut short by a crash reaches them too. So the file starts with what
//! this run can check for itself (proposal 0107).

use std::path::Path;

use vulkane::safe::{Device, PhysicalDevice, PipelineCache};

/// What the file starts with.
const MAGIC: &[u8; 4] = b"EPIP";

/// A hash of what has to be the same for the rest to be any use.
fn identity(physical: &PhysicalDevice) -> u64 {
    let properties = physical.properties();
    let mut text = properties.device_name();
    text.push_str(env!("CARGO_PKG_VERSION"));
    let numbers = [
        properties.vendor_id(),
        properties.device_id(),
        properties.driver_version(),
    ];
    let bytes = text
        .bytes()
        .chain(numbers.iter().flat_map(|n| n.to_le_bytes()));
    hash(bytes)
}

/// A hash of `bytes`.
fn hash(bytes: impl Iterator<Item = u8>) -> u64 {
    bytes.fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// What in `file` is for this device, if it's whole.
fn checked(file: &[u8], physical: &PhysicalDevice) -> Option<Vec<u8>> {
    let (magic, rest) = file.split_at_checked(4)?;
    let (who, rest) = rest.split_at_checked(8)?;
    let (sum, data) = rest.split_at_checked(8)?;
    let number = |bytes: &[u8]| Some(u64::from_le_bytes(bytes.try_into().ok()?));
    let right = magic == MAGIC
        && number(who)? == identity(physical)
        && number(sum)? == hash(data.iter().copied());
    right.then(|| data.to_vec())
}

/// Starts this run's pipeline cache from the file at `path`, if there's a good
/// one, and has the device use it.
pub(super) fn open(
    device: &Device,
    physical: &PhysicalDevice,
    path: Option<&Path>,
) -> PipelineCacheHolder {
    let kept = path
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|file| checked(&file, physical));
    let cache = PipelineCache::with_data(device, kept.as_deref().unwrap_or(&[]))
        .or_else(|_| PipelineCache::new(device))
        .ok();
    if let Some(cache) = &cache {
        device.use_pipeline_cache(cache);
    }
    PipelineCacheHolder {
        cache,
        path: path.map(Path::to_path_buf),
    }
}

/// The run's pipeline cache, and where it's kept.
pub(super) struct PipelineCacheHolder {
    /// The cache, which is `None` if the device wouldn't make one.
    cache: Option<PipelineCache>,
    /// Where to keep it.
    path: Option<std::path::PathBuf>,
}

impl PipelineCacheHolder {
    /// Writes the cache to its file. Failing costs nothing but time next run,
    /// so it's not an error.
    pub(super) fn save(&self, physical: &PhysicalDevice) {
        let (Some(cache), Some(path)) = (&self.cache, &self.path) else {
            return;
        };
        let Ok(data) = cache.data() else {
            return;
        };
        let mut file = Vec::with_capacity(data.len() + 20);
        file.extend_from_slice(MAGIC);
        file.extend_from_slice(&identity(physical).to_le_bytes());
        file.extend_from_slice(&hash(data.iter().copied()).to_le_bytes());
        file.extend_from_slice(&data);
        let spare = path.with_extension("new");
        if std::fs::write(&spare, file).is_ok() {
            let _kept = std::fs::rename(&spare, path);
        }
    }
}
