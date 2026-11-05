use std::panic::{AssertUnwindSafe, catch_unwind};

use engine_cache::{
    AccountingDomain, AccountingError, AllocationClass, Backing, CountLimit, RangeError,
    RangeInsert, RangeStore, SourceSnapshot,
};

fn source(bytes: &[u8]) -> SourceSnapshot<'static, Vec<u8>> {
    let domain = AccountingDomain::new();
    SourceSnapshot::new(
        Backing::try_new(
            bytes.to_vec(),
            &domain,
            bytes.len() as u64,
            AllocationClass::Resident,
        )
        .unwrap(),
    )
}

#[test]
fn copied_full_partial_and_missed_prefixes_report_actual_bytes() {
    let domain = AccountingDomain::new();
    let source = source(b"abcdefghijklmnop");
    let mut store = RangeStore::reference(&domain);
    let mut input = b"efghij".to_vec();
    assert_eq!(
        store.insert(&source, 4, &input).unwrap(),
        RangeInsert::Inserted
    );
    input.fill(b'x');
    drop(input);

    for (offset, length, expected) in [
        (4, 6, &b"efghij"[..]),
        (6, 2, &b"gh"[..]),
        (8, 5, &b"ij"[..]),
        (3, 4, &b""[..]),
        (10, 2, &b""[..]),
    ] {
        let mut destination = vec![b'?'; length];
        let read = store
            .copy_prefix(&source, offset, &mut destination)
            .unwrap();
        assert_eq!(read.copied(), expected.len(), "offset {offset}");
        assert_eq!(&destination[..read.copied()], expected);
        assert!(
            destination[read.copied()..]
                .iter()
                .all(|byte| *byte == b'?')
        );
        assert_eq!(read.remaining_offset(), offset + expected.len() as u64);
        assert_eq!(read.remaining_length(), length - expected.len());
        assert_eq!(read.is_complete(), expected.len() == length);
        assert!(read.source().same_revision(&source));
    }
    assert_eq!(store.snapshot().entries, 1);
}

#[test]
fn a_partial_read_retains_its_source_revision_across_replacement_and_close() {
    let domain = AccountingDomain::new();
    let old = source(b"abcdefgh");
    let mut store = RangeStore::reference(&domain);
    store.insert(&old, 0, b"abcd").unwrap();
    let mut destination = [0; 8];
    let read = store.copy_prefix(&old, 0, &mut destination).unwrap();

    let revised = old.revised(
        Backing::try_new(
            b"ABCDEFGH".to_vec(),
            &AccountingDomain::new(),
            8,
            AllocationClass::Resident,
        )
        .unwrap(),
    );
    assert!(old.same_source(&revised));
    assert!(!old.same_revision(&revised));
    assert_eq!(
        store
            .copy_prefix(&revised, 0, &mut [0; 8])
            .unwrap()
            .copied(),
        0
    );
    assert_eq!(
        store.insert(&revised, 0, b"ABCD").unwrap(),
        RangeInsert::Replaced
    );
    assert_eq!(store.copy_prefix(&old, 0, &mut [0; 8]).unwrap().copied(), 0);
    let mut current = [0; 4];
    assert_eq!(
        store
            .copy_prefix(&revised, 0, &mut current)
            .unwrap()
            .copied(),
        4
    );
    assert_eq!(&current, b"ABCD");
    store.close();
    drop(old);
    drop(revised);
    let remainder = read.remaining_offset() as usize;
    destination[read.copied()..].copy_from_slice(
        &read.source().context().get()[remainder..remainder + read.remaining_length()],
    );
    assert_eq!(&destination, b"abcdefgh");
}

