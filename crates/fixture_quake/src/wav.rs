//! Quake's sounds, following `GetWavinfo` in `snd_mem.c`.

use std::sync::Arc;

use engine_core::sound::Clip;

/// The chunk `name` in `bytes`, from its header on, and where the next starts.
fn find_chunk<'a>(bytes: &'a [u8], name: &[u8; 4]) -> Option<(&'a [u8], &'a [u8])> {
    let mut rest = bytes;
    loop {
        let (id, after) = rest.split_first_chunk::<4>()?;
        let (size, _) = after.split_first_chunk::<4>()?;
        let size = usize::try_from(u32::from_le_bytes(*size)).ok()?;
        let next = size.checked_add(8 + (size & 1))?;
        if id == name {
            return Some((rest, rest.get(next..).unwrap_or(&[])));
        }
        rest = rest.get(next..)?;
    }
}

/// A little-endian number at `offset`.
fn number(bytes: &[u8], offset: usize) -> Option<usize> {
    let field = bytes.get(offset..offset + 4)?;
    let value = u32::from_le_bytes(field.try_into().ok()?);
    usize::try_from(value).ok()
}

/// A little-endian short at `offset`.
fn short(bytes: &[u8], offset: usize) -> Option<u16> {
    let field = bytes.get(offset..offset + 2)?;
    Some(u16::from_le_bytes(field.try_into().ok()?))
}

/// The loop start, and the length a `mark` gives, from what follows `fmt `.
fn loop_points(chunks: &[u8]) -> (Option<usize>, Option<usize>) {
    let Some((cue, after)) = find_chunk(chunks, b"cue ") else {
        return (None, None);
    };
    let start = number(cue, 32);
    let marked = find_chunk(after, b"LIST").and_then(|(list, _)| {
        (list.get(28..32) == Some(b"mark"))
            .then(|| number(list, 24))
            .flatten()
    });
    (
        start,
        start.zip(marked).map(|(start, length)| start + length),
    )
}

/// Reads a PCM WAV of 8 or 16 bits, or `None` for anything else. Channels mix
/// down to one. A `cue ` chunk marks where a loop goes back to.
#[must_use]
pub fn parse_wav(bytes: &[u8]) -> Option<Clip> {
    if bytes.get(..4)? != b"RIFF" || bytes.get(8..12)? != b"WAVE" {
        return None;
    }
    let chunks = bytes.get(12..)?;
    let (format, _) = find_chunk(chunks, b"fmt ")?;
    if short(format, 8)? != 1 {
        return None;
    }
    let channels = usize::from(short(format, 10)?);
    let sample_rate = u32::try_from(number(format, 12)?).ok()?;
    let width = usize::from(short(format, 22)? / 8);
    if channels == 0 || sample_rate == 0 || !matches!(width, 1 | 2) {
        return None;
    }
    let (loop_start, marked_length) = loop_points(chunks);
    let (data, _) = find_chunk(chunks, b"data")?;
    let size = number(data, 4)?;
    let data = data.get(8..)?;
    let data = data.get(..size.min(data.len()))?;
    let frames = data.len() / (width * channels);
    let length = marked_length.unwrap_or(frames);
    if length > frames {
        return None;
    }
    let scale = 1.0 / channels as f32;
    let samples: Arc<[f32]> = data
        .chunks_exact(width * channels)
        .take(length)
        .map(|frame| {
            let sum: f32 = frame
                .chunks_exact(width)
                .map(|sample| match *sample {
                    [byte] => (f32::from(byte) - 128.0) / 128.0,
                    [low, high] => f32::from(i16::from_le_bytes([low, high])) / 32768.0,
                    _ => 0.0,
                })
                .sum();
            sum * scale
        })
        .collect();
    let loop_start = loop_start.filter(|start| *start < samples.len());
    Some(Clip {
        sample_rate,
        samples,
        loop_start,
    })
}
