//! Pictures against the texture standard.

use engine_compile::{Repeat, check_picture};

/// A picture of smooth waves that meet at their edges, with `change` run over it.
fn tiling(size: u32, change: impl Fn(u32, u32, u8) -> u8) -> Vec<u8> {
    let mut rgba = Vec::new();
    for y in 0..size {
        for x in 0..size {
            let wave =
                |at: u32| (f64::from(at) / f64::from(size) * std::f64::consts::TAU * 3.0).sin();
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a byte"
            )]
            let value = (128.0 + 40.0 * wave(x) + 40.0 * wave(y)) as u8;
            let value = change(x, y, value);
            rgba.extend([value, value, value, 255]);
        }
    }
    rgba
}

/// What's made to the standard passes.
#[test]
fn a_tiling_picture_passes() {
    let rgba = tiling(256, |_, _, value| value);
    assert_eq!(
        check_picture(256, 256, &rgba, Repeat::Both),
        Vec::<String>::new()
    );
}

/// A dark line along the edges is found, as the first packs had.
#[test]
fn a_border_is_found() {
    let rgba = tiling(256, |x, y, value| {
        if x >= 254 || y >= 254 {
            value / 2
        } else {
            value
        }
    });
    let problems = check_picture(256, 256, &rgba, Repeat::Both).join(" ");
    assert!(problems.contains("border down its sides"), "{problems}");
    assert!(problems.contains("border along its top"), "{problems}");
}

/// A picture whose sides are in different tones doesn't meet itself.
#[test]
fn edges_that_dont_meet_are_found() {
    let rgba = tiling(256, |x, _, value| {
        value.saturating_add(u8::try_from(x / 4).unwrap_or(0))
    });
    let problems = check_picture(256, 256, &rgba, Repeat::Across).join(" ");
    assert!(problems.contains("sides don't meet"), "{problems}");
}

/// What's shown whole can do what it likes at its edges, but not be any size.
#[test]
fn a_picture_shown_whole_is_only_checked_for_size() {
    let rgba = tiling(232, |x, _, value| if x > 228 { 0 } else { value });
    let problems = check_picture(232, 232, &rgba, Repeat::None);
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert!(problems[0].contains("power of two"));
}
