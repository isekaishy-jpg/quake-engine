//! What live lights bounce, at the probes (proposal 0153).

use engine_core::math::Vec3;
use engine_map::{Contents, Leaf, Map, Node, NodeChild, Plane, ProbeGrid, Tree};
use engine_render::{LIVE_BOUNCE_GAIN, LiveLight, bounce_of_light};

/// Open space with a wall across it from x = 100 to 116, as high as z = 200 and open above, or
/// all the way up if `high`.
fn walled(high: bool) -> Map {
    let plane = |normal: Vec3, distance: f32| Plane { normal, distance };
    let mut tree = Tree {
        planes: vec![
            plane(Vec3::new(0.0, 0.0, 1.0), 200.0),
            plane(Vec3::new(1.0, 0.0, 0.0), 100.0),
            plane(Vec3::new(1.0, 0.0, 0.0), 116.0),
        ],
        nodes: vec![
            Node {
                plane: 0,
                children: [NodeChild::Leaf(1), NodeChild::Node(1)],
            },
            Node {
                plane: 1,
                children: [NodeChild::Node(2), NodeChild::Leaf(1)],
            },
            Node {
                plane: 2,
                children: [NodeChild::Leaf(3), NodeChild::Leaf(2)],
            },
        ],
        leaves: vec![Leaf::default(); 4],
        ..Tree::default()
    };
    tree.leaves[0].contents = Contents::Solid;
    tree.leaves[2].contents = Contents::Solid;
    if high {
        tree.nodes[0] = tree.nodes[1];
    }
    Map {
        tree,
        ..Map::default()
    }
}

/// Probes 50 apart from (-300, -300, 0), 13 by 13 by 2.
fn probes() -> ProbeGrid {
    let count = 13 * 13 * 2;
    ProbeGrid {
        origin: Vec3::new(-300.0, -300.0, 0.0),
        spacing: 50.0,
        size: [13, 13, 2],
        colors: vec![[[0; 3]; 6]; count],
        directions: vec![[0; 3]; count],
        sun_shares: vec![0; count],
    }
}

/// A white light at (0, 0, 50) reaching 600, which bounces `bounce`.
fn lamp(bounce: f32) -> LiveLight {
    LiveLight {
        origin: Vec3::new(0.0, 0.0, 50.0),
        radius: 600.0,
        color: [1.0; 3],
        bounce,
        ..LiveLight::default()
    }
}

/// What the probe at `x` along the middle row, at z = 50, gets.
fn at(bounced: &[(u32, [f32; 3])], x: f32) -> f32 {
    let probes = probes();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "on the grid"
    )]
    let column = ((x - probes.origin.x) / probes.spacing) as u32;
    let index = (13 + 6) * 13 + column;
    bounced
        .iter()
        .find(|(at, _)| *at == index)
        .map_or(0.0, |(_, light)| light[0])
}

/// What's on a light's side of a wall takes what it bounces, and what's behind a wall it can't get
/// round takes none, so its light doesn't go through.
#[test]
fn a_light_bounces_in_its_room_and_not_through_a_wall() {
    let bounced = bounce_of_light(&walled(true), &probes(), &lamp(1.0));
    let near = at(&bounced, 50.0);
    assert!(near > 0.1, "{near}");
    assert!(at(&bounced, -200.0) > 0.0, "the other way too");
    assert_eq!(at(&bounced, 200.0), 0.0, "behind the wall");
}

/// What's in the shadow of a wall it can see over takes some, as light off the room round it
/// reaches it, but less than what sees all the room.
#[test]
fn a_light_bounces_into_its_own_shadow() {
    let bounced = bounce_of_light(&walled(false), &probes(), &lamp(1.0));
    // Far enough back to see over it, to what's lit above.
    let shaded = at(&bounced, 250.0);
    let open = at(&bounced, -250.0);
    assert!(shaded > 0.0, "{shaded}");
    assert!(shaded < open / 2.0, "{shaded} {open}");
}

/// It falls off as a live light's own light does, and goes as its colour and how much it bounces,
/// and with none, or nowhere, there's none.
#[test]
fn bounce_goes_by_the_light() {
    let open = Map::default();
    let full = bounce_of_light(&open, &probes(), &lamp(1.0));
    let near = at(&full, 0.0);
    let far = at(&full, 250.0);
    assert!(far < near, "{far} {near}");
    // A probe at 50 from it has 1 - 1/144 of its light left to fall.
    let fall: f32 = 1.0 - 50.0 * 50.0 / (600.0 * 600.0);
    assert!(
        (near - fall * fall * LIVE_BOUNCE_GAIN).abs() < 0.01,
        "{near}"
    );
    let half = bounce_of_light(&open, &probes(), &lamp(0.5));
    assert!((at(&half, 0.0) - near * 0.5).abs() < 1e-4);
    let red = LiveLight {
        color: [1.0, 0.0, 0.0],
        ..lamp(1.0)
    };
    let tinted = bounce_of_light(&open, &probes(), &red);
    let (_, colour) = tinted[0];
    assert_eq!(colour[1], 0.0, "its own colour");
    assert!(bounce_of_light(&open, &probes(), &lamp(0.0)).is_empty());
    let far_off = LiveLight {
        origin: Vec3::new(5000.0, 0.0, 50.0),
        ..lamp(1.0)
    };
    assert!(
        bounce_of_light(&open, &probes(), &far_off).is_empty(),
        "out of reach"
    );
}
