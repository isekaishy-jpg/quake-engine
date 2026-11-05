//! What's heard.

use fixture_quake::music_file;

use super::World;

/// The sound levels the console sets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct AudioLevels {
    /// `volume`.
    pub(super) volume: f32,
    /// `bgmvolume`.
    pub(super) music: f32,
    /// `ambient_level`.
    pub(super) ambient: f32,
    /// `ambient_water`.
    pub(super) water: f32,
}

impl World {
    /// Passes on the sounds and music the client has heard about.
    pub(super) fn play_sounds(&mut self) {
        let Some(client) = &mut self.client else {
            return;
        };
        for (number, name) in client.take_model_names() {
            self.models.name_number(
                u32::from(number),
                &name,
                &self.filesystem,
                self.renderer.as_mut(),
            );
        }
        for effect in client.take_effects() {
            self.effects.start(&effect);
        }
        let mut sounds = client.take_sounds();
        sounds.extend(self.effects.take_sounds());
        let static_sounds = client.take_static_sounds();
        let music = client.take_music();
        for sound in sounds {
            let clip = self
                .sounds
                .clip(&sound.name, &self.filesystem, &mut self.audio);
            if let Some(clip) = clip {
                self.audio.start(&sound, clip);
            }
        }
        for sound in static_sounds {
            let clip = self
                .sounds
                .clip(&sound.name, &self.filesystem, &mut self.audio);
            if let Some(clip) = clip {
                self.audio.add_static(&sound, clip);
            }
        }
        if let Some(track) = music {
            self.play_track(track);
        }
    }

    /// Plays music track `track`, unless it's already playing. With no file
    /// for it there's no music, as with no disc in the drive.
    pub(crate) fn play_track(&mut self, track: u8) {
        if self.music_track == Some(track) {
            return;
        }
        self.music_track = Some(track);
        let bytes = self.filesystem.read(&music_file(track));
        let playing = bytes.is_ok_and(|bytes| self.audio.play_music(bytes).is_ok());
        if !playing {
            self.audio.stop_music();
        }
    }

    /// Moves sound on by `seconds`, at these volumes.
    pub(super) fn update_audio(&mut self, seconds: f64, levels: AudioLevels, muted: bool) {
        if self.audio_settings != Some((levels, muted)) {
            self.audio_settings = Some((levels, muted));
            self.audio.set_volume(levels.volume);
            self.audio.set_music_volume(levels.music);
            self.audio.set_ambient_level(levels.ambient);
            self.audio.set_water_share(levels.water);
            self.audio.set_muted(muted);
        }
        let origin = self.listener.origin;
        let levels = self
            .ambience
            .as_mut()
            .map_or([0, 0], |ambience| ambience.levels(origin));
        self.audio.update(self.listener, seconds, levels);
    }
}
