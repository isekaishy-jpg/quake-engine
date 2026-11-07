//! Morphs are kept as half floats.

use engine_render::half_bits;

/// What `bits` stand for, as the shader reads them.
fn value_of(bits: u16) -> f32 {
    let sign = if bits & 0x8000 == 0 { 1.0 } else { -1.0 };
    let exponent = i32::from((bits >> 10) & 31);
    let fraction = f32::from(bits & 0x3ff) / 1024.0;
    if exponent == 0 {
        return sign * fraction * 2.0_f32.powi(-14);
    }
    sign * (1.0 + fraction) * 2.0_f32.powi(exponent - 15)
}

#[test]
fn known_values_have_known_bits() {
    assert_eq!(half_bits(0.0), 0);
    assert_eq!(half_bits(1.0), 0x3c00);
    assert_eq!(half_bits(-2.0), 0xc000);
    assert_eq!(half_bits(0.5), 0x3800);
    assert_eq!(half_bits(65504.0), 0x7bff);
}

/// Good to about a part in two thousand, across what a morph can be.
#[test]
fn values_come_back_close() {
    for value in [0.001_f32, 0.1, 0.33, 1.7, 11.2, 96.0, 500.0, -3.25, -0.02] {
        let back = value_of(half_bits(value));
        assert!(
            (back - value).abs() <= value.abs() / 1024.0,
            "{value} came back {back}"
        );
    }
}

/// Too big for a half stays as big as there is, and doesn't turn into infinity or wrap.
#[test]
fn what_is_too_big_stops_at_the_largest() {
    assert_eq!(half_bits(1.0e6), 0x7bff);
    assert_eq!(half_bits(-1.0e6), 0xfbff);
}

/// Too small to have an exponent is still kept, down to where it's nothing.
#[test]
fn what_is_tiny_fades_to_nothing() {
    let back = value_of(half_bits(0.00003));
    assert!((back - 0.00003).abs() < 0.000_001, "{back}");
    assert_eq!(half_bits(1.0e-9), 0);
    assert_eq!(half_bits(-1.0e-9), 0x8000);
}

/// Rounding up out of the fraction carries into the exponent.
#[test]
fn rounding_carries_into_the_exponent() {
    assert_eq!(half_bits(1.999_9), 0x4000);
}
