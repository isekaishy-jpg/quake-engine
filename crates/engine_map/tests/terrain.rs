//! Shaping land (proposal 0144).

use engine_map::{
    TerrainBrush, TerrainData, TerrainLayer, TerrainStroke, copy_land, splat_land, stamp_land,
    stroke_land,
};

/// A chunk of flat land at height 0.
fn flat() -> TerrainData {
    TerrainData::flat([1, 1], 32.0)
}

/// A hard circle of 200 in the middle of the chunk.
const CIRCLE: TerrainBrush = TerrainBrush {
    middle: [1024.0, 1024.0],
    radius: 200.0,
    soft: 0.0,
};

#[test]
fn a_brush_is_full_in_its_middle_and_fades_to_nothing() {
    let brush = TerrainBrush {
        soft: 0.5,
        ..CIRCLE
    };
    assert_eq!(brush.weight(1024.0, 1024.0), 1.0);
    assert_eq!(brush.weight(1100.0, 1024.0), 1.0, "inside its hard part");
    let fading = brush.weight(1174.0, 1024.0);
    assert!(fading > 0.0 && fading < 1.0, "{fading}");
    assert_eq!(brush.weight(1225.0, 1024.0), 0.0);
}

#[test]
fn raising_lifts_only_what_is_in_the_circle() {
    let mut land = flat();
    stroke_land(&mut land, &CIRCLE, TerrainStroke::Raise(50.0));
    assert_eq!(land.height(32, 32), 50.0);
    assert_eq!(land.height(32, 37), 50.0, "160 from the middle");
    assert_eq!(land.height(32, 40), 0.0, "256 from it");
}

#[test]
fn a_brush_at_the_edge_of_the_land_stays_on_it() {
    let mut land = flat();
    let corner = TerrainBrush {
        middle: [0.0, 2048.0],
        ..CIRCLE
    };
    stroke_land(&mut land, &corner, TerrainStroke::Raise(10.0));
    assert_eq!(land.height(0, 64), 10.0);
    assert_eq!(land.height(0, 64 - 8), 0.0);
}

#[test]
fn flattening_brings_land_to_a_height() {
    let mut land = flat();
    stroke_land(&mut land, &CIRCLE, TerrainStroke::Raise(70.0));
    stroke_land(&mut land, &CIRCLE, TerrainStroke::Flatten(20.0));
    assert_eq!(land.height(32, 32), 20.0);
}

#[test]
fn smoothing_takes_the_edge_off_a_step() {
    let mut land = flat();
    let across = land.across() as usize;
    for y in 0..=64 {
        for x in 32..=64 {
            land.heights[y * across + x] = 90.0;
        }
    }
    let (low, high) = (land.height(31, 32), land.height(32, 32));
    stroke_land(&mut land, &CIRCLE, TerrainStroke::Smooth(1.0));
    assert!(land.height(31, 32) > low, "the low side comes up");
    assert!(land.height(32, 32) < high, "and the high side down");
    assert_eq!(land.height(20, 32), 0.0, "and nothing moves far from it");
}

#[test]
fn a_hole_is_cut_and_filled() {
    let mut land = flat();
    stroke_land(&mut land, &CIRCLE, TerrainStroke::Hole(true));
    assert!(land.is_hole(32, 32));
    assert!(!land.is_hole(20, 32));
    stroke_land(&mut land, &CIRCLE, TerrainStroke::Hole(false));
    assert!(!land.is_hole(32, 32));
}

/// A little hill, in the middle of its own land.
fn hill() -> TerrainData {
    let mut stamp = flat();
    let brush = TerrainBrush {
        soft: 1.0,
        ..CIRCLE
    };
    stroke_land(&mut stamp, &brush, TerrainStroke::Raise(100.0));
    stamp
}

#[test]
fn a_stamp_is_pressed_in_where_the_brush_is() {
    let mut land = flat();
    // The stamp's whole land fits the brush, so what's in its middle comes out in the middle.
    let brush = TerrainBrush {
        middle: [1024.0, 1024.0],
        radius: 1000.0,
        soft: 0.0,
    };
    stamp_land(&mut land, &hill(), &brush, 2.0);
    assert!(
        (land.height(32, 32) - 200.0).abs() < 1.0,
        "{}",
        land.height(32, 32)
    );
    assert_eq!(
        land.height(5, 5),
        0.0,
        "and the stamp's own flat stays flat"
    );
}

