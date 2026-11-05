use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use engine_worker::{
    DependencyPolicy, ExecutionClass, Outcome, OwnedLimits, Runtime, RuntimeConfig, SpawnError,
    SpawnOptions, Task, TaskStatus, WaitError, WorkerConfig,
};

#[test]
fn sealed_group_dependencies_cover_early_late_failure_and_cross_class_activation() {
    const TIMEOUT: Duration = Duration::from_secs(5);
    // One contract, four member outcomes: success, typed failure, panic, cancel.
    for variant in 0..4 {
        let config = RuntimeConfig::new(3, [WorkerConfig::new(1); 3]).unwrap();
        let limits = OwnedLimits::new(12, 12, [8; 3], [2; 3]).unwrap();
        let mut runtime = Runtime::builder(config)
            .with_owned_limits(limits)
            .build()
            .unwrap();
        let low = runtime.lane(ExecutionClass::Low);
        let high = runtime.lane(ExecutionClass::High);
        let group = low.group().unwrap();
        let completion = group.completion();
        let count = Arc::new(AtomicUsize::new(0));
        let ran = Arc::clone(&count);
        let (dependent, _) = high
            .try_spawn_after(
                SpawnOptions::default(),
                std::slice::from_ref(&completion),
                DependencyPolicy::SuccessOnly,
                move || {
                    ran.fetch_add(1, Ordering::SeqCst);
                },
            )
            .unwrap();
        // A directly self-dependent member must reject before changing membership.
        assert_eq!(
            low.try_spawn_after_in(
                &group,
                SpawnOptions::default(),
                std::slice::from_ref(&completion),
                DependencyPolicy::OutcomeAware,
                || (),
            )
            .err()
            .unwrap()
            .reason,
            SpawnError::InvalidGroup
        );

        let (release_tx, release_rx) = mpsc::channel();
        let (gate, _) = high
            .try_spawn(SpawnOptions::default(), move || {
                release_rx.recv_timeout(TIMEOUT).unwrap();
            })
            .unwrap();
        let (member, cancel) = low
            .try_spawn_after_fallible_in(
                &group,
                SpawnOptions::default(),
                &[gate.completion()],
                DependencyPolicy::SuccessOnly,
                move || -> Result<(), &'static str> {
                    match variant {
                        1 => Err("failed"),
                        2 => panic!("member panic"),
                        _ => Ok(()),
                    }
                },
            )
            .unwrap();
        let observed = completion.clone();
        let (aware, _) = high
            .try_spawn_after(
                SpawnOptions::default(),
                std::slice::from_ref(&completion),
                DependencyPolicy::OutcomeAware,
                move || observed.status().unwrap(),
            )
            .unwrap();
        if variant == 3 {
            cancel.cancel();
        }
        assert_eq!(completion.status(), None); // Even a cancelled member cannot seal the wave.
        assert_eq!(dependent.status(), None);
        group.seal();
        group.seal(); // Publication is once-only.
        release_tx.send(()).unwrap();
        let expected = if variant == 0 {
            TaskStatus::Succeeded
        } else {
            TaskStatus::PrerequisiteFailed
        };
        assert_eq!(completion.wait_timeout(TIMEOUT).unwrap(), Some(expected));
        group.wait_helping().unwrap();
        assert!(member.status().is_some());
        assert_eq!(
            dependent.completion().wait_timeout(TIMEOUT).unwrap(),
            Some(expected)
        );
        let mut aware = aware;
        assert_eq!(
            aware.completion().wait_timeout(TIMEOUT).unwrap(),
            Some(TaskStatus::Succeeded)
        );
        assert_eq!(aware.try_take(), Some(Outcome::Success(expected)));
        assert_eq!(count.load(Ordering::SeqCst), usize::from(variant == 0));

        let (late, _) = high
            .try_spawn_after(
                SpawnOptions::default(),
                &[completion],
                DependencyPolicy::OutcomeAware,
                || 7,
            )
            .unwrap();
        assert_eq!(
            late.completion().wait_timeout(TIMEOUT).unwrap(),
            Some(TaskStatus::Succeeded)
        );
        let empty = low.group().unwrap();
        let empty_done = empty.completion();
        assert_eq!(empty_done.status(), None);
        empty.seal();
        assert_eq!(empty_done.status(), Some(TaskStatus::Succeeded));
        runtime.shutdown().unwrap();
    }
}

