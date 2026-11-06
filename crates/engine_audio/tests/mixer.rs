//! The channel rules, with no sound device.

use std::sync::Arc;

use engine_audio::{
    Audio, ClipId, Fade, Listener, Room, amplitude_and_pan, stereo_volumes, volumes_by,
};
use engine_core::math::Vec3;
use engine_core::sound::{Clip, SoundStart, StaticSound};

/// A listener at the origin, facing along x, so the right ear is towards -y.
fn listener() -> Listener {
    Listener::default()
}

/// A clip of `seconds`, which loops if `looping`.
fn clip(audio: &mut Audio, seconds: u32, looping: bool) -> ClipId {
    let rate = 1000;
    let samples: Arc<[f32]> = vec![0.0; (rate * seconds) as usize].into();
    audio.add_clip(&Clip {
        sample_rate: rate,
        samples,
        loop_start: looping.then_some(0),
    })
}

/// A sound from `entity` on `channel`, nearby.
fn sound(entity: u64, channel: u8) -> SoundStart {
    SoundStart {
        entity,
        channel,
        origin: Vec3::new(100.0, 0.0, 0.0),
        volume: 1.0,
        attenuation: 1.0,
        own: false,
        name: String::new(),
    }
}

#[test]
fn a_sound_ahead_is_equal_in_both_ears_and_fades_with_distance() {
    let (left, right) = stereo_volumes(Vec3::new(500.0, 0.0, 0.0), &listener(), 1.0, 1.0);
    assert!((left - 0.5).abs() < 1e-5);
    assert!((right - 0.5).abs() < 1e-5);
}

#[test]
fn a_sound_to_the_right_is_only_in_the_right_ear() {
    let (left, right) = stereo_volumes(Vec3::new(0.0, -500.0, 0.0), &listener(), 1.0, 1.0);
    assert!(left.abs() < 1e-5);
    assert!((right - 0.5 * std::f32::consts::SQRT_2).abs() < 1e-5);
}

#[test]
fn turning_never_changes_how_loud_a_sound_is() {
    let power = |x: f32, y: f32| {
        let (left, right) = stereo_volumes(Vec3::new(x, y, 0.0), &listener(), 0.5, 1.0);
        left * left + right * right
    };
    let ahead = power(100.0, 0.0);
    for (x, y) in [
        (0.0, 100.0),
        (0.0, -100.0),
        (-100.0, 0.0),
        (60.0, 80.0),
        (-80.0, 60.0),
    ] {
        assert!((power(x, y) - ahead).abs() < 1e-5, "{x} {y}");
    }
}

#[test]
fn a_sound_beside_the_listener_is_not_doubled_in_the_near_ear() {
    let (_, right) = stereo_volumes(Vec3::new(0.0, -50.0, 0.0), &listener(), 0.5, 3.0);
    assert!(right < 0.61, "{right}");
}

#[test]
fn a_sound_past_its_range_is_silent_and_attenuation_zero_has_no_range() {
    let far = Vec3::new(1500.0, 0.0, 0.0);
    assert_eq!(stereo_volumes(far, &listener(), 1.0, 1.0), (0.0, 0.0));
    let (left, _) = stereo_volumes(far, &listener(), 1.0, 0.0);
    assert!((left - 1.0).abs() < 1e-5);
}

/// What an equal power panner does with an amplitude and a pan.
fn through_panner(amplitude: f32, pan: f32) -> (f32, f32) {
    let mix = (pan + 1.0) / 2.0;
    let root_two = std::f32::consts::SQRT_2;
    (
        amplitude * (1.0 - mix).sqrt() * root_two,
        amplitude * mix.sqrt() * root_two,
    )
}

#[test]
fn the_pan_gives_back_the_volumes_it_came_from() {
    for (left, right) in [(1.0, 1.0), (0.2, 0.9), (1.5, 0.0), (0.0, 0.7), (0.3, 0.3)] {
        let (amplitude, pan) = amplitude_and_pan(left, right);
        let (out_left, out_right) = through_panner(amplitude, pan);
        assert!((out_left - left).abs() < 1e-5, "{left} {right}");
        assert!((out_right - right).abs() < 1e-5, "{left} {right}");
    }
    assert_eq!(amplitude_and_pan(0.0, 0.0), (0.0, 0.0));
}

