use super::{Cost, Limits, ReservationError, ReservationPool};

#[test]
fn direct_owned_charges_refund_independently_and_reuse_empty_output() {
    let limits = Limits::new(Cost::new(4, 6, 0, 0), Cost::default(), 0, None).unwrap();
    let pool = ReservationPool::new(42, limits);
    let mut output = Vec::new();
    pool.try_charge_owned_many_into(2, 64, &mut output).unwrap();
    assert_eq!(output.len(), 3);
    assert_eq!(pool.snapshot().ordinary, Cost::new(3, 6, 0, 0));
    assert!(matches!(
        pool.try_charge_owned(1),
        Err(ReservationError::Full)
    ));
    drop(output.remove(1));
    assert_eq!(pool.snapshot().ordinary, Cost::new(2, 4, 0, 0));
    let replacement = pool.try_charge_owned(2).unwrap();
    output.clear();
    assert_eq!(pool.snapshot().ordinary, Cost::new(1, 2, 0, 0));
    pool.close();
    assert_eq!(pool.try_charge_owned_many_into(0, 0, &mut output), Ok(()));
    assert_eq!(
        pool.try_charge_owned_many_into(0, 1, &mut output),
        Err(ReservationError::Closed)
    );
    assert!(output.is_empty());
    drop(replacement);
    assert_eq!(pool.snapshot().ordinary, Cost::default());
}

#[test]
fn ordinary_portions_shrink_and_members_release_independently() {
    let limits = Limits::new(Cost::new(5, 7, 0, 0), Cost::new(1, 1, 0, 0), 1, None).unwrap();
    let pool = ReservationPool::new(1, limits);
    let per_member = Cost::new(1, 2, 0, 0);
    let mut members = pool.try_reserve_ordinary_many(per_member, 64).unwrap();
    assert_eq!(members.len(), 3);
    assert_eq!(pool.snapshot().ordinary, Cost::new(3, 6, 0, 0));
    assert!(matches!(
        pool.try_reserve_ordinary_many(per_member, 64),
        Err(ReservationError::Full)
    ));

    // A surviving sibling must not pin the retired member's capacity.
    drop(members.remove(1));
    assert_eq!(pool.snapshot().ordinary, Cost::new(2, 4, 0, 0));
    let replacement = pool.try_reserve_ordinary_many(per_member, 64).unwrap();
    assert_eq!(replacement.len(), 1);
    drop(members);
    assert_eq!(pool.snapshot().ordinary, per_member);
    drop(replacement);
    assert_eq!(pool.snapshot().ordinary, Cost::default());
}

#[test]
fn ordinary_portion_checks_products_and_combined_byte_ceiling() {
    for hard_ceiling in [None, Some(usize::MAX)] {
        let limits = Limits::new(
            Cost::new(64, usize::MAX, 0, usize::MAX),
            Cost::new(1, 0, 0, 0),
            1,
            hard_ceiling,
        )
        .unwrap();
        let pool = ReservationPool::new(2, limits);
        let required = pool
            .try_reserve_required(Cost::new(1, 0, 0, usize::MAX / 2))
            .unwrap();
        let cost = Cost::new(1, usize::MAX / 3, 0, usize::MAX / 3);
        let ordinary = pool.try_reserve_ordinary_many(cost, 64).unwrap();
        assert_eq!(ordinary.len(), 1);
        assert_eq!(pool.snapshot().ordinary, cost);
        assert!(matches!(
            pool.try_reserve_ordinary_many(cost, 64),
            Err(ReservationError::Full)
        ));
        drop(required);
        let remainder = pool.try_reserve_ordinary_many(cost, 64).unwrap();
        assert_eq!(remainder.len(), 2);
        assert_eq!(pool.snapshot().ordinary.edges, (usize::MAX / 3) * 3);
        drop(ordinary);
        drop(remainder);
        assert_eq!(pool.snapshot().ordinary, Cost::default());
    }
}

#[test]
fn ordinary_portion_classifies_individual_oversize_and_empty_closed_noop() {
    let limits = Limits::new(Cost::new(1, 1, 0, 0), Cost::default(), 0, None).unwrap();
    let pool = ReservationPool::new(3, limits);
    assert!(matches!(
        pool.try_reserve_ordinary_many(Cost::new(1, 2, 0, 0), 64),
        Err(ReservationError::TooLarge)
    ));
    assert_eq!(pool.snapshot().ordinary, Cost::default());
    pool.close();
    assert!(
        pool.try_reserve_ordinary_many(Cost::new(1, 2, 0, 0), 0)
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        pool.try_reserve_ordinary_many(Cost::new(1, 0, 0, 0), 64),
        Err(ReservationError::Closed)
    ));
}
