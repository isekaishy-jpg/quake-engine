//! Changing an effect by what the node view does (proposal 0146).

use engine_compile::read_effect;
use engine_editor::effect_edit::{
    TIMELINE, add_emitter, apply, blank, emitter_blocks, emitter_mut, emitter_of, remove_emitter,
    rename, set_setting, set_timeline, setting, timeline_of, view_of,
};
use engine_editor::graph_view::{ViewEvent, ViewWire};
use engine_format::{Document, Value, from_text, to_text};

/// An effect with two emitters, the second started when one of the first ends.
fn two() -> Document {
    from_text(
        "format 1 effect
emitter sparks { rate = 10 on_death = puff at = 300 80 }
emitter puff { burst = 3 }
",
    )
    .expect("reads")
}

/// A curve from `from` to row `input` of `to`.
fn joined(from: usize, to: usize, input: usize) -> ViewEvent {
    ViewEvent::Joined(ViewWire { from, to, input })
}

#[test]
fn a_new_effect_reads_and_has_an_emitter() {
    let document = blank();
    assert_eq!(emitter_blocks(&document).len(), 1);
    let effect = read_effect("new", &document).expect("reads");
    assert_eq!(effect.emitters[0].rate, 40.0);
    assert_eq!(effect.emitters[0].name, "emitter1");
}

#[test]
fn emitters_get_names_of_their_own() {
    let mut document = blank();
    assert_eq!(add_emitter(&mut document, [0.0, 0.0]), "emitter2");
    assert_eq!(add_emitter(&mut document, [0.0, 0.0]), "emitter3");
    remove_emitter(&mut document, 1);
    assert_eq!(add_emitter(&mut document, [0.0, 0.0]), "emitter1");
}

#[test]
fn the_view_shows_the_timeline_and_what_starts_what() {
    let mut document = two();
    assert!(apply(&mut document, joined(TIMELINE, 2, 2)));
    let (nodes, wires) = view_of(&document);
    let titles: Vec<&str> = nodes.iter().map(|node| node.title.as_str()).collect();
    assert_eq!(titles, ["timeline", "sparks", "puff"]);
    assert_eq!(nodes[1].at, [300.0, 80.0]);
    assert_ne!(nodes[1].at, nodes[2].at);
    assert_eq!(wires.len(), 2);
    assert!(wires.contains(&ViewWire {
        from: 1,
        to: 2,
        input: 0
    }));
    assert!(wires.contains(&ViewWire {
        from: TIMELINE,
        to: 2,
        input: 2
    }));
}

#[test]
fn a_curve_says_when_one_starts_another() {
    let mut document = two();
    assert!(!apply(&mut document, joined(2, 1, 1)), "that goes round");
    let mut document = from_text("format 1 effect\nemitter a { }\nemitter b { }\n").expect("reads");
    assert!(apply(&mut document, joined(1, 2, 0)));
    assert!(!apply(&mut document, joined(2, 1, 1)), "nor does this");
    assert!(!apply(&mut document, joined(1, 1, 0)), "or itself");
    let a = emitter_of(&document, 1).expect("there");
    assert_eq!(a.get("on_death").and_then(Value::text), Some("b"));
    assert!(apply(&mut document, joined(1, 2, 1)));
    let a = emitter_of(&document, 1).expect("there");
    assert_eq!(a.get("on_hit").and_then(Value::text), Some("b"));
    let effect = read_effect("joined", &document).expect("reads");
    assert_eq!(effect.emitters[0].on_hit.as_deref(), Some("b"));
}

#[test]
fn a_row_only_takes_what_it_says() {
    let mut document = two();
    let before = document.clone();
    assert!(
        !apply(&mut document, joined(TIMELINE, 2, 0)),
        "the timeline has times"
    );
    assert!(
        !apply(&mut document, joined(1, 2, 2)),
        "and an emitter hasn't"
    );
    assert_eq!(document, before);
}

