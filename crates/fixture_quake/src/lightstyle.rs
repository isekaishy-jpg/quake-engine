//! Light style animation, from `world.qc`, until the QuakeC port owns it.

/// How many styles the renderer takes.
pub const LIGHT_STYLE_COUNT: usize = 64;

/// The patterns set in `worldspawn`. Letters run from `a` for dark to `z` for
/// bright, with `m` as normal, at ten letters a second.
const PATTERNS: [&str; 12] = [
    "m",
    "mmnmmommommnonmmonqnmmo",
    "abcdefghijklmnopqrstuvwxyzyxwvutsrqponmlkjihgfedcba",
    "mmmmmaaaaammmmmaaaaaabcdefgabcdefg",
    "mamamamamama",
    "jklmnopqrstuvwxyzyxwvutsrqponmlkj",
    "nmonqnmomnmomomno",
    "mmmaaaabcdefgmmmmaaaammmaamm",
    "mmmaaammmaaammmabcdefaaaammmmabcdefmmmaaaa",
    "aaaaaaaazzzzzzzz",
    "mmamammmmammamamaaamammma",
    "abcdefghijklmnopqrrqponmlkjihgfedcba",
];

/// The brightness of every style at `time`, where 1 is normal.
#[must_use]
pub fn light_styles(time: f64) -> [f32; LIGHT_STYLE_COUNT] {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the step count is small and time is never negative"
    )]
    let step = (time.max(0.0) * 10.0) as usize;
    let mut styles = [1.0; LIGHT_STYLE_COUNT];
    for (style, pattern) in styles.iter_mut().zip(PATTERNS) {
        let bytes = pattern.as_bytes();
        let letter = bytes[step % bytes.len()];
        *style = f32::from(letter - b'a') / f32::from(b'm' - b'a');
    }
    styles
}
