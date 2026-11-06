//! A cache of values found by key.
#![expect(clippy::unwrap_used, reason = "an example keeps to what it shows")]
#![expect(clippy::print_stdout, reason = "an example reports to its console")]

use engine_cache::{AccountingDomain, AllocationClass, ByteLimit, Cache, CleanupContext, Lookup};

fn main() {
    let domain = AccountingDomain::new();
    let cleanup = CleanupContext::new();
    let mut cache = Cache::<&str, Vec<u8>>::new();
    let bytes = vec![10_u8, 20, 30, 40];
    let cost = bytes.capacity() as u64;
    let backing = cleanup
        .try_acquire(bytes, &domain, cost, AllocationClass::Resident)
        .unwrap();
    let (identity, _) = cache.install("source", backing, cost).unwrap();

    let Lookup::Ready(consumer) = cache.share(&"source") else {
        panic!("installed source must be ready");
    };
    // This example explicitly chooses a tiny threshold to demonstrate pressure.
    let threshold = ByteLimit::AtOrAbove(cost);
    let blocked = cache.maintain(threshold, 1, false);
    assert_eq!(blocked.pinned_encountered, 1);
    assert!(blocked.pressure_after);
    assert_eq!(consumer.view().map(|bytes| &bytes[1..3]).get(), &[20, 30]);

    drop(consumer);
    let collected = cache.maintain(threshold, 1, false);
    assert_eq!(collected.evicted_payloads, 1);
    assert_eq!(cache.policy_bytes(), 0);
    assert!(cache.contains_identity(&identity));
    assert!(matches!(cache.lookup_identity(&identity), Lookup::Vacant));
    assert_eq!(domain.snapshot().retiring_bytes, cost);
    assert_eq!(cleanup.drain(), 1);
    assert_eq!(domain.snapshot().total_declared_bytes, 0);
    println!("Collected idle payload; logical identity survived deferred cleanup.");
}
