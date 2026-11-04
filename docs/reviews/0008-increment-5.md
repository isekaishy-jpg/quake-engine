# Review 0008: increment 5

Status: review record, 2026-11-04. Reviewed: proposals 0029 to 0031, and the changes to `engine_core`, `engine_audio`, `engine_map`, `fixture_quake`, `engine_net`, `engine_sim`, `engine_script`, `engine_server`, `engine_client` and `engine_app`.

| # | Finding | Handling |
|---|---|---|
| 1 | A door or lift's movement sound is a looped WAV on an ordinary channel, which plays until its stop sound replaces it. Treating loops as statics only would have left doors silent or humming forever | Channels loop until replaced, with tests on the mixer and on real door and lift scripts |
| 2 | The port's `sound` stub had dropped the channel, volume and attenuation, and channels are what make finding 1 work | Every call has them back, from the QuakeC |
| 3 | The port had no `ambient_*`, torch or flame classes, or `light_fluorospark`, so maps' static sounds never spawned. `light_fluoro` was only an alias of `light`, which removes a light nobody targets | All ported. Lights that show a model keep only their sound until models arrive |
| 4 | `SoundStart` first held entities as 32 bits, but ids are 64 with their generation, so a long game could have confused two entities' channels | It holds 64 |
| 5 | Music isn't in the original folders at all, only under `rerelease/<game>/music` | `mount_music` mounts it behind everything else |

## Verified

These ran against the Steam data, read in place.

| Check | Result |
|---|---|
| Every WAV in the game | 225 parse, with 23 loops |
| Every sound scripts ask for, firing everything on every `id1` map | 44 different sounds, none missing |
| Every touch and target on all 117 maps | No script errors |
| Every music track | All 10 decode |
| `e1m1` and `e1m2` in the real engine | The device opens, tracks 6 and 8 play, as the maps ask, with 14 and 47 static sounds and no sound missing |
| 16 mixer tests | The stereo law, pan conversion, override, stealing, the player's priority, loops, statics and ambient fading |
| Door, lift, teleport, jump and landing tests | The real scripts send the right sounds on the right channels |

`cargo xtask check` passes.

## Remaining

- Nobody has listened to it yet. Tests read the mixer's numbers, not what comes out of the speakers.
- Sounds stay where they started, as in WinQuake, so a lift's hum doesn't rise with it. Following entities needs entity numbers in snapshots.
- Sound names travel as text in each message, which a precache table replaces before online play.
- Weapons, monsters, pain, death and drowning have no sounds until those exist.
- A sound loads the first time it plays, and music reads from disk on the first tick of a level.
- Mixing has no limiter, so many loud sounds at once can clip. Kira's effects can add one.
