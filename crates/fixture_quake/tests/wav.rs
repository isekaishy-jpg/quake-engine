//! WAV reading, on files built here.

use fixture_quake::parse_wav;

/// A chunk.
fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = id.to_vec();
    out.extend_from_slice(&u32::try_from(body.len()).expect("small").to_le_bytes());
    out.extend_from_slice(body);
    if body.len() % 2 == 1 {
        out.push(0);
    }
    out
}

/// A format chunk.
fn format(channels: u16, rate: u32, bits: u16) -> Vec<u8> {
    let mut body = 1_u16.to_le_bytes().to_vec();
    body.extend_from_slice(&channels.to_le_bytes());
    body.extend_from_slice(&rate.to_le_bytes());
    body.extend_from_slice(&[0; 6]);
    body.extend_from_slice(&bits.to_le_bytes());
    chunk(b"fmt ", &body)
}

/// A cue chunk with a loop start.
fn cue(start: u32) -> Vec<u8> {
    let mut body = vec![0; 24];
    body.extend_from_slice(&start.to_le_bytes());
    chunk(b"cue ", &body)
}

/// A `LIST` chunk with a `mark` of `length` samples.
fn mark(length: u32) -> Vec<u8> {
    let mut body = vec![0; 16];
    body.extend_from_slice(&length.to_le_bytes());
    body.extend_from_slice(b"mark");
    chunk(b"LIST", &body)
}

/// A file of `chunks`.
fn wav(chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut body = b"WAVE".to_vec();
    for chunk in chunks {
        body.extend_from_slice(chunk);
    }
    chunk(b"RIFF", &body)
}

#[test]
fn eight_bit_samples_are_unsigned_around_128() {
    let file = wav(&[format(1, 11025, 8), chunk(b"data", &[0, 128, 192, 255])]);
    let clip = parse_wav(&file).expect("it parses");
    assert_eq!(clip.sample_rate, 11025);
    assert_eq!(clip.samples.len(), 4);
    assert!((clip.samples[0] + 1.0).abs() < f32::EPSILON);
    assert!(clip.samples[1].abs() < f32::EPSILON);
    assert!((clip.samples[2] - 0.5).abs() < f32::EPSILON);
    assert_eq!(clip.loop_start, None);
}

#[test]
fn sixteen_bit_samples_are_signed() {
    let file = wav(&[format(1, 22050, 16), chunk(b"data", &[0, 0x40, 0, 0xC0])]);
    let clip = parse_wav(&file).expect("it parses");
    assert_eq!(clip.samples.len(), 2);
    assert!((clip.samples[0] - 0.5).abs() < f32::EPSILON);
    assert!((clip.samples[1] + 0.5).abs() < f32::EPSILON);
}

#[test]
fn stereo_mixes_down() {
    let file = wav(&[format(2, 11025, 8), chunk(b"data", &[192, 128, 64, 64])]);
    let clip = parse_wav(&file).expect("it parses");
    assert_eq!(clip.samples.len(), 2);
    assert!((clip.samples[0] - 0.25).abs() < f32::EPSILON);
    assert!((clip.samples[1] + 0.5).abs() < f32::EPSILON);
}

#[test]
fn a_cue_chunk_marks_the_loop() {
    let file = wav(&[format(1, 11025, 8), cue(2), chunk(b"data", &[128; 6])]);
    assert_eq!(parse_wav(&file).expect("it parses").loop_start, Some(2));
}

#[test]
fn a_mark_ends_the_sound_at_the_end_of_the_loop() {
    let file = wav(&[
        format(1, 11025, 8),
        cue(2),
        mark(3),
        chunk(b"data", &[128; 8]),
    ]);
    let clip = parse_wav(&file).expect("it parses");
    assert_eq!(clip.samples.len(), 5);
    assert_eq!(clip.loop_start, Some(2));
}

#[test]
fn a_mark_past_the_end_is_refused() {
    let file = wav(&[
        format(1, 11025, 8),
        cue(2),
        mark(30),
        chunk(b"data", &[128; 8]),
    ]);
    assert!(parse_wav(&file).is_none());
}

#[test]
fn a_loop_past_the_end_is_ignored() {
    let file = wav(&[format(1, 11025, 8), cue(20), chunk(b"data", &[128; 8])]);
    assert_eq!(parse_wav(&file).expect("it parses").loop_start, None);
}

#[test]
fn a_chunk_of_odd_length_is_padded() {
    let file = wav(&[
        chunk(b"junk", &[1, 2, 3]),
        format(1, 11025, 8),
        chunk(b"data", &[128; 2]),
    ]);
    assert_eq!(parse_wav(&file).expect("it parses").samples.len(), 2);
}

#[test]
fn what_is_not_pcm_is_refused() {
    let data = chunk(b"data", &[128; 4]);
    let mut compressed = format(1, 11025, 8);
    compressed[8] = 2;
    assert!(parse_wav(&wav(&[compressed, data.clone()])).is_none());
    assert!(parse_wav(&wav(&[format(1, 11025, 24), data.clone()])).is_none());
    assert!(parse_wav(&wav(&[format(0, 11025, 8), data.clone()])).is_none());
    assert!(parse_wav(&wav(&[data])).is_none());
    assert!(parse_wav(&wav(&[format(1, 11025, 8)])).is_none());
    assert!(parse_wav(b"nonsense").is_none());
    assert!(parse_wav(b"").is_none());
}

#[test]
fn a_truncated_file_gives_what_it_has() {
    let mut file = wav(&[format(1, 11025, 8), chunk(b"data", &[128; 8])]);
    file.truncate(file.len() - 4);
    assert_eq!(parse_wav(&file).expect("it parses").samples.len(), 4);
}
