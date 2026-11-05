use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc;
use std::thread;

use engine_cache::{
    Admission, CompletionRejectionReason, CountLimit, Production, ProductionClaim, ProductionError,
    ProductionPhase, ProviderDecision, SubmissionOutcome,
};

#[test]
fn inline_completion_waits_for_acceptance_and_final_producer_access() {
    for retain_root in [false, true] {
        let budget = Admission::new(CountLimit::new(1));
        let (production, submission) = Production::<u8, &str>::prepare_with_permit(
            String::from("input"),
            budget.try_acquire().unwrap(),
        );
        let observer = production.clone();
        let mut retained = None;
        let result = submission.submit(|mut producer| {
            producer.complete(Ok(7)).unwrap();
            assert_eq!(observer.snapshot().phase, ProductionPhase::Prepared);
            assert!(observer.snapshot().completion_recorded);
            assert!(matches!(observer.claim(), ProductionClaim::Pending));
            assert_eq!(producer.inputs().unwrap(), "input");
            if retain_root {
                retained = Some(producer);
            } else {
                drop(producer);
            }
            // Even an inline callback that has ended all access cannot publish or
            // reopen admission before the provider's actual acceptance decision.
            assert!(matches!(observer.claim(), ProductionClaim::Pending));
            assert_eq!(budget.snapshot().active, 1);
            ProviderDecision::Accepted
        });
        assert!(matches!(result, SubmissionOutcome::Accepted));
        assert_eq!(production.snapshot().phase, ProductionPhase::Accepted);
        if retain_root {
            assert!(matches!(production.claim(), ProductionClaim::Pending));
            assert_eq!(budget.snapshot().active, 1);
        }
        drop(retained);
        assert_eq!(budget.snapshot().active, 0);
        assert!(matches!(production.claim(), ProductionClaim::Ready(Ok(7))));
        assert!(matches!(production.claim(), ProductionClaim::Taken));
    }
}

#[test]
fn rejection_returns_unconsumed_input_and_inline_outcome_without_accepting_failure() {
    let (production, submission) = Production::<u8, &str>::prepare(Box::new(19));
    let mut address = std::ptr::null();
    let rejected = match submission.submit(|mut producer| {
        address = &**producer.inputs().unwrap() as *const i32;
        producer.complete(Err("failure before decision")).unwrap();
        ProviderDecision::Rejected(producer)
    }) {
        SubmissionOutcome::Rejected(rejected) => rejected,
        _ => panic!(),
    };
    assert_eq!(production.snapshot().phase, ProductionPhase::Rejected);
    assert_eq!(production.snapshot().active_accesses, 0);
    assert!(matches!(production.claim(), ProductionClaim::Rejected));
    let (inputs, staged) = rejected.into_parts();
    let inputs = inputs.unwrap();
    assert_eq!(&*inputs as *const i32, address);
    assert_eq!(staged, Some(Err("failure before decision")));
}

#[test]
fn consumed_input_and_nested_children_survive_root_completion_and_rejection() {
    for accepted in [false, true] {
        let (production, submission) = Production::<u8, &str>::prepare(String::from("consumed"));
        let mut consumed = None;
        let mut nested = None;
        let result = submission.submit(|mut producer| {
            let input = producer.consume_inputs().unwrap().unwrap();
            assert!(producer.inputs().is_none());
            assert!(producer.consume_inputs().unwrap().is_none());
            nested = Some(input.child(vec![1, 2, 3]).unwrap().child(()).unwrap());
            consumed = Some(input);
            producer.complete(Err("accepted failure")).unwrap();
            if accepted {
                drop(producer);
                ProviderDecision::Accepted
            } else {
                ProviderDecision::Rejected(producer)
            }
        });
        assert_eq!(production.snapshot().active_accesses, 2);
        assert_eq!(consumed.as_ref().unwrap().inputs().unwrap(), "consumed");
        if accepted {
            assert!(matches!(result, SubmissionOutcome::Accepted));
            assert!(matches!(production.claim(), ProductionClaim::Pending));
            // Discover more work after root release, then release its parent.
            let parent = nested.take().unwrap();
            nested = Some(parent.child(()).unwrap());
            drop(parent);
            assert_eq!(production.snapshot().active_accesses, 2);
        } else {
            let SubmissionOutcome::Rejected(rejected) = result else {
                panic!()
            };
            let (unconsumed, staged) = rejected.into_parts();
            assert!(unconsumed.is_none());
            assert_eq!(staged, Some(Err("accepted failure")));
            let error = match consumed.as_ref().unwrap().child(23) {
                Err(error) => error,
                Ok(_) => panic!("settled attempt cannot discover more children"),
            };
            assert_eq!(error.reason(), ProductionError::Settled);
            assert_eq!(error.into_inputs(), 23);
        }
        drop(consumed);
        assert_eq!(production.snapshot().active_accesses, 1);
        drop(nested);
        assert_eq!(production.snapshot().active_accesses, 0);
        if accepted {
            assert!(matches!(
                production.claim(),
                ProductionClaim::Ready(Err("accepted failure"))
            ));
        } else {
            assert!(matches!(production.claim(), ProductionClaim::Rejected));
        }
    }
}

