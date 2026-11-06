//! Stopping with work still in hand.
#![expect(clippy::unwrap_used, reason = "a failed test is the intended report")]

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::{self, ThreadId};
use std::time::Duration;

use engine_cache::{
    AccountingDomain, Admission, AdmissionPermit, AllocationClass, Backing, Cache, CleanupContext,
    CountLimit, Pool, Producer, Production, ProductionAccess, ProductionClaim, ProviderDecision,
    RangeStore, SourceBuffers, SourceSnapshot,
};

struct Bytes {
    data: Box<[u8]>,
    dropped: Sender<ThreadId>,
}

impl Drop for Bytes {
    fn drop(&mut self) {
        let _ = self.dropped.send(thread::current().id());
    }
}

type Access<'a> = ProductionAccess<Backing<'a, Bytes>, usize, ()>;

// A concrete simulated provider whose first stop delivers a callback and fails.
// Its slot stays owned on failure; external final access has its own capability.
struct Provider<'a> {
    root: Option<Producer<Backing<'a, Bytes>, usize, ()>>,
    slot: Option<AdmissionPermit>,
}

impl<'a> Provider<'a> {
    fn stop(&mut self, handoff: &Sender<Access<'a>>, roots: &Admission) -> Result<(), ()> {
        assert!(roots.snapshot().closed, "close roots before provider stop");
        if let Some(mut root) = self.root.take() {
            let bytes = root.inputs().unwrap().view().get().data.len();
            let child = root.consume_inputs().unwrap().unwrap();
            root.complete(Ok(bytes)).unwrap();
            assert!(handoff.send(child).is_ok());
            return Err(());
        }
        drop(self.slot.take());
        Ok(())
    }
}

#[test]
fn stop_callback_and_timed_out_wait_preserve_services_and_external_final_access() {
    let roots = Admission::new(CountLimit::new(1));
    let slots = Admission::new(CountLimit::new(1));
    let accounting = AccountingDomain::new();
    let cleanup = CleanupContext::new();
    let (dropped, observed) = mpsc::channel();
    let input = cleanup
        .try_acquire(
            Bytes {
                data: Box::from([1, 2, 3, 4]),
                dropped,
            },
            &accounting,
            4,
            AllocationClass::Temporary,
        )
        .unwrap();
    let (production, submission) =
        Production::prepare_with_permit(input, roots.try_acquire().unwrap());
    let mut provider = Provider {
        root: None,
        slot: Some(slots.try_acquire().unwrap()),
    };
    submission.submit(|root| {
        provider.root = Some(root);
        ProviderDecision::Accepted
    });
    roots.close();
    slots.close();
    thread::scope(|scope| {
        // Gates disconnect before scope auto-join if a caller assertion fails.
        let (handoff, incoming) = mpsc::channel::<Access<'_>>();
        let (release, gate) = mpsc::channel();
        let (finished, completion) = mpsc::channel();
        let reader = scope.spawn(move || {
            let Ok(child) = incoming.recv() else {
                return;
            };
            if gate.recv().is_ok() {
                assert_eq!(
                    child.inputs().unwrap().view().get().data.as_ref(),
                    &[1, 2, 3, 4]
                );
            }
            drop(child);
            let _ = finished.send(());
        });
        // Keep the callback route alive throughout stop, including its failure.
        assert_eq!(provider.stop(&handoff, &roots), Err(()));
        assert_eq!(slots.snapshot().active, 1);
        assert!(production.snapshot().completion_recorded);
        assert!(matches!(production.claim(), ProductionClaim::Pending));
        // The unreleased reader gate makes this an actual failed wait without
        // relying on scheduler timing or sleep-based correctness.
        assert_eq!(
            completion.recv_timeout(Duration::ZERO),
            Err(RecvTimeoutError::Timeout)
        );
        assert_eq!(roots.snapshot().active, 1);
        assert_eq!(accounting.snapshot().temporary_bytes, 4);
        assert_eq!(cleanup.drain_budget(1), 0);
        assert!(observed.try_recv().is_err());
        provider.stop(&handoff, &roots).unwrap();
        assert_eq!(slots.snapshot().active, 0);
        assert_eq!(
            roots.snapshot().active,
            1,
            "provider stop is not external final use"
        );
        release.send(()).unwrap();
        completion.recv().unwrap();
        reader.join().unwrap();
    });
    assert!(matches!(production.claim(), ProductionClaim::Ready(Ok(4))));
    assert_eq!(roots.snapshot().active, 0);
    assert_eq!(cleanup.snapshot().pending_records, 1);
    assert_eq!(accounting.snapshot().retiring_bytes, 4);
    assert_eq!(cleanup.drain_budget(1), 1);
    assert_eq!(observed.recv().unwrap(), thread::current().id());
    assert_eq!(accounting.snapshot().total_declared_bytes, 0);
}