#[test]
fn smaller_replacement_and_whole_take_preserve_capacity_and_its_charge() {
    let domain = AccountingDomain::new();
    let source = source(b"abcdefgh");
    let mut store = RangeStore::reference(&domain);
    store.insert(&source, 0, b"abcdefgh").unwrap();
    let retained = store.snapshot().retained_capacity_bytes;
    assert!(retained >= 8);
    store.insert(&source, 2, b"cd").unwrap();
    assert_eq!(store.snapshot().valid_bytes, 2);
    assert_eq!(store.snapshot().retained_capacity_bytes, retained);
    assert_eq!(domain.snapshot().resident_bytes, retained);
    let allocation = store.take(&source).unwrap().unwrap();
    assert_eq!(allocation.offset(), 2);
    assert_eq!(allocation.bytes(), b"cd");
    assert_eq!(allocation.capacity() as u64, retained);
    assert_eq!(allocation.declared_bytes(), retained);
    assert!(allocation.source().same_revision(&source));
    assert_eq!(store.snapshot().entries, 0);
    assert!(store.take(&source).unwrap().is_none());
    assert_eq!(store.close(), 0);
    assert_eq!(domain.snapshot().total_declared_bytes, retained);
    let (_source, _offset, bytes, charge) = allocation.into_parts();
    drop(bytes);
    assert_eq!(domain.snapshot().total_declared_bytes, retained);
    drop(charge);
    assert_eq!(domain.snapshot().total_declared_bytes, 0);
}

#[test]
fn occupancy_and_eviction_follow_the_mixed_reference_score_rule() {
    let domain = AccountingDomain::new();
    let sources: Vec<_> = (0..17).map(|_| source(&[])).collect();
    let mut reference = RangeStore::reference(&domain);
    for (index, source) in sources.iter().enumerate() {
        reference
            .insert(source, 0, &vec![index as u8; 1024])
            .unwrap();
    }
    assert_eq!(reference.snapshot().entries, 16);
    assert_eq!(reference.snapshot().valid_bytes, 16 * 1024);
    assert_eq!(
        reference
            .copy_prefix(&sources[0], 0, &mut [0])
            .unwrap()
            .copied(),
        0
    );
    assert_eq!(
        reference
            .copy_prefix(&sources[16], 0, &mut [0])
            .unwrap()
            .copied(),
        1
    );

    let mut store = RangeStore::new(CountLimit::new(3), &domain);
    store.insert(&sources[0], 0, b"a").unwrap();
    store.insert(&sources[1], 0, b"b").unwrap();
    store.insert(&sources[2], 0, b"c").unwrap();
    assert_eq!(
        store.insert(&sources[0], 0, b"a").unwrap(),
        RangeInsert::Refreshed
    );
    let mut unchanged = [0];
    store.copy_prefix(&sources[1], 0, &mut unchanged).unwrap();
    store.insert(&sources[0], 0, b"a").unwrap();
    // A's score and C's score are now both three. The first tie is evicted,
    // despite A's most recent duplicate insertion; incoming bytes were not copied.
    store.insert(&sources[3], 0, b"d").unwrap();
    assert_eq!(
        store
            .copy_prefix(&sources[0], 0, &mut unchanged)
            .unwrap()
            .copied(),
        0
    );
    assert_eq!(
        store
            .copy_prefix(&sources[2], 0, &mut unchanged)
            .unwrap()
            .copied(),
        1
    );
    assert_eq!(&unchanged, b"c");

    let mut duplicate = RangeStore::new(CountLimit::new(1), &domain);
    duplicate.insert(&sources[0], 0, b"a").unwrap();
    // Deliberately violate the interchangeable-byte provider contract to probe
    // the recovered no-recopy operation, not to authorize unreported mutation.
    duplicate.insert(&sources[0], 0, b"x").unwrap();
    duplicate
        .copy_prefix(&sources[0], 0, &mut unchanged)
        .unwrap();
    assert_eq!(&unchanged, b"a");
}

