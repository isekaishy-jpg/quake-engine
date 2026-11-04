//! The entity table's contract: slot order and reuse, from `ED_Alloc`.

use engine_sim::{EntityId, SlotTable};

/// Well past the startup grace.
const LATE: f64 = 100.0;

/// A table of six, as ids and values in order.
fn six() -> (SlotTable<u32>, Vec<EntityId>) {
    let mut table = SlotTable::new();
    let ids = (0..6).map(|value| table.insert(value, LATE)).collect();
    (table, ids)
}

/// The values in slot order.
fn values(table: &SlotTable<u32>) -> Vec<u32> {
    table.iter().map(|(_, value)| *value).collect()
}

#[test]
fn iteration_follows_slot_order() {
    let (table, _ids) = six();
    assert_eq!(values(&table), [0, 1, 2, 3, 4, 5]);
}

#[test]
fn a_freed_slot_waits_half_a_second_then_the_lowest_is_reused() {
    let (mut table, ids) = six();
    assert_eq!(table.remove(ids[1], LATE), Some(1));
    assert_eq!(table.remove(ids[3], LATE), Some(3));
    let _soon = table.insert(10, LATE + 0.4);
    assert_eq!(
        values(&table),
        [0, 2, 4, 5, 10],
        "too soon, so it goes on the end"
    );
    let _later = table.insert(11, LATE + 0.6);
    assert_eq!(values(&table), [0, 11, 2, 4, 5, 10], "the lowest free slot");
}

#[test]
fn early_in_a_level_slots_are_reused_at_once() {
    let mut table = SlotTable::new();
    let first = table.insert(1, 0.5);
    let _second = table.insert(2, 0.5);
    assert_eq!(table.remove(first, 0.5), Some(1));
    let _third = table.insert(3, 0.6);
    assert_eq!(values(&table), [3, 2]);
}

#[test]
fn an_old_id_stops_working_after_its_slot_is_reused() {
    let (mut table, ids) = six();
    assert_eq!(table.remove(ids[1], LATE), Some(1));
    let new = table.insert(9, LATE + 1.0);
    assert_eq!(table.get(ids[1]), None);
    assert_eq!(table.get_mut(ids[1]), None);
    assert_eq!(table.remove(ids[1], LATE + 1.0), None);
    assert_eq!(table.get(new), Some(&9));
}

#[test]
fn ids_survive_a_trip_through_bits() {
    let (mut table, ids) = six();
    assert_eq!(table.remove(ids[2], LATE), Some(2));
    let id = table.insert(7, LATE + 1.0);
    assert_eq!(EntityId::from_bits(id.to_bits()), id);
}
