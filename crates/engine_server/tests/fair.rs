//! The fair play zone, against the tables in the networking record.

use engine_server::{FairSettings, ViewSlack, Zone};

/// The zone for `pings`.
fn zone(pings: &[f64]) -> Zone {
    Zone::of(pings, &FairSettings::default())
}

/// Whether two numbers agree to within rounding.
fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1.0
}

#[test]
fn a_mixed_lobby_leaves_out_the_outlier() {
    let zone = zone(&[8.0, 18.0, 25.0, 35.0, 40.0, 48.0, 60.0, 140.0]);
    assert!(near(zone.top, 53.0), "{zone:?}");
    assert!(near(zone.added_delay(8.0), 12.0));
    assert!(near(zone.added_delay(18.0), 2.0));
    assert_eq!(zone.added_delay(35.0), 0.0);
}

#[test]
fn a_host_is_brought_up_to_the_floor() {
    let zone = zone(&[0.0, 25.0, 50.0, 120.0]);
    assert_eq!(zone.added_delay(0.0), 20.0);
}

#[test]
fn faking_a_high_ping_doesnt_move_the_zone() {
    let zone = zone(&[10.0, 200.0]);
    assert!(near(zone.top, 30.0), "{zone:?}");
    assert_eq!(zone.added_delay(10.0), 10.0);
}

#[test]
fn a_lobby_thats_really_high_is_treated_normally_among_itself() {
    let rules = FairSettings::default();
    let zone = Zone::of(&[90.0, 110.0, 120.0, 130.0], &rules);
    assert!(near(zone.top, 132.0), "{zone:?}");
    assert_eq!(zone.ceiling, zone.top, "the ceiling rises");
    assert_eq!(zone.added_delay(90.0), 0.0);
    assert_eq!(zone.compensated(130.0, false, &rules), (130.0, false));
}

#[test]
fn a_lan_game_slows_nobody() {
    let zone = zone(&[1.0, 2.0, 3.0]);
    assert_eq!(zone.floor, 3.0);
    assert_eq!(zone.added_delay(1.0), 2.0, "only up to the highest present");
    let alone = self::zone(&[0.0]);
    assert_eq!(alone.added_delay(0.0), 0.0, "one player is never slowed");
}

#[test]
fn compensation_follows_the_table() {
    let rules = FairSettings::default();
    let zone = Zone {
        floor: 20.0,
        top: 60.0,
        ceiling: 90.0,
    };
    assert_eq!(zone.compensated(60.0, false, &rules).0, 60.0);
    assert_eq!(zone.compensated(76.0, false, &rules).0, 68.0);
    assert_eq!(zone.compensated(90.0, false, &rules).0, 75.0);
    assert_eq!(zone.compensated(100.0, false, &rules), (0.0, true));
}

#[test]
fn the_ceiling_doesnt_flicker() {
    let rules = FairSettings::default();
    let zone = Zone {
        floor: 20.0,
        top: 60.0,
        ceiling: 90.0,
    };
    assert!(zone.compensated(91.0, false, &rules).1, "over it");
    assert!(zone.compensated(87.0, true, &rules).1, "still off");
    assert!(
        !zone.compensated(84.0, true, &rules).1,
        "back under the margin"
    );
}

#[test]
fn settings_come_from_the_level() {
    let rules = FairSettings::from_settings(&[
        ("sv_fair_floor".to_owned(), 0.0),
        ("sv_unlag_enable".to_owned(), 0.0),
    ]);
    assert_eq!(rules.floor, 0.0);
    assert!(!rules.unlag);
    assert_eq!(rules.ceiling, 90.0);
}

/// A tick, as the tests here count it.
const TICK: f64 = 1.0 / 72.0;

/// The slack after they've said they had a snapshot every `gap` seconds, ten times.
fn slack_after_gaps(gap: f64) -> ViewSlack {
    let mut slack = ViewSlack::default();
    for step in 0..10 {
        slack.heard(f64::from(step) * gap, TICK);
    }
    slack
}

/// What a client says about its view isn't what sets how far a shot can reach.
#[test]
fn a_clean_connection_only_reaches_a_little_past_its_ping() {
    let steady = slack_after_gaps(TICK).seconds();
    assert!(steady < 0.05, "{steady}");
}

#[test]
fn a_connection_that_really_stalls_reaches_further_for_a_while() {
    let steady = slack_after_gaps(TICK).seconds();
    let mut slack = slack_after_gaps(TICK);
    slack.heard(10.0 * TICK + 0.05, TICK);
    let stalled = slack.seconds();
    assert!(stalled > steady + 0.03, "{stalled} from {steady}");
    slack.pass(60.0);
    assert!(slack.seconds() < steady + 0.001, "{}", slack.seconds());
}

#[test]
fn no_stall_reaches_past_the_furthest_a_view_can_be() {
    let mut slack = slack_after_gaps(TICK);
    slack.heard(100.0, TICK);
    assert!(slack.seconds() < 0.12, "{}", slack.seconds());
}