#[test]
fn taking_preserves_first_empty_slot_and_first_minimum_tie_order() {
    let domain = AccountingDomain::new();
    let sources: Vec<_> = (0..5).map(|_| source(&[])).collect();
    let mut store = RangeStore::new(CountLimit::new(3), &domain);
    for source in &sources[..3] {
        store.insert(source, 0, b"a").unwrap();
    }
    drop(store.take(&sources[0]).unwrap().unwrap());
    assert_eq!(
        store.insert(&sources[3], 0, b"a").unwrap(),
        RangeInsert::Inserted
    );
    store.insert(&sources[1], 0, b"a").unwrap();
    store.insert(&sources[1], 0, b"a").unwrap();
    store.copy_prefix(&sources[2], 0, &mut [0]).unwrap();
    store.insert(&sources[4], 0, b"a").unwrap();
    assert_eq!(
        store
            .copy_prefix(&sources[3], 0, &mut [0])
            .unwrap()
            .copied(),
        0
    );
    assert_eq!(
        store
            .copy_prefix(&sources[1], 0, &mut [0])
            .unwrap()
            .copied(),
        1
    );
}

struct PanicContext {
    panic_on_drop: bool,
}

impl Drop for PanicContext {
    fn drop(&mut self) {
        assert!(
            !self.panic_on_drop,
            "injected source-context destructor panic"
        );
    }
}

#[test]
fn context_destructor_panics_after_replacement_has_committed_coherently() {
    for bytes in [&b"xy"[..], &b"abcdefgh"[..]] {
        let domain = AccountingDomain::new();
        let old = SourceSnapshot::new(
            Backing::try_new(
                PanicContext {
                    panic_on_drop: true,
                },
                &AccountingDomain::new(),
                0,
                AllocationClass::Resident,
            )
            .unwrap(),
        );
        let revised = old.revised(
            Backing::try_new(
                PanicContext {
                    panic_on_drop: false,
                },
                &AccountingDomain::new(),
                0,
                AllocationClass::Resident,
            )
            .unwrap(),
        );
        let other = SourceSnapshot::new(
            Backing::try_new(
                PanicContext {
                    panic_on_drop: false,
                },
                &AccountingDomain::new(),
                0,
                AllocationClass::Resident,
            )
            .unwrap(),
        );
        let third = SourceSnapshot::new(
            Backing::try_new(
                PanicContext {
                    panic_on_drop: false,
                },
                &AccountingDomain::new(),
                0,
                AllocationClass::Resident,
            )
            .unwrap(),
        );
        let mut store = RangeStore::new(CountLimit::new(2), &domain);
        store.insert(&other, 0, b"q").unwrap();
        store.insert(&old, 0, b"abcd").unwrap();
        drop(old);
        let panic = catch_unwind(AssertUnwindSafe(|| {
            store.insert(&revised, 2, bytes).unwrap();
        }))
        .unwrap_err();
        assert_eq!(
            panic.downcast_ref::<&str>(),
            Some(&"injected source-context destructor panic")
        );
        let mut destination = vec![0; bytes.len()];
        assert_eq!(
            store
                .copy_prefix(&revised, 2, &mut destination)
                .unwrap()
                .copied(),
            bytes.len()
        );
        assert_eq!(destination, bytes);
        assert_eq!(
            store.copy_prefix(&revised, 0, &mut [0]).unwrap().copied(),
            0
        );
        assert_eq!(store.snapshot().valid_bytes, bytes.len() as u64 + 1);
        // The replacement must commit the shared sequence before old cleanup.
        // Three local refreshes tie slot zero at four with the post-panic hit.
        // If cleanup ran before the sequence commit, the revised score is only
        // three and it is incorrectly evicted instead of the first tied slot.
        for _ in 0..3 {
            store.insert(&other, 0, b"q").unwrap();
        }
        store.insert(&third, 0, b"z").unwrap();
        assert_eq!(
            store
                .copy_prefix(&revised, 2, &mut destination)
                .unwrap()
                .copied(),
            bytes.len()
        );
        store.close();
        assert_eq!(domain.snapshot().total_declared_bytes, 0);
    }
}