#[test]
fn the_timeline_starts_at_a_time_that_can_change() {
    let mut document = two();
    assert!(apply(&mut document, joined(TIMELINE, 2, 2)));
    let times = timeline_of(&document);
    assert_eq!(times.len(), 1);
    assert_eq!(times[0].1, 0.0);
    assert_eq!(times[0].2, "puff");
    set_timeline(&mut document, times[0].0, 1.5);
    let effect = read_effect("timed", &document).expect("reads");
    assert_eq!(effect.timeline, [(1.5, "puff".to_owned())]);
}

#[test]
fn cutting_a_row_takes_away_what_starts_it() {
    let mut document = two();
    assert!(apply(&mut document, joined(TIMELINE, 2, 2)));
    assert!(apply(&mut document, ViewEvent::Cut(2, 0)));
    let (_, wires) = view_of(&document);
    assert_eq!(wires.len(), 1, "only that row");
    assert!(apply(&mut document, ViewEvent::Cut(2, 2)));
    assert!(view_of(&document).1.is_empty());
    assert!(
        emitter_of(&document, 1)
            .expect("there")
            .get("on_death")
            .is_none()
    );
}

#[test]
fn moving_a_box_is_kept_in_the_file() {
    let mut document = two();
    assert!(apply(&mut document, ViewEvent::Moved(2, [50.0, 60.0])));
    assert!(apply(&mut document, ViewEvent::Moved(TIMELINE, [7.0, 8.0])));
    let read = from_text(&to_text(&document)).expect("reads back");
    let (nodes, _) = view_of(&read);
    assert_eq!(nodes[2].at, [50.0, 60.0]);
    assert_eq!(nodes[0].at, [7.0, 8.0]);
    let effect = read_effect("moved", &read).expect("reads");
    assert_eq!(effect.emitters.len(), 2, "and it's still an effect");
}

#[test]
fn what_is_typed_becomes_what_it_says() {
    let mut document = two();
    let block = emitter_mut(&mut document, 1).expect("there");
    set_setting(block, "rate", "25").expect("sets");
    set_setting(block, "size", "0 4 1 0").expect("sets");
    set_setting(block, "blend", "add").expect("sets");
    set_setting(block, "lit", "true").expect("sets");
    set_setting(block, "gpu", "sim").expect("sets");
    set_setting(block, "on_death", "").expect("clears");
    assert_eq!(setting(block, "rate"), "25");
    assert_eq!(
        block.fields.iter().filter(|(key, _)| key == "rate").count(),
        1
    );
    assert_eq!(setting(block, "on_death"), "");
    let shown = setting(block, "size");
    set_setting(block, "size", &shown).expect("reads as it shows");
    let effect = read_effect("typed", &document).expect("reads");
    let emitter = &effect.emitters[0];
    assert_eq!(emitter.rate, 25.0);
    assert_eq!(emitter.size.at(1.0), 0.0);
    assert_eq!(emitter.size.at(0.0), 4.0);
    assert!(emitter.lit && emitter.sim);
}

#[test]
fn what_cannot_be_read_changes_nothing() {
    let mut document = two();
    let block = emitter_mut(&mut document, 1).expect("there");
    assert!(set_setting(block, "rate", "} oops {").is_err());
    assert_eq!(setting(block, "rate"), "10");
}

#[test]
fn a_new_name_goes_everywhere_it_is_said() {
    let mut document = two();
    assert!(apply(&mut document, joined(TIMELINE, 2, 2)));
    rename(&mut document, 2, "smoke").expect("renames");
    let effect = read_effect("renamed", &document).expect("reads");
    assert_eq!(effect.emitters[1].name, "smoke");
    assert_eq!(effect.emitters[0].on_death.as_deref(), Some("smoke"));
    assert_eq!(effect.timeline[0].1, "smoke");
    assert!(rename(&mut document, 2, "sparks").is_err(), "that's taken");
    assert!(rename(&mut document, 2, "two words").is_err());
}

#[test]
fn an_emitter_that_goes_takes_what_starts_it_along() {
    let mut document = two();
    assert!(apply(&mut document, joined(TIMELINE, 2, 2)));
    remove_emitter(&mut document, 2);
    assert_eq!(emitter_blocks(&document).len(), 1);
    assert!(timeline_of(&document).is_empty());
    let effect = read_effect("left", &document).expect("reads");
    assert_eq!(effect.emitters[0].on_death, None);
}
