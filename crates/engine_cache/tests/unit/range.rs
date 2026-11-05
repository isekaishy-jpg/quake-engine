use super::{RangeError, RangeInsert, RangeStore, SourceSnapshot};
use crate::{AccountingDomain, AllocationClass, Backing, CountLimit};

#[test]
fn exhausted_sequence_and_entry_scores_reject_before_copy_or_replacement() {
    let domain = AccountingDomain::new();
    let source =
        SourceSnapshot::new(Backing::try_new((), &domain, 0, AllocationClass::Resident).unwrap());
    let mut store = RangeStore::new(CountLimit::new(1), &domain);
    store.insert(&source, 0, b"abcd").unwrap();
    let before = store.snapshot();
    store.sequence = u64::MAX;
    let mut destination = [b'?'; 4];
    assert_eq!(
        store.copy_prefix(&source, 0, &mut destination).err(),
        Some(RangeError::ScoreExhausted),
    );
    assert_eq!(&destination, b"????");
    assert_eq!(
        store.insert(&source, 1, b"x"),
        Err(RangeError::ScoreExhausted)
    );
    assert_eq!(store.snapshot(), before);
    assert_eq!(
        store
            .copy_prefix(&source, 4, &mut destination)
            .unwrap()
            .copied(),
        0
    );
    // Identical refresh needs only the local score, not shared sequence room.
    assert_eq!(
        store.insert(&source, 0, b"abcd"),
        Ok(RangeInsert::Refreshed)
    );
    store.sequence = 3;
    store.entries[0].as_mut().unwrap().score = u64::MAX;
    assert_eq!(
        store.insert(&source, 0, b"abcd"),
        Err(RangeError::ScoreExhausted)
    );
    assert_eq!(store.snapshot(), before);
    store.copy_prefix(&source, 0, &mut destination).unwrap();
    assert_eq!(&destination, b"abcd");
}
