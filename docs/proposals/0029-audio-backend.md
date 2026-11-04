# Proposal 0029: the audio backend

Status: accepted, 2026-11-04, increment 5.

## Research

- `kira` 0.12.6 is current, under MIT or Apache 2.0. It plays through `cpal`, and decodes through `symphonia`, which is MPL 2.0 and so can be used in a GPL project. Only `ogg` and `vorbis` are needed, since [proposal 0030](0030-sound-data.md) parses WAV itself.
- `StaticSoundData` has public fields, so decoded samples go straight in. It resamples, loops a region, and takes volume and panning as values that can change while playing.
- `AudioManager::new` fails with `NoDefaultOutputDevice` on a machine with no output.
- Tracks hold 128 sounds by default, and a builder raises that.
- `Frame::panned` is equal power: `left = a * sqrt(1 - m) * sqrt(2)` and `right = a * sqrt(m) * sqrt(2)`, with `m = (pan + 1) / 2`.
- `snd_dma.c` has 8 dynamic channels and 128 in all, which are 1996 mixing budgets. `SND_Spatialize` sets `left = v * (1 - d) * (1 - dot)` and `right = v * (1 - d) * (1 + dot)`, where `d` is distance over 1000 times attenuation, and `dot` is the listener's right against the direction of the sound.
- `SND_PickChannel` replaces a sound on the same entity and channel, except on channel 0. Otherwise it takes the channel with the least left to play, and never takes a player's sound for anyone else's.
- `SND_PaintChannelFrom8` caps each ear of a channel at 255. `S_Update` adds statics of the same sound into one channel first, so several torches sound like one louder torch, and never past full volume.
- Kira's documentation warns that the `dev` profile performs poorly, and asks for optimized audio crates.
- A looped WAV on a dynamic channel plays until it's replaced. Door and lift movement sounds rely on this: the stop sound replaces the loop on the same channel.

## Design

- `engine_audio::Audio` owns the channels, and `kira` stays inside it. `output.rs` is the only file that names `kira`.
- Quake's two volumes turn exactly into Kira's values: `a = sqrt((L² + R²) / 2)` and `pan = 2R² / (L² + R²) - 1`.
- `spatial.rs` holds that maths, as pure functions.
- Channel logic runs the same with no output, so tests can check it, and so a machine with no sound device still plays.
- Statics that play one clip share a voice, which also saves mixing them.
- `kira`, `cpal` and the decoders are optimized in the `dev` profile.
- Sound effects share a track, which `volume` sets. Music plays beside it, which `bgmvolume` sets.

## Proposal

- **Selected approach:** the above, keeping Quake's override rules, falloff and stereo, as they're part of how the game sounds.
- **Direct upgrades:** 128 dynamic channels, statics with no limit, float mixing and resampling, and volume changes smoothed over 15 ms instead of stepped.
- **Tradeoffs:** each sound's volume is sent to the mixer each frame it changes. Statics only hold a voice while audible, so a loop restarts when it's heard again, which can't be noticed.
- **Acceptance criteria:** tests cover the stereo law, the pan conversion, channel override, stealing, the player's priority, looping, the same sound starting twice, the cap on each ear, and adding up statics. A real output plays when one exists, and the engine runs without one.
- **Future extension:** a replacement for `spatial.rs` can add sounds that follow entities, occlusion or reverb, since nothing else knows the law. Kira's effects can hang off the effects track.

## Review

See [review 0008](../reviews/0008-increment-5.md).