#[test]
fn a_scatter_makes_the_same_land_from_the_same_number() {
    let brush = TerrainBrush {
        radius: 900.0,
        ..CIRCLE
    };
    let mut one = flat();
    let mut same = flat();
    let mut other = flat();
    splat_land(&mut one, &hill(), &brush, (20, 1.0, 7));
    splat_land(&mut same, &hill(), &brush, (20, 1.0, 7));
    splat_land(&mut other, &hill(), &brush, (20, 1.0, 8));
    assert_eq!(one, same);
    assert_ne!(one, other);
    assert!(
        one.heights.iter().any(|height| *height > 5.0),
        "it did something"
    );
}

#[test]
fn copying_moves_the_shape_of_one_place_to_another() {
    let mut land = flat();
    let there = TerrainBrush {
        middle: [512.0, 512.0],
        ..CIRCLE
    };
    stroke_land(&mut land, &there, TerrainStroke::Raise(40.0));
    let here = TerrainBrush {
        middle: [1536.0, 1536.0],
        ..CIRCLE
    };
    copy_land(&mut land, [512.0, 512.0], &here);
    assert_eq!(land.height(48, 48), 40.0);
    assert_eq!(land.height(16, 16), 40.0, "and the first stays");
}

/// Land with `count` layers, all of the first.
fn layered(count: usize) -> TerrainData {
    let mut data = flat();
    for number in 0..count {
        let layer = TerrainLayer {
            material: format!("layer{number}"),
            size: 256.0,
            hex: false,
        };
        assert!(data.set_layer(number, layer));
    }
    data
}

#[test]
fn layers_go_in_order_and_stop_at_eight() {
    let mut data = layered(8);
    assert_eq!(data.paint.len(), data.heights.len());
    assert!(data.paint.iter().all(|paint| paint[0] == 255));
    assert!(!data.set_layer(8, TerrainLayer::default()), "one too many");
    let mut data = layered(1);
    assert!(!data.set_layer(3, TerrainLayer::default()), "with a gap");
    assert!(data.set_layer(0, TerrainLayer::default()), "one can change");
    assert_eq!(data.layers.len(), 1);
}

#[test]
fn paint_comes_in_by_what_the_others_give_up_and_always_comes_to_all_of_it() {
    let mut data = layered(4);
    let brush = TerrainBrush {
        middle: [320.0, 320.0],
        radius: 200.0,
        soft: 0.5,
    };
    for (number, share) in [(1, 0.5), (2, 0.3), (3, 0.77), (1, 0.01), (0, 0.33)] {
        stroke_land(&mut data, &brush, TerrainStroke::Paint(number, share));
    }
    for paint in &data.paint {
        let sum: u32 = paint.iter().copied().map(u32::from).sum();
        assert_eq!(sum, 255, "{paint:?}");
    }
    let middle = (10 * data.across() + 10) as usize;
    assert!(data.paint[middle][1..4].iter().all(|part| *part > 0));
    assert_eq!(data.paint[0][0], 255, "and it's only in the circle");
}

#[test]
fn enough_paint_covers_what_was_there() {
    let mut data = layered(2);
    let brush = TerrainBrush {
        middle: [320.0, 320.0],
        radius: 200.0,
        soft: 0.0,
    };
    stroke_land(&mut data, &brush, TerrainStroke::Paint(1, 1.0));
    let middle = (10 * data.across() + 10) as usize;
    assert_eq!(data.paint[middle][..2], [0, 255]);
}

#[test]
fn a_layer_that_is_not_there_paints_nothing() {
    let mut data = layered(2);
    let before = data.paint.clone();
    let brush = TerrainBrush {
        middle: [320.0, 320.0],
        radius: 200.0,
        soft: 0.0,
    };
    stroke_land(&mut data, &brush, TerrainStroke::Paint(9, 1.0));
    assert_eq!(data.paint, before);
}
