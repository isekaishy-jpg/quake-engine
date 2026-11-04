//! Channels: who plays, who gets replaced, and how loud each one is.

use std::time::Duration;

use engine_core::math::Vec3;
use engine_core::sound::{Clip, SoundStart, StaticSound};

use crate::error::AudioError;
use crate::output::{Output, Prepared, Voice};
use crate::spatial::{Listener, stereo_volumes};

/// How many sounds can start from entities at once. The original had 8.
const DYNAMIC_CHANNELS: usize = 128;

/// `ambient_fade`: how fast ambient sounds change, out of 255, each second.
const AMBIENT_FADE: f32 = 100.0;

/// Ambient levels under this, out of 255, are silent.
const AMBIENT_FLOOR: f32 = 8.0;

/// The most of the same sound's start that a second copy skips, in seconds.
const MAX_SKIP: f64 = 0.1;

/// The most one ear gets from one channel, from `SND_PaintChannelFrom8`.
const FULL_VOLUME: f32 = 1.0;

/// A clip the mixer holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClipId(usize);

/// A clip's details.
struct Loaded {
    /// Ready for the backend.
    prepared: Prepared,
    /// Its length in seconds.
    seconds: f64,
    /// Whether it loops.
    looping: bool,
}

/// One sound from an entity.
struct Channel {
    /// What's playing.
    clip: ClipId,
    /// Where it comes from.
    origin: Vec3,
    /// From 0 to 1.
    volume: f32,
    /// How fast it fades.
    attenuation: f32,
    /// The entity it comes from.
    entity: u64,
    /// The entity's channel.
    channel: u8,
    /// Whether it's the listener's own.
    own: bool,
    /// Seconds until it ends, which never comes for a loop.
    remaining: f64,
    /// Whether it started since the last update.
    fresh: bool,
    /// The volume in each ear.
    ears: (f32, f32),
    /// The backend's sound.
    voice: Option<Voice>,
}

impl Channel {
    /// Works out the volume in each ear.
    fn spatialize(&mut self, listener: &Listener) {
        let (left, right) = if self.own {
            (self.volume, self.volume)
        } else {
            stereo_volumes(self.origin, listener, self.volume, self.attenuation)
        };
        self.ears = (left.min(FULL_VOLUME), right.min(FULL_VOLUME));
    }
}

/// A looping sound at a fixed place.
struct Static {
    /// What's playing.
    clip: ClipId,
    /// Where it comes from.
    origin: Vec3,
    /// From 0 to 1.
    volume: f32,
    /// How fast it fades.
    attenuation: f32,
}

/// All the sounds at fixed places that play one clip. `S_Update` adds them
/// into one channel, so five torches are one louder torch, up to full volume.
struct StaticGroup {
    /// What's playing.
    clip: ClipId,
    /// The backend's sound, only while it can be heard.
    voice: Option<Voice>,
    /// The volume in each ear.
    ears: (f32, f32),
}

/// A sound the whole of a leaf hears.
#[derive(Default)]
struct Ambient {
    /// What it plays.
    clip: Option<ClipId>,
    /// The volume now, out of 255.
    level: f32,
    /// The backend's sound.
    voice: Option<Voice>,
}

/// A sound playing, as tests and tools see it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayingSound {
    /// What's playing.
    pub clip: ClipId,
    /// The entity it comes from.
    pub entity: u64,
    /// The entity's channel.
    pub channel: u8,
    /// The volume in the left ear.
    pub left: f32,
    /// The volume in the right ear.
    pub right: f32,
}