#[test]
fn ready_unique_result_moves_once_and_completion_survives_take() {
    let mut task = Task::ready(String::from("prepared"));
    let completion = task.completion();

    assert_eq!(completion.status(), Some(TaskStatus::Succeeded));
    assert_eq!(
        task.try_result().as_deref(),
        Some(&Outcome::Success(String::from("prepared")))
    );
    assert_eq!(
        task.try_take(),
        Some(Outcome::Success(String::from("prepared")))
    );
    assert_eq!(task.try_take(), None);
    assert!(task.try_result().is_none());
    assert_eq!(completion.wait().unwrap(), TaskStatus::Succeeded);

    let config = RuntimeConfig::new(3, [WorkerConfig::new(1); 3]).unwrap();
    let limits = OwnedLimits::new(16, 16, [8; 3], [1; 3]).unwrap();
    let mut runtime = Runtime::builder(config)
        .with_owned_limits(limits)
        .build()
        .unwrap();
    let lane = runtime.lane(ExecutionClass::Low);
    let rejected = task
        .try_then(&lane, SpawnOptions::default(), |value| value.len())
        .err()
        .expect("a consumed unique result cannot feed a successor");
    assert_eq!(rejected.reason, SpawnError::Consumed);
    assert_eq!(rejected.input.status(), Some(TaskStatus::Succeeded));
    assert_eq!((rejected.operation)(String::from("other")), 5);

    let mut taken = Task::ready(7usize);
    assert_eq!(taken.try_take(), Some(Outcome::Success(7)));
    let rejected = taken
        .try_then_outcome(&lane, SpawnOptions::default(), |outcome| outcome)
        .err()
        .expect("a consumed outcome cannot feed an outcome-aware successor");
    assert_eq!(rejected.reason, SpawnError::Consumed);

    let shared = rejected.input.into_shared();
    assert!(shared.try_result().is_none());
    let rejected = shared
        .clone()
        .try_then(&lane, SpawnOptions::default(), |input| input.status())
        .err()
        .expect("a shared handle converted after take has no payload");
    assert_eq!(rejected.reason, SpawnError::Consumed);
    let rejected = shared
        .try_then_outcome(&lane, SpawnOptions::default(), |input| input.status())
        .err()
        .expect("a shared handle converted after take has no outcome");
    assert_eq!(rejected.reason, SpawnError::Consumed);
    runtime.shutdown().unwrap();
}

#[test]
fn unique_task_transfers_send_payload_that_is_not_sync() {
    let task = Task::ready(Cell::new(23usize));
    let value = thread::spawn(move || {
        let mut task = task;
        match task.try_take() {
            Some(Outcome::Success(value)) => value.get(),
            _ => panic!("expected unique result"),
        }
    })
    .join()
    .unwrap();
    assert_eq!(value, 23);
}

