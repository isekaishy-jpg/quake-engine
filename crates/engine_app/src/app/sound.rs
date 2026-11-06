//! What's heard.

use engine_audio::Room;
use engine_core::math::Vec3;
use engine_physics::CollisionWorld;
use fixture_quake::music_file;

use super::World;

/// How often the room round the listener is measured, in seconds.
const ROOM_SECONDS: f64 = 0.25;

/// How far a ray goes looking for the room's walls.
const ROOM_REACH: f32 = 2048.0;

/// The ways a room is looked at, from a listener.
const ROOM_RAYS: [(f32, f32, f32); 10] = [
    (1.0, 0.0, 0.0),
    (-1.0, 0.0, 0.0),
    (0.0, 1.0, 0.0),
    (0.0, -1.0, 0.0),
    (0.0, 0.0, 1.0),
    (0.0, 0.0, -1.0),
    (0.7, 0.7, 0.0),
    (-0.7, 0.7, 0.0),
    (0.7, -0.7, 0.0),
    (-0.7, -0.7, 0.0),
];

/// Whether a straight line from the listener reaches `place`. With no world, it does.
fn heard_from(world: Option<&CollisionWorld>, listener: Vec3, place: Vec3) -> bool {
    world.is_none_or(|world| world.with(&[]).trace(listener, place).fraction >= 1.0)
}

/// How the room round `origin` sounds, from how far it is to what's there. What a ray
/// doesn't reach, it takes to be open.
fn room_round(world: &CollisionWorld, origin: Vec3) -> Room {
    let view = world.with(&[]);
    let rays: Vec<Option<f32>> = ROOM_RAYS
        .iter()
        .map(|(x, y, z)| {
            let end = origin + Vec3::new(*x, *y, *z) * ROOM_REACH;
            let fraction = view.trace(origin, end).fraction;
            (fraction < 1.0).then_some(fraction * ROOM_REACH)
        })
        .collect();
    Room::from_rays(&rays)
}

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
        let (world, listener) = (self.collision.clone(), self.listener.origin);
        let mut sounds = client.take_sounds();
        sounds.extend(self.effects.take_sounds());
        let static_sounds = client.take_static_sounds();
        let music = client.take_music();
        for sound in sounds {
            let clip = self
                .sounds
                .clip(&sound.name, &self.filesystem, &mut self.audio);
            if let Some(clip) = clip {
                let behind = !sound.own && !heard_from(world.as_deref(), listener, sound.origin);
                self.audio.start_behind(&sound, clip, behind);
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

    /// Plays what a zone names: a track by number, or a file.
    pub(crate) fn play_named_music(&mut self, music: &str) {
        if let Ok(track) = music.parse::<u8>() {
            self.play_track(track);
            return;
        }
        self.music_track = None;
        let bytes = self.filesystem.read(music);
        let playing = bytes.is_ok_and(|bytes| self.audio.play_music(bytes).is_ok());
        if !playing {
            self.audio.stop_music();
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
        if let Some(client) = &self.client {
            self.audio.follow(|entity| client.entity_origin(entity));
        }
        let world = self.collision.clone();
        #[expect(clippy::cast_possible_truncation, reason = "a frame fits f32")]
        self.audio.occlude(seconds as f32, |place| {
            !heard_from(world.as_deref(), origin, place)
        });
        self.room_wait -= seconds;
        if self.room_wait <= 0.0 {
            self.room_wait = ROOM_SECONDS;
            if let Some(world) = world.as_deref() {
                self.audio.set_room(room_round(world, origin));
            }
        }
        let levels = self
            .ambience
            .as_mut()
            .map_or([0, 0], |ambience| ambience.levels(origin));
        self.audio.update(self.listener, seconds, levels);
    }
}