#[test]
fn a_sound_ends_when_its_time_is_up() {
    let mut audio = Audio::silent();
    let short = clip(&mut audio, 1, false);
    audio.start(&sound(1, 2), short);
    audio.update(listener(), 0.5, [0, 0]);
    assert_eq!(audio.playing().len(), 1);
    audio.update(listener(), 0.6, [0, 0]);
    assert!(audio.playing().is_empty());
}

#[test]
fn a_loop_plays_until_the_same_entity_and_channel_replaces_it() {
    let mut audio = Audio::silent();
    let hum = clip(&mut audio, 1, true);
    let stop = clip(&mut audio, 1, false);
    audio.start(&sound(1, 2), hum);
    audio.update(listener(), 60.0, [0, 0]);
    assert_eq!(audio.playing()[0].clip, hum);
    audio.start(&sound(1, 2), stop);
    let playing = audio.playing();
    assert_eq!(playing.len(), 1);
    assert_eq!(playing[0].clip, stop);
}

#[test]
fn another_channel_or_entity_plays_alongside() {
    let mut audio = Audio::silent();
    let beep = clip(&mut audio, 1, false);
    audio.start(&sound(1, 2), beep);
    audio.start(&sound(1, 4), beep);
    audio.start(&sound(2, 2), beep);
    assert_eq!(audio.playing().len(), 3);
}

#[test]
fn channel_zero_never_replaces() {
    let mut audio = Audio::silent();
    let beep = clip(&mut audio, 1, false);
    audio.start(&sound(1, 0), beep);
    audio.start(&sound(1, 0), beep);
    assert_eq!(audio.playing().len(), 2);
}

#[test]
fn a_sound_out_of_earshot_does_not_start() {
    let mut audio = Audio::silent();
    let beep = clip(&mut audio, 1, false);
    let far = SoundStart {
        origin: Vec3::new(5000.0, 0.0, 0.0),
        ..sound(1, 2)
    };
    audio.start(&far, beep);
    assert!(audio.playing().is_empty());
}

#[test]
fn the_listeners_own_sounds_are_full_volume_anywhere() {
    let mut audio = Audio::silent();
    let beep = clip(&mut audio, 1, false);
    let own = SoundStart {
        origin: Vec3::new(5000.0, 0.0, 0.0),
        own: true,
        ..sound(1, 2)
    };
    audio.start(&own, beep);
    let playing = audio.playing();
    assert_eq!((playing[0].left, playing[0].right), (1.0, 1.0));
}

#[test]
fn when_full_the_sound_with_least_left_goes_but_not_the_listeners_own() {
    let mut audio = Audio::silent();
    let long = clip(&mut audio, 100, false);
    let short = clip(&mut audio, 1, false);
    let own = SoundStart {
        own: true,
        ..sound(1000, 2)
    };
    audio.start(&own, short);
    audio.start(&sound(1001, 2), short);
    for entity in 0..126 {
        audio.start(&sound(entity, 2), long);
    }
    assert_eq!(audio.playing().len(), 128);
    audio.start(&sound(2000, 2), long);
    let playing = audio.playing();
    assert_eq!(playing.len(), 128);
    assert!(playing.iter().any(|sound| sound.entity == 1000));
    assert!(!playing.iter().any(|sound| sound.entity == 1001));
    assert!(playing.iter().any(|sound| sound.entity == 2000));
}

#[test]
fn volumes_follow_the_listener() {
    let mut audio = Audio::silent();
    let hum = clip(&mut audio, 1, true);
    audio.start(&sound(1, 2), hum);
    let near = audio.playing()[0].left;
    let moved = Listener {
        origin: Vec3::new(-500.0, 0.0, 0.0),
        ..listener()
    };
    audio.update(moved, 0.01, [0, 0]);
    assert!(audio.playing()[0].left < near);
}

