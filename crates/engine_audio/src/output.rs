//! The backend: the only place that knows `kira`.

use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use engine_core::sound::Clip;
use kira::effect::{Effect, EffectBuilder};
use kira::info::Info;
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

/// How long a sound takes to fade when stopped.
const FADE: Duration = Duration::from_millis(30);

/// How long everything takes to fade when muted. Fades run in decibels, so a
/// short one drops most of the way at once, which crackles.
const MUTE_FADE: Duration = Duration::from_millis(250);

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

/// Times the gaps between the audio thread's turns, on that thread. A gap
/// longer than the device's buffer is a dropout, and the thread's own load
/// can't show one, since it's only counted once the thread gets to run.
struct GapWatch {
    /// When the last turn started.
    last: Option<Instant>,
    /// The longest gap since last asked, in microseconds.
    longest: Arc<AtomicU32>,
}

impl Effect for GapWatch {
    fn on_start_processing(&mut self) {
        let now = Instant::now();
        if let Some(last) = self.last {
            let gap = now.duration_since(last).as_micros();
            let gap = u32::try_from(gap).unwrap_or(u32::MAX);
            self.longest.fetch_max(gap, Ordering::Relaxed);
        }
        self.last = Some(now);
    }

    fn process(&mut self, _input: &mut [Frame], _dt: f64, _info: &Info<'_>) {}
}

/// Adds a [`GapWatch`] to a track.
struct GapWatchBuilder(Arc<AtomicU32>);

impl EffectBuilder for GapWatchBuilder {
    type Handle = ();

    fn build(self) -> (Box<dyn Effect>, Self::Handle) {
        let watch = GapWatch {
            last: None,
            longest: self.0,
        };
        (Box::new(watch), ())
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
    /// The longest gap between the audio thread's turns, in microseconds.
    longest_gap: Arc<AtomicU32>,
}

impl Output {
    /// Opens the default device, if there is one.
    pub(crate) fn open() -> Option<Self> {
        let mut manager =
            AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()).ok()?;
        let longest_gap = Arc::new(AtomicU32::new(0));
        let track = TrackBuilder::new()
            .sound_capacity(MAX_VOICES)
            .with_effect(GapWatchBuilder(Arc::clone(&longest_gap)));
        let effects = manager.add_sub_track(track).ok()?;
        Some(Self {
            manager,
            effects,
            music: None,
            longest_gap,
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
        self.manager
            .main_track()
            .set_volume(volume, tween(MUTE_FADE));
    }

    /// The longest gap between the audio thread's turns since last asked.
    pub(crate) fn longest_gap(&self) -> Duration {
        Duration::from_micros(u64::from(self.longest_gap.swap(0, Ordering::Relaxed)))
    }

    /// The most of the audio thread's time used since last asked, from 0 to 1.
    pub(crate) fn peak_load(&mut self) -> Option<f32> {
        let backend = self.manager.backend_mut();
        std::iter::from_fn(|| backend.pop_cpu_usage()).reduce(f32::max)
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
