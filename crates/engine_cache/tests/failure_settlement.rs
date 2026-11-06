//! What a failed production leaves behind.

use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use engine_cache::{
    AccountingDomain, Admission, AllocationClass, Backing, Cache, CountLimit, Production,
    ProductionClaim, ProductionMap, ProductionPhase, ProductionStart, ProviderDecision,
    StartRejectionReason, SubmissionOutcome,
};

#[test]
fn shared_rejection_preserves_demand_and_explicit_retry_returns_unused_inputs() {
    let mut cache = Cache::<_, ()>::new();
    let identity = cache.ensure("key");
    let budget = Admission::new(CountLimit::new(1));
    let mut map = ProductionMap::<_, u8, ()>::new(budget.clone());
    let first = map.interest(identity.clone(), 0, 10);
    let second = map.interest(identity.clone(), 0, 20);
    let ProductionStart::Started(attempt) = map.begin_shared(&identity, Box::new(4)).unwrap()
    else {
        panic!()
    };
    let ProductionStart::Joined {
        production,
        unused_input,
    } = map.begin_shared(&identity, Box::new(5)).unwrap()
    else {
        panic!()
    };
    assert_eq!(*unused_input, 5);
    assert_eq!(budget.snapshot().active, 1);
    drop(first);
    let demand = map.demand(&identity).unwrap();
    assert_eq!(demand.snapshot().consumers(), 1);
    assert_eq!(demand.snapshot().urgency(), Some(20));
    let SubmissionOutcome::Rejected(rejection) =
        attempt.submission.submit(ProviderDecision::Rejected)
    else {
        panic!()
    };
    assert_eq!(rejection.into_parts().0.map(|value| *value), Some(4));
    assert!(matches!(production.claim(), ProductionClaim::Rejected));
    assert_eq!(budget.snapshot().active, 0);
    assert_eq!(demand.snapshot().consumers(), 1);
    let ProductionStart::Started(retry) = map.begin_shared(&identity, ()).unwrap() else {
        panic!()
    };
    retry.submission.submit(|mut root| {
        root.complete(Ok(9)).unwrap();
        drop(root);
        ProviderDecision::Accepted
    });
    // Completed but unclaimed results continue to coalesce without reserving.
    assert!(matches!(
        map.begin_shared(&identity, ()).unwrap(),
        ProductionStart::Joined { .. }
    ));
    assert!(matches!(
        retry.production.claim(),
        ProductionClaim::Ready(Ok(9))
    ));
    let ProductionStart::Started(next) = map.begin_shared(&identity, ()).unwrap() else {
        panic!()
    };
    drop(next.submission);
    assert!(matches!(next.production.claim(), ProductionClaim::Aborted));
    drop(second);
    let error = match map.begin_shared(&identity, 123) {
        Err(error) => error,
        Ok(_) => panic!(),
    };
    assert_eq!(error.reason(), StartRejectionReason::NoDemand);
    assert_eq!(error.into_inputs(), 123);
}

#[test]
fn consumed_rejection_holds_production_allowance_but_not_provider_slot_or_demand() {
    let mut cache = Cache::<_, ()>::new();
    let identity = cache.ensure("key");
    let budget = Admission::new(CountLimit::new(1));
    let providers = Admission::new(CountLimit::new(1));
    let accounting = AccountingDomain::new();
    let input = Backing::try_new(vec![0u8; 8], &accounting, 8, AllocationClass::Temporary).unwrap();
    let mut map = ProductionMap::<_, (), ()>::new(budget.clone());
    let consumer = map.interest(identity.clone(), 0, 1);
    let ProductionStart::Started(attempt) = map.begin_shared(&identity, input).unwrap() else {
        panic!()
    };
    let mut consumed = None;
    let slot = providers.try_acquire().unwrap();
    let SubmissionOutcome::Rejected(rejection) = attempt.submission.submit(|mut root| {
        consumed = root.consume_inputs().unwrap();
        drop(slot);
        ProviderDecision::Rejected(root)
    }) else {
        panic!()
    };
    assert!(rejection.into_parts().0.is_none());
    assert_eq!(providers.snapshot().active, 0);
    assert_eq!(budget.snapshot().active, 1);
    assert_eq!(accounting.snapshot().temporary_bytes, 8);
    let error = match map.begin_shared(&identity, 41) {
        Err(error) => error,
        Ok(_) => panic!(),
    };
    assert_eq!(error.reason(), StartRejectionReason::AtCapacity);
    assert_eq!(error.into_inputs(), 41);
    assert_eq!(
        map.current(&identity).unwrap().snapshot().phase,
        ProductionPhase::Rejected
    );
    drop(consumer);
    assert!(map.remove(&identity));
    drop(map);
    assert_eq!(budget.snapshot().active, 1);
    drop(consumed);
    assert_eq!(budget.snapshot().active, 0);
    assert_eq!(accounting.snapshot().temporary_bytes, 0);
}

