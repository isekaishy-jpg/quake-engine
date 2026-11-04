# Review 0008: increment 5

Status: review record, 2026-11-04. Reviewed: proposals 0029 to 0031, and the changes to `engine_core`, `engine_audio`, `engine_map`, `fixture_quake`, `engine_net`, `engine_sim`, `engine_script`, `engine_server`, `engine_client` and `engine_app`.

| # | Finding | Handling |
|---|---|---|
| 1 | A door or lift's movement sound is a looped WAV on an ordinary channel, which plays until its stop sound replaces it. Treating loops as statics only would have left doors silent or humming forever | Channels loop until replaced, with tests on the mixer and on real door and lift scripts |
| 2 | The port's `sound` stub had dropped the channel, volume and attenuation, and channels are what make finding 1 work | Every call has them back, from the QuakeC |
| 3 | The port had no `ambient_*`, torch or flame classes, or `light_fluorospark`, so maps' static sounds never spawned. `light_fluoro` was only an alias of `light`, which removes a light nobody targets | All ported. Lights that show a model keep only their sound until models arrive |
| 4 | `SoundStart` first held entities as 32 bits, but ids are 64 with their generation, so a long game could have confused two entities' channels | It holds 64 |
| 5 | Music isn't in the original folders at all, only under `rerelease/<game>/music` | `mount_music` mounts it behind everything else |
| 6 | In play, ambient sounds needed checking. Statics of one clip played separately and added up with no limit, where `S_Update` merges them into one channel | They share a voice, capped at full volume. `e1m1`'s 14 statics are 5 voices, and `start`'s 55 are 6 |
| 7 | No channel was capped, where `SND_PaintChannelFrom8` stops each ear at 255. A loud sound beside the player reached twice full volume in one ear | Every ear stops at full volume, with a test |
| 8 | In play, audio crackled when alt-tabbing away. The audio thread wasn't optimized in `dev` builds, and peaked at 99.7% of its time in a short run, with no alt-tab. Kira's documentation warns of this | `kira`, `cpal` and the decoders are optimized in `dev`, which brought the peak down to 3.5% in the same run. `stats` shows the peak |
| 9 | The mute fade was 30 ms. Fades run in decibels, so most of the drop came in the first few milliseconds | It's 250 ms |
| 10 | In play, alt-tabbing still popped on a busy machine. `cpal-realtime` had been left out, so the audio thread ran at ordinary priority, and a late turn doesn't show in its load | It's on. With 8 spinning processes on top of a loaded machine, the longest gap between turns fell from 33 to 39 ms to 12.8 ms, where a normal turn is about 10. `stats` shows the gap |
| 11 | In play, ambient sound was too loud against the regular client, with no sense of distance. Statics matched the original's law, measured live. Wind and water come from leaf levels, and `vis` overwrites the distance it works out, so those are silent or full | `AmbientSounds` measures it live, as `vis` intended, with tests. At three spots in `e1m1` with no water nearby, water fell from 0.30 to 0, 0 and 0.13 |
| 12 | In play, sounds were louder close up than they should be. Quake's stereo adds to the near ear, so a drone of volume 0.5 reached 0.85 in one ear at 50 units, and turning changed how loud it was | Stereo shares constant power between the ears, with tests |
| 13 | Water and wind have no place, so finding 12 doesn't touch them. Their level matches WinQuake's numbers, so what's right is a question of balance | `ambient_level` changes them while playing, so the default can be set by ear |
| 14 | In play, water was the main culprit. Finding 11 lets sky fade, since it's usually far off, but a player near water is nearly always within full volume of it, so water stayed at the top of its range | Water gets its own share of the level, `ambient_water`, which starts at half |

## Verified

These ran against the Steam data, read in place.

| Check | Result |
|---|---|
| Every WAV in the game | 225 parse, with 23 loops |
| Every sound scripts ask for, firing everything on every `id1` map | 44 different sounds, none missing |
| Every touch and target on all 117 maps | No script errors |
| Every music track | All 10 decode |
| `e1m1` and `e1m2` in the real engine | The device opens, tracks 6 and 8 play, as the maps ask, with 14 and 47 static sounds and no sound missing |
| 8 ambience tests | Distance, the nearest surface, what makes a sound, and stored levels as a limit |
| 24 mixer tests | The stereo, pan conversion, override, stealing, the player's priority, loops, statics, the cap on each ear and ambient fading |
| Door, lift, teleport, jump and landing tests | The real scripts send the right sounds on the right channels |

`cargo xtask check` passes.

## Remaining

- Played by hand after finding 14, and it sounds much better, with the distance fade working and no more popping heard when alt-tabbing. If popping comes back, `stats` shows the longest gap to start from.
- The regular client's settings have a low-pass filter on, at 6833 Hz. Run through Kira's interpolation, Quake's sounds have under 1% of their energy above 5.5 kHz, and a filter like that changes them by at most 0.2 dB, so it isn't copied.
- `ambient_level` is still the original's 0.3, and `ambient_water` stays at 0.5, which sounded right by hand. Neither is measured against anything.
- Once, before finding 10, the engine took over 30 s to quit under heavy load. It didn't happen again, and the cause isn't known.
- This is a port with fixes on top. [Real sound features](../research/plan.md#real-sound-features) are planned to replace it.
- Sounds stay where they started, as in WinQuake, so a lift's hum doesn't rise with it. Following entities needs entity numbers in snapshots.
- Sound names travel as text in each message, which a precache table replaces before online play.
- Weapons, monsters, pain, death and drowning have no sounds until those exist.
- A sound loads the first time it plays, and music reads from disk on the first tick of a level.
- Mixing has no limiter, so many loud sounds at once can clip. Kira's effects can add one.