#[test]
fn a_static_sound_has_to_loop_and_stays_where_it_was_put() {
    let mut audio = Audio::silent();
    let hum = clip(&mut audio, 1, true);
    let beep = clip(&mut audio, 1, false);
    let sound = StaticSound {
        origin: Vec3::new(0.0, -100.0, 0.0),
        volume: 0.5,
        attenuation: 3.0,
        name: String::new(),
    };
    audio.add_static(&sound, beep);
    assert!(audio.static_volumes().is_empty());
    audio.add_static(&sound, hum);
    audio.update(listener(), 0.01, [0, 0]);
    let (left, right) = audio.static_volumes()[0];
    assert!(right > left);
    let away = Listener {
        origin: Vec3::new(0.0, 900.0, 0.0),
        ..listener()
    };
    audio.update(away, 0.01, [0, 0]);
    assert_eq!(audio.static_volumes()[0], (0.0, 0.0));
}

#[test]
fn ambient_sound_fades_towards_the_leafs_level() {
    let mut audio = Audio::silent();
    let wind = clip(&mut audio, 1, true);
    audio.set_ambient_clips(None, Some(wind));
    audio.update(listener(), 0.1, [0, 255]);
    let first = audio.ambient_volumes()[1];
    assert!((first - 10.0 / 255.0).abs() < 1e-4, "{first}");
    audio.update(listener(), 10.0, [0, 255]);
    assert!((audio.ambient_volumes()[1] - 0.3).abs() < 1e-4);
    assert!(audio.ambient_volumes()[0].abs() < f32::EPSILON);
    audio.update(listener(), 10.0, [0, 20]);
    assert!(
        audio.ambient_volumes()[1].abs() < f32::EPSILON,
        "under the floor"
    );
}

#[test]
fn a_new_level_silences_everything() {
    let mut audio = Audio::silent();
    let hum = clip(&mut audio, 1, true);
    audio.start(&sound(1, 2), hum);
    audio.add_static(
        &StaticSound {
            origin: Vec3::ZERO,
            volume: 1.0,
            attenuation: 3.0,
            name: String::new(),
        },
        hum,
    );
    audio.stop_sounds();
    assert!(audio.playing().is_empty());
    assert!(audio.static_volumes().is_empty());
}

#[test]
fn the_real_device_opens_and_plays_when_there_is_one() {
    let mut audio = Audio::new();
    let hum = clip(&mut audio, 1, true);
    audio.start(&sound(1, 2), hum);
    audio.update(listener(), 0.01, [0, 0]);
    audio.set_volume(0.5);
    audio.set_muted(true);
    audio.stop_sounds();
    assert!(audio.play_music(vec![1, 2, 3]).is_err() || !audio.has_device());
}

/// A static sound at `origin`.
fn fixed(origin: Vec3, volume: f32) -> StaticSound {
    StaticSound {
        origin,
        volume,
        attenuation: 3.0,
        name: String::new(),
    }
}

#[test]
fn statics_of_one_clip_add_up() {
    let mut audio = Audio::silent();
    let hum = clip(&mut audio, 1, true);
    audio.add_static(&fixed(Vec3::new(10.0, 0.0, 0.0), 0.2), hum);
    audio.add_static(&fixed(Vec3::new(20.0, 0.0, 0.0), 0.2), hum);
    audio.update(listener(), 0.01, [0, 0]);
    let volumes = audio.static_volumes();
    assert_eq!(volumes.len(), 1, "one voice for the clip");
    assert_eq!(audio.static_count(), 2);
    assert!(volumes[0].0 > 0.3 && volumes[0].0 < 0.4, "{volumes:?}");
}

#[test]
fn statics_of_one_clip_never_pass_full_volume() {
    let mut audio = Audio::silent();
    let hum = clip(&mut audio, 1, true);
    for _ in 0..8 {
        audio.add_static(&fixed(Vec3::new(10.0, 0.0, 0.0), 0.5), hum);
    }
    audio.update(listener(), 0.01, [0, 0]);
    assert_eq!(audio.static_volumes(), [(1.0, 1.0)]);
}

#[test]
fn statics_of_different_clips_stay_apart() {
    let mut audio = Audio::silent();
    let hum = clip(&mut audio, 1, true);
    let drip = clip(&mut audio, 1, true);
    audio.add_static(&fixed(Vec3::new(10.0, 0.0, 0.0), 0.5), hum);
    audio.add_static(&fixed(Vec3::new(10.0, 0.0, 0.0), 0.5), drip);
    audio.update(listener(), 0.01, [0, 0]);
    assert_eq!(audio.static_volumes().len(), 2);
}

