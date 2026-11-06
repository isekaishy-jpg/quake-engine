# Proposal 0129: real sound

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

[The research plan](../research/plan.md) lists what increment 5 left as stopgaps, a port with fixes on top. It also lists entity culling, which [proposal 0088](0088-visibility-and-entity-culling.md) already did: each seat's snapshot leaves out what it can't see, and the plan's table was never crossed off. So item 28 is sound.

Taking the plan's table in order:

| Stopgap | Today's practice | Here |
|---|---|---|
| A straight line of volume over distance | Inverse distance with a limit, and a curve for each kind of sound | A curve, and a kind of sound by its name |
| Sounds stay where they started | Emitters attached to what makes them | Sounds follow their entity |
| Sounds pass through walls | Occlusion by raycast, with a low pass | The same |
| No sense of the room | Reverb zones, or room size from rays | Room size from rays |
| One track with no limiter | Buses, ducking, a limiter on the output | The same |
| Fixed levels | Sliders for each bus | `volume`, `bgmvolume` and `ambient_level` |

`kira` has what's needed: sub tracks, send tracks, and filter, reverb and compressor effects. `kira`'s own spatial tracks would mean a track for every voice, up to 128, each with its own effect chain, for what `spatial.rs` does already in a few lines, with tests.

Surround and binaural aren't here. `kira` mixes to stereo, and a good binaural mix needs a head model nobody here has asked for. Equal power panning stays, which is what its tests hold.

Water and wind already have a level for each, and zones have their own sound from [proposal 0116](0116-zones.md), so that row of the plan has what it needs.

## Design

- **`Fade`.** `Natural` is what most sounds get: about a quarter as loud by halfway, ending where the old line ended. `Straight` is the old line, for gunfire, which has to carry across a level. Which a sound gets goes by its name.
- **Following.** `Audio::follow` moves each sound to where its entity is now, by `client.entity_origin`. A sound whose entity has gone stays where it was last.
- **Occlusion.** A line from the listener says what's behind something. It takes 0.15 s to go to 35% and back, so a door or a corner doesn't click. A sound that starts behind a wall goes through a bus with a low pass at 900 Hz too. The listener's own sounds are never behind anything.
- **The room.** Four times a second, ten rays from the listener measure the room. A bigger room rings for longer and sends more back, and what's open to the sky sends nothing. It changes over 0.4 s.
- **The mix.** Effects, muffled effects and ambience are buses, music is its own, and all of them go through a limiter on the output, so many loud sounds at once don't clip. Under a loud sound, ambience and music drop by half over 0.05 s and come back over 0.6 s.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a looping sound keeps the bus it started on, so going behind a wall turns it down without the low pass. Rays are against the world, so a closed door doesn't block sound.
- **Acceptance criteria:** curves, following, muffling, ducking and rooms behave as above, nothing the listener does to themself is muffled, and it runs on the real device.
- **Future extension:** none planned.

## Review

Each of those is tested with no device. The game was run on the real device with this mix, firing weapons, and the audio thread peaked at 1.3%.

**How it sounds needs a person.** No test can say whether a room's reverb is too much, or if a muffled voice sounds right. Those numbers are at the top of `spatial.rs` and `output.rs`.