#[test]
fn duplicate_and_late_completion_preserve_original_candidate() {
    let (production, submission) = Production::<Box<u8>, Box<str>>::prepare(());
    let mut escaped = None;
    let panic = catch_unwind(AssertUnwindSafe(|| {
        submission.submit(|mut producer| {
            producer.complete(Ok(Box::new(1))).unwrap();
            let candidate = Box::new(2);
            let address = &*candidate as *const u8;
            let error = producer.complete(Ok(candidate)).unwrap_err();
            assert_eq!(error.reason(), CompletionRejectionReason::AlreadyCompleted);
            let candidate = error.into_outcome().unwrap();
            assert_eq!(&*candidate as *const u8, address);
            escaped = Some(producer);
            panic!("provider panics after transferring root");
        })
    }))
    .err()
    .expect("provider must panic");
    assert_eq!(
        panic.downcast_ref::<&str>(),
        Some(&"provider panics after transferring root")
    );
    assert_eq!(production.snapshot().phase, ProductionPhase::Aborted);
    assert_eq!(production.snapshot().active_accesses, 1);
    let error = escaped
        .as_mut()
        .unwrap()
        .complete(Err("late failure".into()))
        .unwrap_err();
    assert_eq!(error.reason(), CompletionRejectionReason::Settled);
    assert_eq!(error.into_outcome().unwrap_err().as_ref(), "late failure");
    drop(escaped);
    assert_eq!(production.snapshot().active_accesses, 0);
    assert!(matches!(production.claim(), ProductionClaim::Aborted));
}

#[test]
fn foreign_rejection_preserves_foreign_producer_and_aborts_original_attempt() {
    let (foreign, foreign_submission) = Production::<u8, ()>::prepare(11);
    let mut foreign_root = None;
    foreign_submission.submit(|producer| {
        foreign_root = Some(producer);
        ProviderDecision::Accepted
    });
    let (original, submission) = Production::<u8, ()>::prepare(22);
    let returned = match submission.submit(|producer| {
        drop(producer);
        ProviderDecision::Rejected(foreign_root.take().unwrap())
    }) {
        SubmissionOutcome::InvalidRejection(producer) => producer,
        _ => panic!(),
    };
    assert_eq!(returned.inputs(), Some(&11));
    assert_eq!(original.snapshot().phase, ProductionPhase::Aborted);
    assert_eq!(foreign.snapshot().phase, ProductionPhase::Accepted);
    assert_eq!(foreign.snapshot().active_accesses, 1);
    drop(returned);
    assert!(matches!(foreign.claim(), ProductionClaim::Abandoned));
}

#[test]
fn explicit_child_final_access_controls_cross_thread_settlement() {
    let (production, submission) = Production::<u8, ()>::prepare(());
    let (handoff, received) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let worker = thread::spawn(move || {
        let child = received.recv().unwrap();
        released.recv().unwrap();
        drop(child);
    });
    submission.submit(|mut producer| {
        assert!(handoff.send(producer.child(vec![1u8; 8]).unwrap()).is_ok());
        producer.complete(Ok(9)).unwrap();
        drop(producer);
        ProviderDecision::Accepted
    });
    assert_eq!(production.snapshot().active_accesses, 1);
    assert!(matches!(production.claim(), ProductionClaim::Pending));
    release.send(()).unwrap();
    worker.join().unwrap();
    assert!(matches!(production.claim(), ProductionClaim::Ready(Ok(9))));
}

#[test]
fn input_cleanup_can_reenter_observation_and_panics_still_release_access() {
    struct Input {
        production: Production<u8, ()>,
        panic: bool,
    }
    impl Drop for Input {
        fn drop(&mut self) {
            assert_eq!(self.production.snapshot().active_accesses, 1);
            assert!(matches!(self.production.claim(), ProductionClaim::Pending));
            assert!(!self.panic, "input destructor panic");
        }
    }
    // Input wraps observation of its own attempt after preparation, exercising
    // arbitrary cleanup while the last lease remains held.
    let (production, submission) = Production::<u8, ()>::prepare(None::<Input>);
    let mut retained = None;
    submission.submit(|mut producer| {
        *producer.inputs_mut().unwrap() = Some(Input {
            production: production.clone(),
            panic: true,
        });
        producer.complete(Ok(3)).unwrap();
        retained = Some(producer);
        ProviderDecision::Accepted
    });
    let panic = catch_unwind(AssertUnwindSafe(|| drop(retained))).unwrap_err();
    assert_eq!(
        panic.downcast_ref::<&str>(),
        Some(&"input destructor panic")
    );
    assert_eq!(production.snapshot().active_accesses, 0);
    assert!(matches!(production.claim(), ProductionClaim::Ready(Ok(3))));
}