#[test]
fn a_sound_right_beside_the_listener_stops_at_full_volume() {
    let mut audio = Audio::silent();
    let beep = clip(&mut audio, 1, false);
    let beside = SoundStart {
        origin: Vec3::new(0.0, -10.0, 0.0),
        ..sound(1, 2)
    };
    audio.start(&beside, beep);
    let playing = audio.playing();
    assert!((playing[0].right - 1.0).abs() < f32::EPSILON);
    assert!(playing[0].left < 0.1);
}

#[test]
fn the_ambient_level_can_be_changed_while_playing() {
    let mut audio = Audio::silent();
    let wind = clip(&mut audio, 1, true);
    audio.set_ambient_clips(None, Some(wind));
    audio.update(listener(), 10.0, [0, 255]);
    assert!((audio.ambient_volumes()[1] - 0.3).abs() < 1e-4);
    audio.set_ambient_level(0.15);
    audio.update(listener(), 10.0, [0, 255]);
    assert!((audio.ambient_volumes()[1] - 0.15).abs() < 1e-4);
}

#[test]
fn water_has_its_own_share_of_the_ambient_level() {
    let mut audio = Audio::silent();
    let water = clip(&mut audio, 1, true);
    let wind = clip(&mut audio, 1, true);
    audio.set_ambient_clips(Some(water), Some(wind));
    audio.update(listener(), 10.0, [255, 255]);
    let [water_volume, wind_volume] = audio.ambient_volumes();
    assert!((water_volume - 0.15).abs() < 1e-4, "{water_volume}");
    assert!((wind_volume - 0.3).abs() < 1e-4, "{wind_volume}");
    audio.set_water_share(1.0);
    audio.update(listener(), 10.0, [255, 255]);
    assert!((audio.ambient_volumes()[0] - 0.3).abs() < 1e-4);
}

#[test]
fn natural_fade_falls_quickly_then_slowly_and_ends_where_a_straight_line_does() {
    let (near, half, far) = (
        Fade::Natural.at(0.1),
        Fade::Natural.at(0.5),
        Fade::Natural.at(0.9),
    );
    assert!(near > 0.7 && near < 1.0, "{near}");
    assert!(half < 0.3, "{half}");
    assert!(far > 0.0 && far < 0.05, "{far}");
    assert!(Fade::Natural.at(1.0).abs() < 1e-6);
    assert!(Fade::Natural.at(1.5).abs() < 1e-6);
    assert!((Fade::Straight.at(0.5) - 0.5).abs() < 1e-6);
}

#[test]
fn gunfire_carries_further_than_other_sounds() {
    assert_eq!(Fade::of("weapons/rocket1i.wav"), Fade::Straight);
    assert_eq!(Fade::of("soldier/pain1.wav"), Fade::Natural);
    let ears = |fade| volumes_by(fade, Vec3::new(500.0, 0.0, 0.0), &listener(), 1.0, 1.0);
    assert!(ears(Fade::Straight).0 > ears(Fade::Natural).0 * 1.5);
}

/// A sound at `x` along from the listener, from entity 5.
fn sound_at(x: f32) -> SoundStart {
    let mut started = sound(5, 1);
    started.origin = Vec3::new(x, 0.0, 0.0);
    started.volume = 1.0;
    started.attenuation = 1.0;
    started
}

#[test]
fn a_sound_follows_what_made_it() {
    let mut audio = Audio::silent();
    let clip = clip(&mut audio, 10, false);
    audio.update(listener(), 0.0, [0, 0]);
    audio.start(&sound_at(100.0), clip);
    audio.update(listener(), 0.01, [0, 0]);
    let near = audio.playing()[0].left;
    audio.follow(|entity| (entity == 5).then_some(Vec3::new(600.0, 0.0, 0.0)));
    audio.update(listener(), 0.01, [0, 0]);
    let far = audio.playing()[0].left;
    assert!(far < near * 0.5, "{far} of {near}");
}