#[test]
fn independent_candidates_and_recreated_identities_do_not_replace_shared_work() {
    let mut cache = Cache::<_, ()>::new();
    let identity = cache.ensure("key");
    let budget = Admission::new(CountLimit::new(3));
    let mut map = ProductionMap::<_, u8, ()>::new(budget.clone());
    let old_interest = map.interest(identity.clone(), 0, 1);
    let ProductionStart::Started(shared) = map.begin_shared(&identity, ()).unwrap() else {
        panic!()
    };
    let candidate = map.begin_candidate(&identity, ()).unwrap();
    candidate.submission.submit(|mut root| {
        root.complete(Ok(7)).unwrap();
        ProviderDecision::Accepted
    });
    assert!(matches!(
        candidate.production.claim(),
        ProductionClaim::Ready(Ok(7))
    ));
    assert_eq!(
        map.current(&identity).unwrap().snapshot().phase,
        ProductionPhase::Prepared
    );
    cache.remove(&"key");
    let replacement = cache.ensure("key");
    assert!(map.current(&replacement).is_none());
    assert!(matches!(
        map.begin_candidate(&replacement, ())
            .err()
            .unwrap()
            .reason(),
        StartRejectionReason::NoDemand
    ));
    assert!(map.remove(&identity));
    let new_interest = map.interest(identity.clone(), 0, 5);
    drop(old_interest);
    assert_eq!(map.demand(&identity).unwrap().snapshot().consumers(), 1);
    // Old accepted discovery can still settle after association removal.
    let mut child = None;
    shared.submission.submit(|mut root| {
        child = Some(root.child(()).unwrap());
        root.complete(Ok(3)).unwrap();
        ProviderDecision::Accepted
    });
    drop(new_interest);
    drop(map);
    assert!(matches!(
        shared.production.claim(),
        ProductionClaim::Pending
    ));
    assert_eq!(budget.snapshot().active, 1);
    drop(child);
    assert_eq!(budget.snapshot().active, 0);
    assert!(matches!(
        shared.production.claim(),
        ProductionClaim::Ready(Ok(3))
    ));
}

#[test]
fn abort_with_escaped_producer_drops_staged_output_outside_lock_and_retains_allowance() {
    struct Output {
        observer: Production<Output, ()>,
        budget: Admission,
        drops: Rc<Cell<usize>>,
    }
    impl Drop for Output {
        fn drop(&mut self) {
            assert_eq!(self.observer.snapshot().phase, ProductionPhase::Aborted);
            assert_eq!(self.budget.snapshot().active, 1);
            self.drops.set(self.drops.get() + 1);
        }
    }
    let budget = Admission::new(CountLimit::new(1));
    let (production, submission) =
        Production::<Output, ()>::prepare_with_permit((), budget.try_acquire().unwrap());
    let mut escaped = None;
    let drops = Rc::new(Cell::new(0));
    let panic = catch_unwind(AssertUnwindSafe(|| {
        submission.submit(|mut root| {
            root.complete(Ok(Output {
                observer: production.clone(),
                budget: budget.clone(),
                drops: Rc::clone(&drops),
            }))
            .unwrap();
            escaped = Some(root);
            panic!("provider panic after escaped root");
        })
    }))
    .err()
    .expect("provider must panic");
    assert_eq!(
        panic.downcast_ref::<&str>(),
        Some(&"provider panic after escaped root")
    );
    assert_eq!(
        drops.get(),
        1,
        "aborting must actually destroy the staged output"
    );
    assert_eq!(budget.snapshot().active, 1);
    drop(escaped);
    assert_eq!(budget.snapshot().active, 0);
    assert!(matches!(production.claim(), ProductionClaim::Aborted));
}

#[test]
fn abandoned_attempt_retries_without_reusing_old_observer() {
    let mut cache = Cache::<_, ()>::new();
    let identity = cache.ensure(1);
    let budget = Admission::new(CountLimit::new(1));
    let mut map = ProductionMap::<_, (), ()>::new(budget.clone());
    let _interest = map.interest(identity.clone(), 0, 0);
    let ProductionStart::Started(first) = map.begin_shared(&identity, ()).unwrap() else {
        panic!()
    };
    first.submission.submit(|_| ProviderDecision::Accepted);
    assert_eq!(budget.snapshot().active, 0);
    let ProductionStart::Started(second) = map.begin_shared(&identity, ()).unwrap() else {
        panic!()
    };
    assert!(matches!(
        first.production.claim(),
        ProductionClaim::Abandoned
    ));
    assert!(matches!(
        second.production.claim(),
        ProductionClaim::Pending
    ));
    drop(second.submission);
    assert_eq!(budget.snapshot().active, 0);
}

#[test]
fn unsubmitted_input_panic_aborts_and_releases_allowance_after_cleanup() {
    struct Input {
        budget: Admission,
    }
    impl Drop for Input {
        fn drop(&mut self) {
            assert_eq!(self.budget.snapshot().active, 1);
            panic!("unsubmitted input cleanup");
        }
    }
    let budget = Admission::new(CountLimit::new(1));
    let (production, submission) = Production::<(), ()>::prepare_with_permit(
        Input {
            budget: budget.clone(),
        },
        budget.try_acquire().unwrap(),
    );
    let panic = catch_unwind(AssertUnwindSafe(|| drop(submission))).unwrap_err();
    assert_eq!(
        panic.downcast_ref::<&str>(),
        Some(&"unsubmitted input cleanup")
    );
    assert_eq!(budget.snapshot().active, 0);
    assert!(matches!(production.claim(), ProductionClaim::Aborted));
}
