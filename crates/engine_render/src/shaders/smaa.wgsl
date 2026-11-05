// SMAA 1x: find edges, then work out from each edge's shape how much each side
// takes from the other (proposal 0093). The final pass does the blending.
//
// The original looks areas up in a table, to save working them out. Here they're
// worked out, as a table of them is only that sum done in advance. There's no
// diagonal or corner pass, as in its low and medium presets.

@group(0) @binding(0) var source: texture_2d<f32>;

// How much a neighbour has to differ to be an edge, on a scale of 0 to 1.
const THRESHOLD: f32 = 0.1;

// How much stronger a nearby edge has to be to hide this one.
const LOCAL_CONTRAST: f32 = 2.0;

// How many pixels an edge is followed each way.
const MAX_SEARCH: i32 = 32;

// How long a rounded end has to be before it's a plain slope.
const SMOOTH_LENGTH: f32 = 32.0;

@vertex
fn vs_full(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - vec2<f32>(1.0), 0.0, 1.0);
}

fn clamped(at: vec2<i32>) -> vec2<i32> {
    return clamp(at, vec2<i32>(0), vec2<i32>(textureDimensions(source)) - vec2<i32>(1));
}

fn luma_at(at: vec2<i32>) -> f32 {
    let color = textureLoad(source, clamped(at), 0).rgb;
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// The red channel says there's an edge on a pixel's left, and green on its top.
@fragment
fn fs_edges(@builtin(position) place: vec4<f32>) -> @location(0) vec4<f32> {
    let at = vec2<i32>(place.xy);
    let middle = luma_at(at);
    let left = abs(middle - luma_at(at + vec2<i32>(-1, 0)));
    let top = abs(middle - luma_at(at + vec2<i32>(0, -1)));
    var edges = step(vec2<f32>(THRESHOLD), vec2<f32>(left, top));
    if edges.x + edges.y == 0.0 {
        return vec4<f32>(0.0);
    }
    // A faint edge beside a strong one is left alone, so a bright line doesn't
    // pull its faint neighbours along.
    let right = abs(middle - luma_at(at + vec2<i32>(1, 0)));
    let bottom = abs(middle - luma_at(at + vec2<i32>(0, 1)));
    let further_left = abs(luma_at(at + vec2<i32>(-1, 0)) - luma_at(at + vec2<i32>(-2, 0)));
    let further_top = abs(luma_at(at + vec2<i32>(0, -1)) - luma_at(at + vec2<i32>(0, -2)));
    let strongest = max(max(max(left, top), max(right, bottom)), max(further_left, further_top));
    edges = edges * step(vec2<f32>(strongest), LOCAL_CONTRAST * vec2<f32>(left, top));
    return vec4<f32>(edges, 0.0, 1.0);
}

// Whether a pixel has the edge a line is made of: a
// top edge for a line across, and a left edge for one down.
fn runs(at: vec2<i32>, across: bool) -> bool {
    let edges = textureLoad(source, clamped(at), 0).rg;
    if across {
        return edges.g > 0.5;
    }
    return edges.r > 0.5;
}

// How the end of a line turns at `at`, the first pixel past it. +0.5 turns away
// from this pixel, towards the neighbour the line separates it from, -0.5 turns
// towards it, 2.0 is both ways, and 0.0 is neither.
fn turn_at(at: vec2<i32>, side: vec2<i32>, across: bool) -> f32 {
    let toward = runs(at, !across);
    let away = runs(at + side, !across);
    if toward && away {
        return 2.0;
    }
    if away {
        return 0.5;
    }
    if toward {
        return -0.5;
    }
    return 0.0;
}

// What part of a line from (x1, y1) to (x2, y2) lies over the pixel from `x` to
// `x + 1`, above zero in x and below it in y.
fn slice_of_line(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> vec2<f32> {
    let from_x = max(x, x1);
    let to_x = min(x + 1.0, x2);
    if to_x <= from_x || x2 <= x1 {
        return vec2<f32>(0.0);
    }
    let slope = (y2 - y1) / (x2 - x1);
    let a = y1 + slope * (from_x - x1);
    let b = y1 + slope * (to_x - x1);
    let width = to_x - from_x;
    if a * b >= 0.0 {
        let area = (a + b) * 0.5 * width;
        return vec2<f32>(max(area, 0.0), max(-area, 0.0));
    }
    // It crosses zero inside the pixel, so each side is a triangle.
    let crossing = width * abs(a) / (abs(a) + abs(b));
    let first = a * crossing * 0.5;
    let second = b * (width - crossing) * 0.5;
    return vec2<f32>(max(first, 0.0) + max(second, 0.0), max(-first, 0.0) + max(-second, 0.0));
}

// How much a pixel `before` from one end of a line, and `after` from the other,
// is covered, from how the ends turn. Above zero is on the neighbour's side.
fn covered(before: f32, after: f32, turn_a: f32, turn_b: f32) -> vec2<f32> {
    var a = turn_a;
    var b = turn_b;
    // A crossing has no way of its own, so it goes as the far end makes it.
    if a > 1.0 {
        a = select(0.0, -b, abs(b) < 1.0);
    }
    if b > 1.0 {
        b = select(0.0, -turn_a, abs(turn_a) < 1.0);
    }
    let span = before + after + 1.0;
    let half_way = span * 0.5;
    if a == 0.0 && b == 0.0 {
        return vec2<f32>(0.0);
    }
    if a != 0.0 && b != 0.0 && a != b {
        return slice_of_line(0.0, a, span, b, before);
    }
    var total = vec2<f32>(0.0);
    if a != 0.0 && before <= after {
        total = total + slice_of_line(0.0, a, half_way, 0.0, before);
    }
    if b != 0.0 && before >= after {
        total = total + slice_of_line(half_way, 0.0, span, b, before);
    }
    if a != 0.0 && b != 0.0 {
        // Both ends turn the same way. Short ones round off, as they'd
        // otherwise leave a notch.
        let rounded = sqrt(total * 2.0) * 0.5;
        total = mix(rounded, total, clamp(span / SMOOTH_LENGTH, 0.0, 1.0));
    }
    return total;
}

// The shares of a line through `at`: `along` runs along it, and `side` goes to
// the neighbour it separates this pixel from.
fn line_shares(at: vec2<i32>, along: vec2<i32>, side: vec2<i32>, across: bool) -> vec2<f32> {
    var before = 0;
    var after = 0;
    var from_a = at;
    var from_b = at;
    for (var count = 0; count < MAX_SEARCH; count = count + 1) {
        let next = from_a - along;
        if !runs(next, across) || runs(from_a, !across) || runs(from_a + side, !across) {
            break;
        }
        from_a = next;
        before = before + 1;
    }
    for (var count = 0; count < MAX_SEARCH; count = count + 1) {
        let next = from_b + along;
        if !runs(next, across) || runs(next, !across) || runs(next + side, !across) {
            break;
        }
        from_b = next;
        after = after + 1;
    }
    let turn_a = turn_at(from_a, side, across);
    let turn_b = turn_at(from_b + along, side, across);
    return covered(f32(before), f32(after), turn_a, turn_b);
}

// For a top edge, red is how much this pixel takes from above, and green how
// much the one above takes from it. Blue and alpha are the same for a left edge.
@fragment
fn fs_weights(@builtin(position) place: vec4<f32>) -> @location(0) vec4<f32> {
    let at = vec2<i32>(place.xy);
    let edges = textureLoad(source, at, 0).rg;
    var weights = vec4<f32>(0.0);
    if edges.g > 0.5 {
        let shares = line_shares(at, vec2<i32>(1, 0), vec2<i32>(0, -1), true);
        weights = vec4<f32>(shares.y, shares.x, weights.z, weights.w);
    }
    if edges.r > 0.5 {
        let shares = line_shares(at, vec2<i32>(0, 1), vec2<i32>(-1, 0), false);
        weights = vec4<f32>(weights.x, weights.y, shares.y, shares.x);
    }
    return weights;
}
