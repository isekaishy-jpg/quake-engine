# Proposal 0030: sound data

Status: accepted, 2026-11-04, increment 5.

## Research

- The original paks hold 225 WAVs, 3.7 MB in all: all mono, mostly 8-bit at 11025 Hz, some 16-bit or 22050 Hz.
- 23 have a `cue ` chunk, which is how Quake marks a loop. `GetWavinfo` reads the loop start from it, and a `LIST` chunk after it can end the sound early, with a `mark` length. A generic decoder wouldn't read either.
- Music was CD audio in the original. Steam only has it as `rerelease/<game>/music/trackNN.ogg`, 79 MB for `id1`, so an original `id1` folder has none.
- `SV_SendServerinfo` sends worldspawn's `sounds` as the track. `e1m1` is 6 and `start` is 4.

## Design

- `engine_core::sound::Clip` holds float samples, a rate and a loop start, so `fixture_quake` and `engine_audio` don't know each other.
- `fixture_quake::parse_wav` ports `GetWavinfo`. Anything else is refused, not guessed at.
- `mount_music` mounts the loose files of `rerelease/<game>`, behind everything else.
- `engine_app` turns names into clips on first use, and keeps them. Ogg bytes go to `engine_audio`, which streams them.

## Proposal

- **Selected approach:** the above.
- **Direct upgrades:** stereo files mix down instead of being refused, and music streams from files instead of a disc.
- **Tradeoffs:** a sound loads the first time it plays, where Quake precached it. A read from a pak is far under a frame, and every sound together is 3.7 MB. Music reads a whole file into memory, a few megabytes.
- **Acceptance criteria:** tests cover 8 and 16-bit samples, the loop start, the `mark` length and bad files. Every WAV in the game parses, with 23 loops.
- **Future extension:** `assets_manager` can take over caching when models arrive. Other formats only need a parser that makes a `Clip`.

## Review

See [review 0008](../reviews/0008-increment-5.md).
