//! Sounds by name, loaded the first time each one plays.

use std::collections::HashMap;

use engine_assets::FileSystem;
use engine_audio::{Audio, ClipId};
use fixture_quake::parse_wav;

/// Every sound asked for so far.
#[derive(Default)]
pub(crate) struct SoundBank {
    /// Each name's clip, or `None` if it wouldn't load.
    clips: HashMap<String, Option<ClipId>>,
}

impl SoundBank {
    /// The clip called `name`, loading it if this is the first time.
    pub(crate) fn clip(
        &mut self,
        name: &str,
        filesystem: &FileSystem,
        audio: &mut Audio,
    ) -> Option<ClipId> {
        if let Some(clip) = self.clips.get(name) {
            return *clip;
        }
        let clip = filesystem
            .read(&format!("sound/{name}"))
            .ok()
            .and_then(|bytes| parse_wav(&bytes))
            .map(|clip| audio.add_clip(&clip));
        self.clips.insert(name.to_owned(), clip);
        clip
    }

    /// How many sounds have been asked for.
    pub(crate) fn len(&self) -> usize {
        self.clips.len()
    }

    /// How many of those wouldn't load.
    pub(crate) fn missing(&self) -> usize {
        self.clips.values().filter(|clip| clip.is_none()).count()
    }
}
