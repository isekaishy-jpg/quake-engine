//! Asking whether light gets along a line.

use std::rc::Rc;
use std::sync::Arc;

use engine_client::Client;
use engine_core::math::Vec3;
use engine_map::{Contents, Map};
use engine_net::loopback;
use engine_physics::CollisionWorld;
use engine_test_maps::room;

/// Where the player stands.
const START: Vec3 = Vec3::new(0.0, 0.0, -200.0);

/// A client in `map`.
fn client_in(map: Arc<Map>) -> Client {
    let world = Rc::new(CollisionWorld::new(map));
    let (client_end, _server_end) = loopback();
    Client::new(Box::new(client_end), world, START)
}

/// The room, with sky under its floor.
fn room_over_sky() -> Arc<Map> {
    let mut map = (*room()).clone();
    map.tree.leaves[0].contents = Contents::Sky;
    Arc::new(map)
}

/// Open air is clear.
#[test]
fn open_air_is_clear() {
    let client = client_in(room());
    let (from, to) = (Vec3::new(-100.0, 100.0, 0.0), Vec3::new(100.0, 100.0, 0.0));
    assert!(client.line_is_clear(from, to, false));
}

/// A wall stops a line.
#[test]
fn a_wall_stops_a_line() {
    let client = client_in(room());
    let (from, to) = (Vec3::new(0.0, 100.0, 0.0), Vec3::new(400.0, 100.0, 0.0));
    assert!(!client.line_is_clear(from, to, false));
    assert!(!client.line_is_clear(from, to, true), "a wall isn't sky");
}

/// The player's own body is in the way of the light, as anyone else's is.
#[test]
fn a_body_stops_a_line() {
    let client = client_in(room());
    let (from, to) = (
        Vec3::new(-100.0, 0.0, -200.0),
        Vec3::new(100.0, 0.0, -200.0),
    );
    assert!(!client.line_is_clear(from, to, false));
}

/// With nothing in the way, light gets all the way along.
#[test]
fn light_reaches_the_end_of_open_air() {
    let client = client_in(room());
    let (from, to) = (Vec3::new(-100.0, 100.0, 0.0), Vec3::new(100.0, 100.0, 0.0));
    assert!((client.light_reach(from, to) - 200.0).abs() < 0.5);
}

/// A wall stops light where it is.
#[test]
fn light_stops_at_a_wall() {
    let client = client_in(room());
    let to = Vec3::new(4000.0, 100.0, 0.0);
    let reach = client.light_reach(Vec3::new(0.0, 100.0, 0.0), to);
    let clear = client.light_reach(
        Vec3::new(0.0, 100.0, 0.0),
        Vec3::new(reach - 1.0, 100.0, 0.0),
    );
    assert!(reach > 0.0 && reach < 4000.0, "{reach}");
    assert!((clear - (reach - 1.0)).abs() < 0.5, "{clear} {reach}");
}

/// Light that starts in a wall comes out of it, if it's thin.
#[test]
fn light_comes_out_of_what_it_starts_in() {
    let client = client_in(room());
    let reach = client.light_reach(Vec3::new(0.0, 100.0, 0.0), Vec3::new(4000.0, 100.0, 0.0));
    // Just inside the wall, going back into the room.
    let from = Vec3::new(reach + 8.0, 100.0, 0.0);
    let back = client.light_reach(from, Vec3::new(0.0, 100.0, 0.0));
    assert!((back - (reach + 8.0)).abs() < 0.5, "{back} {reach}");
}

/// Light that starts deep in something stays there.
#[test]
fn light_stays_in_what_is_too_thick() {
    let client = client_in(room());
    let reach = client.light_reach(Vec3::new(0.0, 100.0, 0.0), Vec3::new(4000.0, 100.0, 0.0));
    // There's open room past all of this, but it's too far in to get to.
    let from = Vec3::new(reach + 100.0, 100.0, 0.0);
    let to = Vec3::new(0.0, 100.0, 0.0);
    assert_eq!(client.light_reach(from, to), 0.0);
}

/// In the open, the sun gets down to the ground, whatever height it's asked about.
#[test]
fn the_sun_gets_to_the_ground_in_the_open() {
    let client = client_in(room_over_sky());
    // Upside down, as the room's sky is under its floor, so "up" is down here.
    let down = Vec3::new(0.0, 0.0, -1.0);
    let place = Vec3::new(-100.0, 100.0, 0.0);
    let ceiling = client.light_reach(place, Vec3::new(-100.0, 100.0, 4000.0));
    let gets_to = client.sun_gets_to(place, down);
    assert!((gets_to - ceiling).abs() < 1.0, "{gets_to} {ceiling}");
}

/// A roof keeps the sun off, so it gets no lower than the roof.
#[test]
fn a_roof_stops_the_sun() {
    let client = client_in(room());
    let up = Vec3::new(0.0, 0.0, 1.0);
    let place = Vec3::new(-100.0, 100.0, 0.0);
    let roof = client.light_reach(place, Vec3::new(-100.0, 100.0, 4000.0));
    assert!((client.sun_gets_to(place, up) - roof).abs() < 1.0);
}

/// A slanting sun is stopped by what's along its line, not what's straight up.
#[test]
fn the_sun_goes_along_its_own_line() {
    let client = client_in(room_over_sky());
    let place = Vec3::new(-100.0, 100.0, 0.0);
    let slant = Vec3::new(0.6, 0.0, -0.8);
    let lands = client.light_reach(place, place - slant * 4000.0);
    let expected = place.z + 0.8 * lands;
    let gets_to = client.sun_gets_to(place, slant);
    assert!((gets_to - expected).abs() < 1.0, "{gets_to} {expected}");
}

/// A place in something solid has only what's above it to go on.
#[test]
fn the_sun_stops_at_what_it_starts_in() {
    let client = client_in(room());
    let place = Vec3::new(9000.0, 100.0, 40.0);
    assert!((client.sun_gets_to(place, Vec3::new(0.0, 0.0, 1.0)) - 40.0).abs() < 0.01);
}

/// Sky is solid, but light comes from it.
#[test]
fn sky_is_clear_only_when_asked() {
    let client = client_in(room_over_sky());
    let (from, to) = (
        Vec3::new(100.0, 100.0, 0.0),
        Vec3::new(100.0, 100.0, -400.0),
    );
    assert!(client.line_is_clear(from, to, true));
    assert!(!client.line_is_clear(from, to, false));
}

/// A line from inside a wall has nothing to say, so it counts as clear: a
/// stencil may sit in its own window frame.
#[test]
fn a_line_from_inside_solid_is_clear() {
    let client = client_in(room());
    let (from, to) = (Vec3::new(300.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0));
    assert!(client.line_is_clear(from, to, false));
}
