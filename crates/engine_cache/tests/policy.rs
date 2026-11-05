use engine_cache::{
    ByteLimit, CACHE_REFERENCE_PAYLOAD_RETENTION, CACHE_REFERENCE_PRODUCTION_LIMIT,
    CACHE_REFERENCE_PROVIDER_SLOTS, CACHE_REFERENCE_RANGE_ENTRIES, CountLimit, PolicyCounter,
    PolicyCounterError,
};

#[test]
fn count_admission_includes_existing_work_and_preserves_over_limit_state() {
    let limit = CACHE_REFERENCE_PRODUCTION_LIMIT;
    assert_eq!(limit.remaining(5), 11);
    assert!(limit.allows(15));
    assert!(!limit.allows(16));
    assert!(!limit.allows(17));
    assert_eq!(limit.remaining(usize::MAX), 0);
    assert!(!CountLimit::new(0).allows(0));
    assert_eq!(CountLimit::new(0).remaining(0), 0);
    assert_eq!(CACHE_REFERENCE_PROVIDER_SLOTS.limit(), 8);
    assert_eq!(CACHE_REFERENCE_RANGE_ENTRIES.limit(), 16);
}

#[test]
fn byte_thresholds_keep_equality_and_zero_semantics_distinct() {
    for (limit, usage, reached) in [
        (ByteLimit::Disabled, u64::MAX, false),
        (ByteLimit::AtOrAbove(0), 0, true),
        (ByteLimit::Above(0), 0, false),
        (ByteLimit::Above(0), 1, true),
        (ByteLimit::AtOrAbove(10), 9, false),
        (ByteLimit::AtOrAbove(10), 10, true),
        (ByteLimit::Above(10), 10, false),
        (ByteLimit::Above(10), 11, true),
    ] {
        assert_eq!(limit.reached(usage), reached, "{limit:?}, {usage}");
    }
    let reference_bytes = 32 * 1024 * 1024;
    assert!(!CACHE_REFERENCE_PAYLOAD_RETENTION.reached(reference_bytes - 1));
    assert!(CACHE_REFERENCE_PAYLOAD_RETENTION.reached(reference_bytes));
}

#[test]
fn policy_arithmetic_is_checked_and_reset_does_not_imply_cleanup() {
    let mut counter = PolicyCounter::new(7);
    assert_eq!(counter.try_sub(8), Err(PolicyCounterError::Underflow));
    assert_eq!(counter.value(), 7);
    assert_eq!(counter.try_add(u64::MAX), Err(PolicyCounterError::Overflow));
    assert_eq!(counter.value(), 7);
    counter.try_add(u64::MAX - 7).unwrap();
    counter.try_add(0).unwrap();
    assert_eq!(counter.value(), u64::MAX);
    assert_eq!(counter.clear(), u64::MAX);
    assert_eq!(counter.value(), 0);
    counter.try_sub(0).unwrap();
}

#[test]
fn soft_limit_observation_can_follow_an_insertion_that_crosses_it() {
    let stop = ByteLimit::Above(10);
    let mut admitted_bytes = PolicyCounter::new(9);
    admitted_bytes.try_add(4).unwrap();
    assert!(stop.reached(admitted_bytes.value()));
    assert_eq!(admitted_bytes.value(), 13);
    assert!(!ByteLimit::Disabled.reached(admitted_bytes.value()));
}
