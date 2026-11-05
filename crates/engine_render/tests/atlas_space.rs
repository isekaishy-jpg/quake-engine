//! Handing out squares of the atlas.

use engine_render::AtlasSpace;

/// Nothing overlaps what's reserved, or anything else handed out.
#[test]
fn squares_do_not_overlap() {
    let mut space = AtlasSpace::new(1024, 256);
    let mut taken = Vec::new();
    while let Some(at) = space.take(128) {
        taken.push(at);
    }
    // 64 squares fit, and the reserved corner holds 4 of them.
    assert_eq!(taken.len(), 60);
    for (index, a) in taken.iter().enumerate() {
        assert!(a[0] >= 256 || a[1] >= 256, "in the reserved corner: {a:?}");
        assert!(a[0] + 128 <= 1024 && a[1] + 128 <= 1024, "{a:?}");
        for b in &taken[index + 1..] {
            assert_ne!(a, b);
        }
    }
}

/// Small squares given back join up, so a big one fits again.
#[test]
fn freed_squares_join_up() {
    let mut space = AtlasSpace::new(1024, 256);
    let first = space.take(512).expect("room");
    let second = space.take(512).expect("room");
    let third = space.take(512).expect("room");
    assert!(space.take(512).is_none(), "the corner holds the last");
    space.give(third, 512);
    let mut parts = Vec::new();
    for _ in 0..4 {
        parts.push(space.take(256).expect("room"));
    }
    assert!(space.take(512).is_none());
    for part in parts {
        space.give(part, 256);
    }
    assert_eq!(space.take(512), Some(third));
    space.give(first, 512);
    space.give(second, 512);
    assert!(space.take(512).is_some());
}

/// Asking for too much is only a no.
#[test]
fn a_square_too_big_is_refused() {
    let mut space = AtlasSpace::new(512, 128);
    assert!(space.take(512).is_none());
    assert!(space.take(256).is_some());
}