#[test]
fn explicit_fallible_ready_result_preserves_typed_error_and_marks_status() {
    let mut ordinary = Task::ready(Result::<usize, &'static str>::Err("ordinary"));
    let mut fallible = Task::ready_fallible(Result::<usize, &'static str>::Err("failed"));

    assert_eq!(ordinary.status(), Some(TaskStatus::Succeeded));
    assert_eq!(fallible.status(), Some(TaskStatus::ApplicationFailed));
    assert_eq!(ordinary.try_take(), Some(Outcome::Success(Err("ordinary"))));
    assert_eq!(fallible.try_take(), Some(Outcome::Success(Err("failed"))));
}

#[test]
fn shared_conversion_retains_the_same_payload_without_cloning_it() {
    let value = Arc::new(17usize);
    let shared = Task::ready(Arc::clone(&value)).into_shared();
    let second = shared.clone();

    let first_result = shared.try_result().unwrap();
    let second_result = second.try_result().unwrap();
    let first = match &*first_result {
        Outcome::Success(first) => Arc::as_ptr(first),
        _ => panic!("expected shared success"),
    };
    match &*second_result {
        Outcome::Success(second) => {
            assert_eq!(first, Arc::as_ptr(second));
            assert_eq!(Arc::strong_count(&value), 2);
        }
        _ => panic!("expected shared success"),
    }
}

#[test]
fn shared_work_is_reused_before_and_after_completion_and_outlives_runtime() {
    let config = RuntimeConfig::new(3, [WorkerConfig::new(1); 3]).unwrap();
    let limits = OwnedLimits::new(16, 16, [8; 3], [1; 3]).unwrap();
    let mut runtime = Runtime::builder(config)
        .with_owned_limits(limits)
        .build()
        .unwrap();
    let lane = runtime.lane(ExecutionClass::Low);
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let producer_calls = Arc::new(AtomicUsize::new(0));
    let calls = Arc::clone(&producer_calls);
    let (task, _) = lane
        .try_spawn(SpawnOptions::default(), move || {
            calls.fetch_add(1, Ordering::Relaxed);
            started_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            29usize
        })
        .unwrap();
    started_rx.recv().unwrap();
    let shared = task.into_shared();
    assert_eq!(shared.status(), None);
    assert!(shared.try_result().is_none());

    // Several consumers retain one in-flight producer, including across lanes.
    let mut consumers = Vec::new();
    for class in [ExecutionClass::Mid, ExecutionClass::High] {
        let (consumer, _) = shared
            .clone()
            .try_then(&runtime.lane(class), SpawnOptions::default(), |input| {
                input.try_result().unwrap()
            })
            .unwrap();
        consumers.push(consumer);
    }

    release_tx.send(()).unwrap();
    assert_eq!(shared.completion().wait().unwrap(), TaskStatus::Succeeded);
    let first = shared.try_result().unwrap();
    let second = shared.clone().try_result().unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(&*first, &Outcome::Success(29));

    // A later consumer reuses the completed value without restarting its job.
    let (later, _) = shared
        .clone()
        .try_then(&lane, SpawnOptions::default(), |input| {
            input.try_result().unwrap()
        })
        .unwrap();
    consumers.push(later);
    for mut consumer in consumers {
        assert_eq!(
            consumer
                .completion()
                .wait_timeout(Duration::from_secs(5))
                .unwrap(),
            Some(TaskStatus::Succeeded)
        );
        let Some(Outcome::Success(observed)) = consumer.try_take() else {
            panic!("consumer did not retain the shared result");
        };
        assert!(Arc::ptr_eq(&first, &observed));
    }
    runtime.shutdown().unwrap();
    drop(runtime);
    assert_eq!(producer_calls.load(Ordering::Relaxed), 1);
    assert!(Arc::ptr_eq(&first, &shared.try_result().unwrap()));
}

#[test]
fn passive_wait_rejects_participating_cpu_context() {
    let config = RuntimeConfig::new(3, [WorkerConfig::new(1); 3]).unwrap();
    let mut runtime = Runtime::builder(config).build().unwrap();
    let lane = runtime.lane(ExecutionClass::Low);
    let completion = Task::ready(1usize).completion();
    let worker_completion = completion.clone();

    let (worker, caller) = lane
        .join(
            move || worker_completion.wait(),
            move || completion.wait_timeout(Duration::ZERO),
        )
        .unwrap();
    assert_eq!(worker.unwrap(), Err(WaitError::ExecutionContext));
    assert_eq!(caller.unwrap(), Err(WaitError::ExecutionContext));
    runtime.shutdown().unwrap();
}

#[test]
fn typed_continuations_preserve_ownership_and_classify_failure_explicitly() {
    let config = RuntimeConfig::new(3, [WorkerConfig::new(1); 3]).unwrap();
    let limits = OwnedLimits::new(16, 16, [8; 3], [1; 3]).unwrap();
    let mut runtime = Runtime::builder(config)
        .with_owned_limits(limits)
        .build()
        .unwrap();
    let lane = runtime.lane(ExecutionClass::Mid);

    let (mut successor, _) = Task::ready(5usize)
        .try_then(&lane, SpawnOptions::default(), |value| value * 3)
        .unwrap();
    assert_eq!(
        successor.completion().wait().unwrap(),
        TaskStatus::Succeeded
    );
    assert_eq!(successor.try_take(), Some(Outcome::Success(15)));

    let (mut suppressed, _) = Task::<usize>::ready_outcome(Outcome::Cancelled)
        .try_then(&lane, SpawnOptions::default(), |value| value + 1)
        .unwrap();
    assert_eq!(
        suppressed.completion().wait().unwrap(),
        TaskStatus::PrerequisiteFailed
    );
    assert_eq!(suppressed.try_take(), Some(Outcome::PrerequisiteFailed));

    let (mut fallback, _) = Task::<usize>::ready_outcome(Outcome::Cancelled)
        .try_then_outcome(&lane, SpawnOptions::default(), |outcome| {
            matches!(outcome, Outcome::Cancelled)
        })
        .unwrap();
    assert_eq!(fallback.completion().wait().unwrap(), TaskStatus::Succeeded);
    assert_eq!(fallback.try_take(), Some(Outcome::Success(true)));

    // Explicit fallibility composes with both ownership modes and input policies.
    for form in ["unique", "unique_outcome", "shared", "shared_outcome"] {
        for fail in [false, true] {
            let operation = move |_| {
                if fail {
                    Err(String::from("prepare failed"))
                } else {
                    Ok(11usize)
                }
            };
            let (mut stage, _) = match form {
                "unique" => Task::ready(7usize)
                    .try_then_fallible(&lane, SpawnOptions::default(), operation)
                    .unwrap(),
                "unique_outcome" => Task::<usize>::ready_outcome(Outcome::Cancelled)
                    .try_then_outcome_fallible(&lane, SpawnOptions::default(), move |outcome| {
                        assert!(matches!(outcome, Outcome::Cancelled));
                        if fail {
                            Err(String::from("prepare failed"))
                        } else {
                            Ok(11usize)
                        }
                    })
                    .unwrap(),
                "shared" => Task::ready(7usize)
                    .into_shared()
                    .try_then_fallible(&lane, SpawnOptions::default(), move |input| {
                        assert_eq!(&*input.try_result().unwrap(), &Outcome::Success(7));
                        if fail {
                            Err(String::from("prepare failed"))
                        } else {
                            Ok(11usize)
                        }
                    })
                    .unwrap(),
                "shared_outcome" => Task::<usize>::ready_outcome(Outcome::Cancelled)
                    .into_shared()
                    .try_then_outcome_fallible(&lane, SpawnOptions::default(), move |input| {
                        assert_eq!(input.status(), Some(TaskStatus::Cancelled));
                        if fail {
                            Err(String::from("prepare failed"))
                        } else {
                            Ok(11usize)
                        }
                    })
                    .unwrap(),
                _ => unreachable!(),
            };
            let calls = Arc::new(AtomicUsize::new(0));
            let invoked = Arc::clone(&calls);
            let (next, _) = lane
                .try_spawn_after(
                    SpawnOptions::default(),
                    &[stage.completion()],
                    engine_worker::DependencyPolicy::SuccessOnly,
                    move || {
                        invoked.fetch_add(1, Ordering::Relaxed);
                    },
                )
                .unwrap();
            assert_eq!(
                next.completion().wait().unwrap(),
                if fail {
                    TaskStatus::PrerequisiteFailed
                } else {
                    TaskStatus::Succeeded
                },
                "{form}"
            );
            assert_eq!(
                stage.status(),
                Some(if fail {
                    TaskStatus::ApplicationFailed
                } else {
                    TaskStatus::Succeeded
                }),
                "{form}"
            );
            assert_eq!(
                stage.try_take(),
                Some(Outcome::Success(if fail {
                    Err(String::from("prepare failed"))
                } else {
                    Ok(11)
                })),
                "{form}"
            );
            assert_eq!(calls.load(Ordering::Relaxed), usize::from(!fail), "{form}");
        }
    }

    // Ordinary continuations still treat Result as a value, not an error policy.
    let (ordinary, _) = Task::ready(1usize)
        .try_then(&lane, SpawnOptions::default(), |_| {
            Err::<usize, _>("ordinary value")
        })
        .unwrap();
    assert_eq!(ordinary.completion().wait().unwrap(), TaskStatus::Succeeded);

    runtime.shutdown().unwrap();

    let rejected = Task::ready(8usize)
        .try_then(&lane, SpawnOptions::default(), |value| value * 2)
        .err()
        .expect("closed runtime rejects the continuation");
    assert_eq!(rejected.reason, SpawnError::Closed);
    let mut returned = rejected.input;
    assert_eq!(returned.try_take(), Some(Outcome::Success(8)));
    assert_eq!((rejected.operation)(4), 8);
    let capture = String::from("retained error");
    let rejected = Task::ready(9usize)
        .try_then_fallible(&lane, SpawnOptions::default(), move |_| {
            Err::<usize, _>(capture)
        })
        .err()
        .expect("closed runtime returns fallible continuation inputs");
    assert_eq!(rejected.reason, SpawnError::Closed);
    let mut input = rejected.input;
    assert_eq!(input.try_take(), Some(Outcome::Success(9)));
    assert_eq!((rejected.operation)(9), Err(String::from("retained error")));
}

#[test]
fn successor_attached_while_predecessor_runs_activates_once() {
    let config = RuntimeConfig::new(3, [WorkerConfig::new(1); 3]).unwrap();
    let limits = OwnedLimits::new(16, 16, [8; 3], [1; 3]).unwrap();
    let mut runtime = Runtime::builder(config)
        .with_owned_limits(limits)
        .build()
        .unwrap();
    let lane = runtime.lane(ExecutionClass::High);
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let calls = Arc::new(AtomicUsize::new(0));
    let (task, _) = lane
        .try_spawn(SpawnOptions::default(), move || {
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            9usize
        })
        .unwrap();
    started_rx.recv().unwrap();
    let successor_calls = Arc::clone(&calls);
    let (mut successor, _) = task
        .try_then(&lane, SpawnOptions::default(), move |value| {
            successor_calls.fetch_add(1, Ordering::SeqCst);
            value + 1
        })
        .unwrap();

    release_tx.send(()).unwrap();
    assert_eq!(
        successor.completion().wait().unwrap(),
        TaskStatus::Succeeded
    );
    assert_eq!(successor.try_take(), Some(Outcome::Success(10)));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    runtime.shutdown().unwrap();
}