/// Everything that sounds. With no device it keeps track all the same.
pub struct Audio {
    /// The sound device, if there is one.
    output: Option<Output>,
    /// Every clip.
    clips: Vec<Loaded>,
    /// Sounds from entities.
    channels: Vec<Option<Channel>>,
    /// Sounds at fixed places.
    statics: Vec<Static>,
    /// What they add up to.
    static_groups: Vec<StaticGroup>,
    /// Sounds from leaves.
    ambients: [Ambient; 2],
    /// Where sounds are heard from.
    listener: Listener,
    /// How loud music is.
    music_volume: f32,
    /// `ambient_level`: how much of a leaf's level is heard.
    ambient_level: f32,
    /// Picks how far a repeated sound skips.
    seed: u32,
}

impl Audio {
    /// Audio on the default device, or silent with no device.
    #[must_use]
    pub fn new() -> Self {
        Self::with_output(Output::open())
    }

    /// Audio with no device.
    #[must_use]
    pub fn silent() -> Self {
        Self::with_output(None)
    }

    /// Audio on `output`.
    fn with_output(output: Option<Output>) -> Self {
        Self {
            output,
            clips: Vec::new(),
            channels: std::iter::repeat_with(|| None)
                .take(DYNAMIC_CHANNELS)
                .collect(),
            statics: Vec::new(),
            static_groups: Vec::new(),
            ambients: [Ambient::default(), Ambient::default()],
            listener: Listener::default(),
            music_volume: 1.0,
            ambient_level: 0.3,
            seed: 1,
        }
    }

    /// Whether sound reaches a device.
    #[must_use]
    pub fn has_device(&self) -> bool {
        self.output.is_some()
    }