#[test]
fn facility_close_preserves_escaped_outputs_until_bounded_owner_cleanup() {
    let roots = Admission::new(CountLimit::new(1));
    let accounting = AccountingDomain::new();
    let range_accounting = AccountingDomain::new();
    let cleanup = CleanupContext::new();
    let (dropped, observations) = mpsc::channel();
    let acquire = |bytes: &[u8]| {
        cleanup
            .try_acquire(
                Bytes {
                    data: Box::from(bytes),
                    dropped: dropped.clone(),
                },
                &accounting,
                bytes.len() as u64,
                AllocationClass::Resident,
            )
            .unwrap()
    };
    let mut cache = Cache::<_, _, ()>::new();
    let cached = acquire(&[1, 2, 3, 4]);
    let escaped = cached.clone();
    cache.install("cached", cached, 4).unwrap();
    let pool = Pool::with_cleanup(&cleanup);
    for bytes in [[5, 6, 7, 8], [9, 10, 11, 12]] {
        pool.insert(
            1,
            Bytes {
                data: Box::from(bytes),
                dropped: dropped.clone(),
            },
            accounting
                .charge(4, AllocationClass::RetainedCapacity)
                .unwrap(),
        )
        .unwrap();
    }
    let checked_out = pool.checkout(1, 4).unwrap().unwrap();
    let source_owner = acquire(&[13, 14, 15]);
    let source = SourceSnapshot::new(source_owner.clone());
    let mut ranges = RangeStore::reference(&range_accounting);
    ranges
        .insert(&source, 0, source.context().get().data.as_ref())
        .unwrap();
    let taken_range = ranges.take(&source).unwrap().unwrap();
    let mut sources = SourceBuffers::new(0);
    sources.insert_read(1, source_owner, 3).unwrap();
    let taken_source = sources.take(1).unwrap();
    drop(source);

    roots.close();
    drop(cache);
    assert_eq!(pool.close(), 1);
    assert_eq!(ranges.close(), 0);
    sources.close();
    assert!(roots.snapshot().closed);
    assert_eq!(roots.snapshot().active, 0);
    assert!(pool.snapshot().closed);
    assert_eq!(pool.snapshot().checked_out_items, 1);
    assert!(pool.checkout(1, 0).is_err());
    assert!(ranges.snapshot().closed);
    assert!(sources.snapshot().closed);
    assert_eq!(escaped.view().get().data.as_ref(), &[1, 2, 3, 4]);
    assert_eq!(checked_out.get().data.len(), 4);
    assert_eq!(taken_range.bytes(), &[13, 14, 15]);
    assert_eq!(taken_source.view().get().data.as_ref(), &[13, 14, 15]);
    assert_eq!(cleanup.snapshot().pending_records, 1);
    assert_eq!(cleanup.drain_budget(1), 1);
    assert_eq!(accounting.snapshot().total_declared_bytes, 11);
    assert!(range_accounting.snapshot().resident_bytes >= 3);

    drop((escaped, checked_out, taken_range, taken_source));
    assert_eq!(pool.snapshot().checked_out_items, 0);
    assert_eq!(cleanup.snapshot().pending_records, 3);
    assert_eq!(range_accounting.snapshot().total_declared_bytes, 0);
    assert_eq!(cleanup.drain_budget(1), 1);
    assert_eq!(cleanup.snapshot().pending_records, 2);
    assert_eq!(cleanup.drain_budget(2), 2);
    assert_eq!(cleanup.snapshot().in_flight_records, 0);
    assert_eq!(accounting.snapshot().total_declared_bytes, 0);
    assert_eq!(
        observations.try_iter().collect::<Vec<_>>(),
        vec![thread::current().id(); 4]
    );
}
