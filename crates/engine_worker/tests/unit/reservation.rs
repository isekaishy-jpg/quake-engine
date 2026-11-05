use super::{CapacityUsage, Cost, LimitError, Limits, ReservationError, ReservationPool};

fn limits(ceiling: Option<usize>) -> Limits {
    Limits::new(Cost::new(2, 2, 2, 100), Cost::new(2, 2, 2, 0), 1, ceiling).unwrap()
}

#[test]
fn ordinary_cannot_borrow_required_metadata_and_required_can_cross_byte_target() {
    let pool = ReservationPool::new(7, limits(None));
    let ordinary = pool.try_reserve_ordinary(Cost::new(2, 0, 0, 100)).unwrap();
    assert!(matches!(
        pool.try_reserve_ordinary(Cost::new(1, 0, 0, 0)),
        Err(ReservationError::Full)
    ));
    let required = pool.try_reserve_required(Cost::new(2, 1, 1, 500)).unwrap();
    assert_eq!(required.runtime_identity(), 7);
    assert_eq!(pool.snapshot().required.bytes, 500);
    assert!(matches!(
        pool.try_reserve_required(Cost::new(0, 0, 0, 1)),
        Err(ReservationError::Full)
    ));
    drop(required);
    drop(ordinary);
    assert_eq!(
        pool.snapshot(),
        CapacityUsage {
            ordinary: Cost::default(),
            required: Cost::default(),
            required_pipelines: 0
        }
    );
}

#[test]
fn child_and_retained_bytes_outlive_pipeline_handle() {
    let pool = ReservationPool::new(9, limits(Some(150)));
    let root = pool.try_reserve_required(Cost::new(1, 1, 1, 125)).unwrap();
    let stage = root.stage(Cost::new(1, 1, 1, 125)).unwrap();
    let retained = stage.retain_bytes(100).unwrap();
    drop(root);
    drop(stage);
    assert_eq!(pool.snapshot().required.bytes, 100);
    assert_eq!(pool.snapshot().required_pipelines, 0);
    assert!(matches!(
        pool.try_reserve_required(Cost::new(1, 0, 0, 60)),
        Err(ReservationError::Full)
    ));
    let next = pool.try_reserve_required(Cost::new(1, 0, 0, 40)).unwrap();
    drop(next);
    drop(retained);
    assert_eq!(pool.snapshot().required.bytes, 0);
}

#[test]
fn stage_credits_return_after_nested_children_drop() {
    for drop_order in 0..3 {
        let pool = ReservationPool::new(1, limits(None));
        let root = pool.try_reserve_required(Cost::new(2, 2, 2, 20)).unwrap();
        let stage = root.stage(Cost::new(2, 2, 2, 10)).unwrap();
        let first_output = stage.retain_bytes(5).unwrap();
        let second_output = stage.retain_bytes(5).unwrap();
        let child = stage.stage(Cost::new(1, 1, 1, 0)).unwrap();
        match drop_order {
            0 => {
                drop(stage);
                drop(child);
            }
            1 => {
                drop(child);
                drop(stage);
            }
            _ => {
                let barrier = std::sync::Barrier::new(2);
                let barrier = &barrier;
                std::thread::scope(|inner| {
                    inner.spawn(move || {
                        barrier.wait();
                        drop(stage);
                    });
                    inner.spawn(move || {
                        barrier.wait();
                        drop(child);
                    });
                });
            }
        }
        assert_eq!(root.available(), Cost::new(2, 2, 2, 10));
        drop(first_output);
        assert_eq!(root.available(), Cost::new(2, 2, 2, 15));
        drop(second_output);
        assert_eq!(root.available(), Cost::new(2, 2, 2, 20));
        assert_eq!(pool.snapshot().required.bytes, 20);
    }
}

#[test]
fn grow_failure_preserves_original_credits() {
    let pool = ReservationPool::new(1, limits(Some(120)));
    let mut root = pool.try_reserve_required(Cost::new(1, 0, 0, 80)).unwrap();
    let ordinary = pool.try_reserve_ordinary(Cost::new(0, 0, 0, 30)).unwrap();
    let before = root.available();
    assert_eq!(
        root.try_grow(Cost::new(0, 0, 0, 50)),
        Err(ReservationError::TooLarge)
    );
    assert_eq!(root.available(), before);
    assert_eq!(pool.snapshot().required, before);
    assert_eq!(
        root.try_grow(Cost::new(0, 0, 0, 20)),
        Err(ReservationError::Full)
    );
    assert_eq!(root.available(), before);
    drop(ordinary);
    root.try_grow(Cost::new(1, 0, 0, 20)).unwrap();
    assert_eq!(root.available(), Cost::new(2, 0, 0, 100));
    assert_eq!(
        root.try_grow(Cost::new(1, 0, 0, 0)),
        Err(ReservationError::TooLarge)
    );
}

#[test]
fn invalid_limits_and_close() {
    assert_eq!(
        Limits::new(Cost::default(), Cost::default(), 1, None),
        Err(LimitError::ZeroRequiredCapacity)
    );
    assert_eq!(
        Limits::new(Cost::new(0, 0, 0, 10), Cost::new(1, 0, 0, 0), 1, Some(9)),
        Err(LimitError::OrdinaryTargetExceedsCeiling)
    );
    let pool = ReservationPool::new(1, limits(None));
    pool.close();
    assert!(matches!(
        pool.try_reserve_required(Cost::new(1, 0, 0, 0)),
        Err(ReservationError::Closed)
    ));
}