#[test]
fn zero_length_lookup_refreshes_only_a_contained_start() {
    let domain = AccountingDomain::new();
    let sources = [source(&[]), source(&[]), source(&[])];
    for (offset, expected_first_survives) in [(0, true), (1, false)] {
        let mut store = RangeStore::new(CountLimit::new(2), &domain);
        store.insert(&sources[0], 0, b"a").unwrap();
        store.insert(&sources[1], 0, b"b").unwrap();
        assert!(
            store
                .copy_prefix(&sources[0], offset, &mut [])
                .unwrap()
                .is_complete()
        );
        store.insert(&sources[2], 0, b"c").unwrap();
        assert_eq!(
            store
                .copy_prefix(&sources[0], 0, &mut [0])
                .unwrap()
                .copied()
                == 1,
            expected_first_survives,
        );
    }
}

#[test]
fn checked_bounds_disabled_and_closed_errors_preserve_bytes_and_membership() {
    let domain = AccountingDomain::new();
    let source = source(b"a");
    let mut store = RangeStore::reference(&domain);
    store.insert(&source, u64::MAX - 1, b"a").unwrap();
    let before = store.snapshot();
    assert_eq!(
        store.insert(&source, u64::MAX, b"x"),
        Err(RangeError::BoundsOverflow)
    );
    let mut destination = [b'?'; 2];
    assert_eq!(
        store
            .copy_prefix(&source, u64::MAX - 1, &mut destination)
            .err(),
        Some(RangeError::BoundsOverflow),
    );
    assert_eq!(&destination, b"??");
    assert_eq!(store.snapshot(), before);
    assert_eq!(
        store.copy_prefix(&source, u64::MAX, &mut [0]).err(),
        Some(RangeError::BoundsOverflow)
    );
    assert!(
        store
            .copy_prefix(&source, u64::MAX, &mut [])
            .unwrap()
            .is_complete()
    );
    let mut hit = [0];
    assert_eq!(
        store
            .copy_prefix(&source, u64::MAX - 1, &mut hit)
            .unwrap()
            .copied(),
        1
    );
    assert_eq!(&hit, b"a");

    let mut disabled = RangeStore::new(CountLimit::new(0), &domain);
    assert_eq!(disabled.insert(&source, 0, b"a"), Err(RangeError::Disabled));
    assert_eq!(
        disabled.copy_prefix(&source, 0, &mut hit).unwrap().copied(),
        0
    );
    assert_eq!(store.close(), 1);
    assert_eq!(store.close(), 0);
    assert_eq!(store.insert(&source, 0, b"a"), Err(RangeError::Closed));
    assert_eq!(
        store.copy_prefix(&source, 0, &mut hit).err(),
        Some(RangeError::Closed)
    );
    assert_eq!(store.take(&source).err(), Some(RangeError::Closed));
}

#[test]
fn accounting_rejection_preserves_the_previous_range_and_capacity() {
    let domain = AccountingDomain::new();
    let source = source(b"abcdefgh");
    let mut store = RangeStore::reference(&domain);
    store.insert(&source, 0, b"abcd").unwrap();
    let before = store.snapshot();
    let blocker = domain
        .charge(
            u64::MAX - before.retained_capacity_bytes,
            AllocationClass::Temporary,
        )
        .unwrap();
    assert_eq!(
        store.insert(&source, 1, b"abcdefgh"),
        Err(RangeError::Accounting(AccountingError::Overflow)),
    );
    assert_eq!(store.snapshot(), before);
    let mut destination = [0; 4];
    store.copy_prefix(&source, 0, &mut destination).unwrap();
    assert_eq!(&destination, b"abcd");
    drop(blocker);
    assert_eq!(
        domain.snapshot().total_declared_bytes,
        before.retained_capacity_bytes
    );
}
