//! The backend: the only place that knows `kira`.

use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;

use engine_core::sound::Clip;
use kira::sound::FromFileError;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings};
use kira::sound::streaming::{StreamingSoundData, StreamingSoundHandle};
use kira::track::{TrackBuilder, TrackHandle};
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Frame, Panning, Tween};

use crate::error::AudioError;
use crate::spatial::amplitude_and_pan;

/// The most sound effects that can play at once.
const MAX_VOICES: usize = 1024;

/// How long a volume takes to change, so changes don't click.
const SMOOTHING: Duration = Duration::from_millis(15);

/// How long a sound takes to fade when stopped or muted.
const FADE: Duration = Duration::from_millis(30);

/// A change over `duration`.
fn tween(duration: Duration) -> Tween {
    Tween {
        duration,
        ..Tween::default()
    }
}

/// An amplitude in decibels.
fn decibels(amplitude: f32) -> Decibels {
    if amplitude <= 0.0 {
        Decibels::SILENCE
    } else {
        Decibels(20.0 * amplitude.log10())
    }
}

/// A clip, ready to play.
#[derive(Clone)]
pub(crate) struct Prepared(StaticSoundData);

impl Prepared {
    /// Prepares `clip`.
    pub(crate) fn new(clip: &Clip) -> Self {
        let frames: Arc<[Frame]> = clip.samples.iter().copied().map(Frame::from_mono).collect();
        let data = StaticSoundData {
            sample_rate: clip.sample_rate,
            frames,
            settings: StaticSoundSettings::default(),
            slice: None,
        };
        Self(match clip.loop_start {
            Some(start) => {
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "sounds are far shorter than f64 can count"
                )]
                let seconds = start as f64 / f64::from(clip.sample_rate.max(1));
                data.loop_region(seconds..)
            }
            None => data,
        })
    }
}

/// A sound playing.
pub(crate) struct Voice(StaticSoundHandle);

impl Voice {
    /// Sets the volume in each ear.
    pub(crate) fn set_volumes(&mut self, left: f32, right: f32) {
        let (amplitude, pan) = amplitude_and_pan(left, right);
        self.0.set_volume(decibels(amplitude), tween(SMOOTHING));
        self.0.set_panning(Panning(pan), tween(SMOOTHING));
    }

    /// Stops it, with a short fade.
    pub(crate) fn stop(&mut self) {
        self.0.stop(tween(FADE));
    }
}

/// The sound device.
pub(crate) struct Output {
    /// Mixes everything.
    manager: AudioManager<DefaultBackend>,
    /// Sound effects.
    effects: TrackHandle,
    /// The music playing.
    music: Option<StreamingSoundHandle<FromFileError>>,
}

impl Output {
    /// Opens the default device, if there is one.
    pub(crate) fn open() -> Option<Self> {
        let mut manager =
            AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()).ok()?;
        let effects = manager
            .add_sub_track(TrackBuilder::new().sound_capacity(MAX_VOICES))
            .ok()?;
        Some(Self {
            manager,
            effects,
            music: None,
        })
    }

    /// Starts `clip` `offset` seconds in, with these volumes.
    pub(crate) fn play(
        &mut self,
        clip: &Prepared,
        offset: f64,
        left: f32,
        right: f32,
    ) -> Option<Voice> {
        let (amplitude, pan) = amplitude_and_pan(left, right);
        let data = clip
            .0
            .start_position(offset)
            .volume(decibels(amplitude))
            .panning(Panning(pan));
        self.effects.play(data).ok().map(Voice)
    }

    /// Sets how loud sound effects are.
    pub(crate) fn set_effects_volume(&mut self, volume: f32) {
        self.effects.set_volume(decibels(volume), tween(SMOOTHING));
    }

    /// Sets how loud music is.
    pub(crate) fn set_music_volume(&mut self, volume: f32) {
        if let Some(music) = &mut self.music {
            music.set_volume(decibels(volume), tween(SMOOTHING));
        }
    }

    /// Silences everything, or lets it back.
    pub(crate) fn set_muted(&mut self, muted: bool) {
        let volume = if muted {
            Decibels::SILENCE
        } else {
            Decibels::IDENTITY
        };
        self.manager.main_track().set_volume(volume, tween(FADE));
    }

    /// Loops the music in `bytes`, in place of any already playing.
    pub(crate) fn play_music(&mut self, bytes: Vec<u8>, volume: f32) -> Result<(), AudioError> {
        self.stop_music();
        let data = StreamingSoundData::from_cursor(Cursor::new(bytes))
            .map_err(|error| AudioError(error.to_string()))?
            .loop_region(0.0..)
            .volume(decibels(volume));
        let music = self
            .manager
            .play(data)
            .map_err(|error| AudioError(error.to_string()))?;
        self.music = Some(music);
        Ok(())
    }

    /// Stops the music.
    pub(crate) fn stop_music(&mut self) {
        if let Some(mut music) = self.music.take() {
            music.stop(tween(FADE));
        }
    }
}