#[test]
fn a_sound_whose_maker_has_gone_stays_where_it_was() {
    let mut audio = Audio::silent();
    let clip = clip(&mut audio, 10, false);
    audio.update(listener(), 0.0, [0, 0]);
    audio.start(&sound_at(100.0), clip);
    audio.update(listener(), 0.01, [0, 0]);
    let before = audio.playing()[0].left;
    audio.follow(|_| None);
    audio.update(listener(), 0.01, [0, 0]);
    assert!((audio.playing()[0].left - before).abs() < 1e-6);
}

#[test]
fn a_wall_muffles_a_sound_soon_after_it_gets_in_the_way() {
    let mut audio = Audio::silent();
    let clip = clip(&mut audio, 10, false);
    audio.update(listener(), 0.0, [0, 0]);
    audio.start(&sound_at(100.0), clip);
    audio.update(listener(), 0.01, [0, 0]);
    let clear = audio.playing()[0].left;
    audio.occlude(0.02, |_| true);
    audio.update(listener(), 0.01, [0, 0]);
    let going = audio.playing()[0].left;
    assert!(
        going < clear && going > clear * 0.5,
        "it eases: {going} of {clear}"
    );
    audio.occlude(1.0, |_| true);
    audio.update(listener(), 0.01, [0, 0]);
    let behind = audio.playing()[0].left;
    assert!((behind / clear - 0.35).abs() < 0.01, "{behind} of {clear}");
    audio.occlude(1.0, |_| false);
    audio.update(listener(), 0.01, [0, 0]);
    assert!((audio.playing()[0].left - clear).abs() < 1e-5);
}

#[test]
fn a_listeners_own_sound_has_no_wall_in_the_way() {
    let mut audio = Audio::silent();
    let clip = clip(&mut audio, 10, false);
    let mut own = sound_at(0.0);
    own.own = true;
    audio.update(listener(), 0.0, [0, 0]);
    audio.start(&own, clip);
    audio.occlude(1.0, |_| true);
    audio.update(listener(), 0.01, [0, 0]);
    assert!((audio.playing()[0].left - 1.0).abs() < 1e-6);
}

#[test]
fn a_sound_that_starts_behind_a_wall_is_quiet_from_the_start() {
    let mut audio = Audio::silent();
    let clip = clip(&mut audio, 10, false);
    audio.update(listener(), 0.0, [0, 0]);
    audio.start_behind(&sound_at(100.0), clip, true);
    let behind = audio.playing()[0].left;
    let mut open = Audio::silent();
    let other = self::clip(&mut open, 10, false);
    open.update(listener(), 0.0, [0, 0]);
    open.start(&sound_at(100.0), other);
    assert!(behind < open.playing()[0].left * 0.5);
}

#[test]
fn loud_sounds_push_the_rest_down_and_it_comes_back() {
    let mut audio = Audio::silent();
    let clip = clip(&mut audio, 1, false);
    audio.update(listener(), 0.0, [0, 0]);
    assert!((audio.ducking() - 1.0).abs() < 1e-6);
    let mut loud = sound_at(0.0);
    loud.own = true;
    audio.start(&loud, clip);
    for _ in 0..10 {
        audio.update(listener(), 0.016, [0, 0]);
    }
    assert!(audio.ducking() < 0.55, "{}", audio.ducking());
    for _ in 0..200 {
        audio.update(listener(), 0.016, [0, 0]);
    }
    assert!(audio.ducking() > 0.99, "{}", audio.ducking());
}

#[test]
fn a_big_room_rings_longer_than_a_small_one_and_the_open_not_at_all() {
    let small = Room::from_rays(&[Some(100.0); 6]);
    let big = Room::from_rays(&[Some(900.0); 6]);
    let open = Room::from_rays(&[None; 6]);
    assert!(big.feedback() > small.feedback());
    assert!(big.send() > small.send());
    assert!(open.send().abs() < 1e-6);
    assert!(Room::from_rays(&[]).open.abs() < 1e-6);
}

#[test]
fn a_room_half_open_sends_half_as_much_back() {
    let closed = Room::from_rays(&[Some(400.0), Some(400.0)]);
    let half = Room::from_rays(&[Some(400.0), None]);
    assert!((half.send() - closed.send() * 0.5).abs() < 1e-6);
}
