//! The colour over the view.

use engine_client::ViewTint;

#[test]
fn nothing_colours_a_clear_view() {
    assert_eq!(ViewTint::default().color()[3], 0.0);
}

#[test]
fn the_colour_of_the_water_is_what_shows() {
    let mut tint = ViewTint::default();
    tint.set_surroundings([0.1, 0.2, 0.3, 0.5]);
    assert_eq!(tint.color(), [0.1, 0.2, 0.3, 0.5]);
}

#[test]
fn being_hurt_shows_and_fades_while_water_stays() {
    let mut tint = ViewTint::default();
    tint.set_surroundings([0.0, 0.0, 1.0, 0.25]);
    let calm = tint.color();
    tint.hurt(0, 40);
    assert!(tint.color()[3] > calm[3]);
    assert!(tint.color()[0] > 0.1);
    tint.pass(10.0);
    assert_eq!(tint.color(), calm);
}

#[test]
fn hurts_add_up_to_a_limit() {
    let mut tint = ViewTint::default();
    for _ in 0..50 {
        tint.hurt(0, 100);
    }
    assert!(tint.color()[3] <= 150.0 / 255.0 + 1e-6);
}