    /// Takes in `clip`.
    #[expect(
        clippy::cast_precision_loss,
        reason = "sounds are far shorter than f64 can count"
    )]
    pub fn add_clip(&mut self, clip: &Clip) -> ClipId {
        self.clips.push(Loaded {
            prepared: Prepared::new(clip),
            seconds: clip.samples.len() as f64 / f64::from(clip.sample_rate.max(1)),
            looping: clip.loop_start.is_some(),
        });
        ClipId(self.clips.len() - 1)
    }

    /// `SND_PickChannel`: the channel for a new sound. The same entity and
    /// channel always gets replaced. Otherwise the sound with least left goes,
    /// and only the listener's own can replace the listener's own.
    fn pick_channel(&self, entity: u64, channel: u8, own: bool) -> Option<usize> {
        let mut best = None;
        let mut least = f64::INFINITY;
        for (index, slot) in self.channels.iter().enumerate() {
            if let Some(playing) = slot {
                if channel != 0 && playing.entity == entity && playing.channel == channel {
                    return Some(index);
                }
                if playing.own && !own {
                    continue;
                }
            }
            let left = slot.as_ref().map_or(0.0, |playing| playing.remaining);
            if best.is_none() || left < least {
                least = left;
                best = Some(index);
            }
        }
        best
    }

    /// A number from 0 up to 1, for what doesn't have to be repeatable.
    fn next_random(&mut self) -> f64 {
        self.seed = self
            .seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        f64::from(self.seed >> 8) / f64::from(1_u32 << 24)
    }

    /// Starts `sound` playing `clip`, unless it can't be heard from here.
    pub fn start(&mut self, sound: &SoundStart, clip: ClipId) {
        let Some(loaded) = self.clips.get(clip.0) else {
            return;
        };
        let (seconds, looping) = (loaded.seconds, loaded.looping);
        let Some(index) = self.pick_channel(sound.entity, sound.channel, sound.own) else {
            return;
        };
        if let Some(mut old) = self.channels[index].take()
            && let Some(voice) = &mut old.voice
        {
            voice.stop();
        }
        let mut channel = Channel {
            clip,
            origin: sound.origin,
            volume: sound.volume,
            attenuation: sound.attenuation,
            entity: sound.entity,
            channel: sound.channel,
            own: sound.own,
            remaining: if looping { f64::INFINITY } else { seconds },
            fresh: true,
            ears: (0.0, 0.0),
            voice: None,
        };
        channel.spatialize(&self.listener);
        if channel.ears.0 <= 0.0 && channel.ears.1 <= 0.0 {
            return;
        }
        // Two of the same sound at once would only sound like a louder one.
        let repeated = self
            .channels
            .iter()
            .flatten()
            .any(|other| other.clip == clip && other.fresh);
        let mut offset = 0.0;
        if repeated {
            offset = (self.next_random() * MAX_SKIP).min(seconds);
            channel.remaining -= offset;
        }
        if let (Some(output), Some(loaded)) = (&mut self.output, self.clips.get(clip.0)) {
            let (left, right) = channel.ears;
            channel.voice = output.play(&loaded.prepared, offset, left, right);
        }
        self.channels[index] = Some(channel);
    }

    /// Adds a sound at a fixed place, which has to be a loop.
    pub fn add_static(&mut self, sound: &StaticSound, clip: ClipId) {
        if self.clips.get(clip.0).is_some_and(|loaded| loaded.looping) {
            self.statics.push(Static {
                clip,
                origin: sound.origin,
                volume: sound.volume,
                attenuation: sound.attenuation,
            });
            if !self.static_groups.iter().any(|group| group.clip == clip) {
                self.static_groups.push(StaticGroup {
                    clip,
                    voice: None,
                    ears: (0.0, 0.0),
                });
            }
        }
    }

    /// Sets what plays for water and for sky in leaves that have them.
    pub fn set_ambient_clips(&mut self, water: Option<ClipId>, sky: Option<ClipId>) {
        self.ambients[0].clip = water;
        self.ambients[1].clip = sky;
    }

    /// `S_Update`: moves on by `seconds`, heard from `listener`, in a leaf
    /// with these levels of water and sky out of 255.
    pub fn update(&mut self, listener: Listener, seconds: f64, levels: [u8; 2]) {
        self.listener = listener;
        for slot in &mut self.channels {
            let Some(channel) = slot else {
                continue;
            };
            channel.fresh = false;
            channel.remaining -= seconds;
            if channel.remaining <= 0.0 {
                *slot = None;
                continue;
            }
            channel.spatialize(&listener);
            let (left, right) = channel.ears;
            if let Some(voice) = &mut channel.voice {
                voice.set_volumes(left, right);
            }
        }
        self.update_statics(&listener);
        self.update_ambients(seconds, levels);
    }

    /// Adds up the sounds at fixed places, and plays what can be heard.
    fn update_statics(&mut self, listener: &Listener) {
        for group in &mut self.static_groups {
            let (mut left, mut right) = (0.0, 0.0);
            for sound in self.statics.iter().filter(|sound| sound.clip == group.clip) {
                let (one_left, one_right) =
                    stereo_volumes(sound.origin, listener, sound.volume, sound.attenuation);
                left += one_left;
                right += one_right;
            }
            let (left, right) = (left.min(FULL_VOLUME), right.min(FULL_VOLUME));
            group.ears = (left, right);
            let audible = left > 0.0 || right > 0.0;
            match (&mut group.voice, audible, &mut self.output) {
                (Some(voice), true, _) => voice.set_volumes(left, right),
                (Some(voice), false, _) => {
                    voice.stop();
                    group.voice = None;
                }
                (None, true, Some(output)) => {
                    group.voice = self
                        .clips
                        .get(group.clip.0)
                        .and_then(|clip| output.play(&clip.prepared, 0.0, left, right));
                }
                (None, _, _) => {}
            }
        }
    }

    /// `S_UpdateAmbientSounds`.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a frame's fade needs no more than f32"
    )]
    fn update_ambients(&mut self, seconds: f64, levels: [u8; 2]) {
        for (ambient, level) in self.ambients.iter_mut().zip(levels) {
            let Some(clip) = ambient.clip else {
                continue;
            };
            let mut wanted = self.ambient_level * f32::from(level);
            if wanted < AMBIENT_FLOOR {
                wanted = 0.0;
            }
            let step = AMBIENT_FADE * seconds as f32;
            ambient.level = if ambient.level < wanted {
                (ambient.level + step).min(wanted)
            } else {
                (ambient.level - step).max(wanted)
            };
            let volume = ambient.level / 255.0;
            match (&mut ambient.voice, &mut self.output) {
                (Some(voice), _) => voice.set_volumes(volume, volume),
                (None, Some(output)) if volume > 0.0 => {
                    ambient.voice = self
                        .clips
                        .get(clip.0)
                        .and_then(|loaded| output.play(&loaded.prepared, 0.0, volume, volume));
                }
                (None, _) => {}
            }
        }
    }

    /// What's playing from entities.
    #[must_use]
    pub fn playing(&self) -> Vec<PlayingSound> {
        self.channels
            .iter()
            .flatten()
            .map(|channel| PlayingSound {
                clip: channel.clip,
                entity: channel.entity,
                channel: channel.channel,
                left: channel.ears.0,
                right: channel.ears.1,
            })
            .collect()
    }

    /// How many sounds there are at fixed places.
    #[must_use]
    pub fn static_count(&self) -> usize {
        self.statics.len()
    }

    /// The volume in each ear of each clip playing at fixed places.
    #[must_use]
    pub fn static_volumes(&self) -> Vec<(f32, f32)> {
        self.static_groups.iter().map(|group| group.ears).collect()
    }

    /// The most of the audio thread's time used since last asked, from 0 to 1.
    pub fn peak_load(&mut self) -> Option<f32> {
        self.output.as_mut().and_then(Output::peak_load)
    }

    /// The water and sky volumes, from 0 to 1.
    #[must_use]
    pub fn ambient_volumes(&self) -> [f32; 2] {
        [
            self.ambients[0].level / 255.0,
            self.ambients[1].level / 255.0,
        ]
    }

    /// `S_StopAllSounds`: silences everything but music, for a new level.
    pub fn stop_sounds(&mut self) {
        for mut channel in self.channels.iter_mut().filter_map(Option::take) {
            if let Some(voice) = &mut channel.voice {
                voice.stop();
            }
        }
        self.statics.clear();
        for mut group in self.static_groups.drain(..) {
            if let Some(voice) = &mut group.voice {
                voice.stop();
            }
        }
        for ambient in &mut self.ambients {
            ambient.level = 0.0;
            if let Some(mut voice) = ambient.voice.take() {
                voice.stop();
            }
        }
    }

    /// Sets how loud sound effects are, from 0 to 1.
    pub fn set_volume(&mut self, volume: f32) {
        if let Some(output) = &mut self.output {
            output.set_effects_volume(volume);
        }
    }

    /// Sets how much of a leaf's level is heard, from 0 to 1.
    pub fn set_ambient_level(&mut self, level: f32) {
        self.ambient_level = level;
    }

    /// Sets how loud music is, from 0 to 1.
    pub fn set_music_volume(&mut self, volume: f32) {
        self.music_volume = volume;
        if let Some(output) = &mut self.output {
            output.set_music_volume(volume);
        }
    }

    /// The longest gap between the audio thread's turns since last asked.
    #[must_use]
    pub fn longest_gap(&self) -> Option<Duration> {
        self.output.as_ref().map(Output::longest_gap)
    }

    /// Silences everything, or lets it back.
    pub fn set_muted(&mut self, muted: bool) {
        if let Some(output) = &mut self.output {
            output.set_muted(muted);
        }
    }

    /// Loops the music in `bytes`.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError`] if it can't be decoded.
    pub fn play_music(&mut self, bytes: Vec<u8>) -> Result<(), AudioError> {
        let volume = self.music_volume;
        self.output
            .as_mut()
            .map_or(Ok(()), |output| output.play_music(bytes, volume))
    }

    /// Stops the music.
    pub fn stop_music(&mut self) {
        if let Some(output) = &mut self.output {
            output.stop_music();
        }
    }
}

impl Default for Audio {
    fn default() -> Self {
        Self::new()
    }
}
