//! Scheduling work that an owner holds.
#![expect(clippy::unwrap_used, reason = "a failed test is the intended report")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use engine_worker::{
    CallerEligibility, DependencyPolicy, ExecutionClass, Outcome, OwnedLimits, Runtime,
    RuntimeConfig, SpawnError, SpawnOptions, TaskStatus, WorkerConfig,
};

fn runtime(runnable: usize, handoff: usize) -> Runtime {
    let config = RuntimeConfig::new(3, [WorkerConfig::new(1); 3]).unwrap();
    let limits = OwnedLimits::new(16, 16, [runnable; 3], [handoff; 3]).unwrap();
    Runtime::builder(config)
        .with_owned_limits(limits)
        .build()
        .unwrap()
}

fn caller_eligible() -> SpawnOptions {
    SpawnOptions {
        eligibility: CallerEligibility::CallerEligible,
    }
}

const TIMEOUT: Duration = Duration::from_secs(10);

#[test]
fn runnable_saturation_inline_is_explicit_and_try_spawn_never_runs_inline() {
    struct Release(mpsc::Sender<()>);
    impl Drop for Release {
        fn drop(&mut self) {
            let _ = self.0.send(());
        }
    }
    let mut runtime = runtime(1, 1);
    let lane = runtime.lane(ExecutionClass::High);
    let (started_send, started_recv) = mpsc::channel();
    let (release_send, release_recv) = mpsc::channel();
    let release = Release(release_send);
    let (mut first, _) = lane
        .try_spawn(SpawnOptions::default(), move || {
            started_send.send(()).unwrap();
            release_recv.recv_timeout(TIMEOUT).unwrap();
            1
        })
        .unwrap();
    started_recv.recv_timeout(TIMEOUT).unwrap();

    let (mut queued, _) = lane.try_spawn(caller_eligible(), || 2).unwrap();
    let rejected = lane
        .try_spawn(caller_eligible(), || 3)
        .err()
        .expect("runnable capacity should reject");
    assert_eq!(rejected.reason, SpawnError::Full);
    let caller = thread::current().id();
    let (mut inline, _) = lane
        .submit_or_run(caller_eligible(), move || thread::current().id())
        .unwrap();
    assert_eq!(inline.try_take(), Some(Outcome::Success(caller)));
    let group = lane.group().unwrap();
    let (mut fallible, _) = lane
        .submit_or_run_fallible_in(&group, caller_eligible(), move || {
            assert_eq!(thread::current().id(), caller);
            Err::<usize, _>("inline failure")
        })
        .unwrap();
    group.seal();
    assert_eq!(fallible.status(), Some(TaskStatus::ApplicationFailed));
    assert_eq!(
        fallible.try_take(),
        Some(Outcome::Success(Err("inline failure")))
    );
    assert!(group.is_complete());
    let (mut succeeded, _) = lane
        .submit_or_run_fallible(caller_eligible(), || Ok::<_, ()>(4))
        .unwrap();
    assert_eq!(succeeded.status(), Some(TaskStatus::Succeeded));
    assert_eq!(succeeded.try_take(), Some(Outcome::Success(Ok(4))));
    let saturated = runtime.progress().scheduler;
    assert_eq!(saturated.ready, 1);
    assert_eq!(saturated.running, 1);
    assert_eq!(saturated.handoff_wrappers, [0, 0, 1]);
    assert!(saturated.runnable_full[2]);
    drop(release);
    assert_eq!(first.completion().wait().unwrap(), TaskStatus::Succeeded);
    assert_eq!(queued.completion().wait().unwrap(), TaskStatus::Succeeded);
    assert_eq!(first.try_take(), Some(Outcome::Success(1)));
    assert_eq!(queued.try_take(), Some(Outcome::Success(2)));
    runtime.shutdown().unwrap();
}

#[test]
fn full_handoff_window_does_not_mean_runnable_saturation() {
    let mut runtime = runtime(2, 1);
    let lane = runtime.lane(ExecutionClass::Mid);
    let (started_send, started_recv) = mpsc::channel();
    let (release_send, release_recv) = mpsc::channel();
    let (running, _) = lane
        .try_spawn(SpawnOptions::default(), move || {
            started_send.send(()).unwrap();
            release_recv.recv_timeout(TIMEOUT).unwrap();
        })
        .unwrap();
    started_recv.recv_timeout(TIMEOUT).unwrap();
    let (pending, _) = lane.try_spawn(caller_eligible(), || 5).unwrap();
    let (more, _) = lane.try_spawn(caller_eligible(), || 6).unwrap();
    assert_eq!(pending.status(), None);
    assert_eq!(more.status(), None);
    release_send.send(()).unwrap();
    assert_eq!(running.completion().wait().unwrap(), TaskStatus::Succeeded);
    assert_eq!(pending.completion().wait().unwrap(), TaskStatus::Succeeded);
    assert_eq!(more.completion().wait().unwrap(), TaskStatus::Succeeded);
    runtime.shutdown().unwrap();
}

#[test]
fn repeated_cancelled_ready_jobs_leave_no_queue_entries() {
    struct Release(mpsc::Sender<()>);
    impl Drop for Release {
        fn drop(&mut self) {
            let _ = self.0.send(());
        }
    }
    struct CountDrop(Arc<AtomicUsize>);
    impl Drop for CountDrop {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    let mut runtime = runtime(1, 1);
    let lane = runtime.lane(ExecutionClass::Mid);
    let (started_send, started_recv) = mpsc::channel();
    let (release_send, release_recv) = mpsc::channel();
    let release = Release(release_send);
    let (running, _) = lane
        .try_spawn(SpawnOptions::default(), move || {
            started_send.send(()).unwrap();
            release_recv.recv_timeout(TIMEOUT).unwrap();
        })
        .unwrap();
    started_recv.recv_timeout(TIMEOUT).unwrap();
    let drops = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(AtomicUsize::new(0));
    for iteration in 0..2048 {
        let capture = CountDrop(Arc::clone(&drops));
        let invoked = Arc::clone(&calls);
        let (task, control) = lane
            .try_spawn(SpawnOptions::default(), move || {
                let _capture = capture;
                invoked.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();
        control.cancel();
        assert_eq!(
            task.completion().wait_timeout(TIMEOUT).unwrap(),
            Some(TaskStatus::Cancelled)
        );
        assert_eq!(drops.load(Ordering::SeqCst), iteration + 1);
        let progress = runtime.progress().scheduler;
        assert_eq!(progress.ready, 0);
        assert_eq!(progress.deferred, 0);
        assert_eq!(progress.handoff_wrappers, [0, 1, 0]);
        assert!(!progress.records_full);
        assert!(!progress.runnable_full[1]);
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let (sentinel_send, sentinel_recv) = mpsc::channel();
    let (sentinel, _) = lane
        .try_spawn(SpawnOptions::default(), move || {
            sentinel_send.send(()).unwrap();
        })
        .unwrap();
    drop(release);
    sentinel_recv.recv_timeout(TIMEOUT).unwrap();
    assert_eq!(
        sentinel.completion().wait_timeout(TIMEOUT).unwrap(),
        Some(TaskStatus::Succeeded)
    );
    assert_eq!(running.completion().wait().unwrap(), TaskStatus::Succeeded);
    runtime.shutdown().unwrap();
}

#[test]
fn prerequisite_failure_suppresses_only_success_only_successors() {
    for form in ["standalone", "group", "dependent", "grouped_dependent"] {
        for fail in [false, true] {
            let mut runtime = runtime(8, 2);
            let lane = runtime.lane(ExecutionClass::Low);
            let group = lane.group().unwrap();
            let options = SpawnOptions::default();
            let ready = engine_worker::Task::ready(()).completion();
            let cancelled =
                engine_worker::Task::<()>::ready_outcome(Outcome::Cancelled).completion();
            let operation = move || {
                if fail {
                    Err(String::from("decode failed"))
                } else {
                    Ok(7usize)
                }
            };
            let (mut source, _) = match form {
                "standalone" => lane.try_spawn_fallible(options, operation),
                "group" => lane.try_spawn_fallible_in(&group, options, operation),
                "dependent" => lane.try_spawn_after_fallible(
                    options,
                    &[ready],
                    DependencyPolicy::SuccessOnly,
                    operation,
                ),
                "grouped_dependent" => lane.try_spawn_after_fallible_in(
                    &group,
                    options,
                    &[cancelled],
                    DependencyPolicy::OutcomeAware,
                    operation,
                ),
                _ => unreachable!(),
            }
            .unwrap();
            group.seal();
            let prerequisite = source.completion();
            let invoked = Arc::new(AtomicUsize::new(0));
            let calls = Arc::clone(&invoked);
            let (mut success_only, _) = lane
                .try_spawn_after(
                    options,
                    std::slice::from_ref(&prerequisite),
                    DependencyPolicy::SuccessOnly,
                    move || {
                        calls.fetch_add(1, Ordering::Relaxed);
                        1
                    },
                )
                .unwrap();
            let (mut aware, _) = lane
                .try_spawn_after(
                    options,
                    &[prerequisite],
                    DependencyPolicy::OutcomeAware,
                    || 2,
                )
                .unwrap();
            runtime.shutdown().unwrap();
            assert!(group.is_complete(), "{form}");
            assert_eq!(
                source.status(),
                Some(if fail {
                    TaskStatus::ApplicationFailed
                } else {
                    TaskStatus::Succeeded
                }),
                "{form}"
            );
            assert_eq!(
                source.try_take(),
                Some(Outcome::Success(if fail {
                    Err(String::from("decode failed"))
                } else {
                    Ok(7)
                })),
                "{form}"
            );
            assert_eq!(
                success_only.status(),
                Some(if fail {
                    TaskStatus::PrerequisiteFailed
                } else {
                    TaskStatus::Succeeded
                }),
                "{form}"
            );
            assert_eq!(
                success_only.try_take(),
                Some(if fail {
                    Outcome::PrerequisiteFailed
                } else {
                    Outcome::Success(1)
                }),
                "{form}"
            );
            assert_eq!(aware.try_take(), Some(Outcome::Success(2)), "{form}");
            assert_eq!(
                invoked.load(Ordering::Relaxed),
                usize::from(!fail),
                "{form}"
            );
        }
    }
}

#[test]
fn completed_dependency_defers_when_runnable_queue_is_full() {
    let mut runtime = runtime(1, 1);
    let lane = runtime.lane(ExecutionClass::Low);
    let (started_send, started_recv) = mpsc::channel();
    let (release_send, release_recv) = mpsc::channel();
    let (running, _) = lane
        .try_spawn(SpawnOptions::default(), move || {
            started_send.send(()).unwrap();
            release_recv.recv_timeout(TIMEOUT).unwrap();
        })
        .unwrap();
    started_recv.recv_timeout(TIMEOUT).unwrap();
    let (queued, _) = lane.try_spawn(SpawnOptions::default(), || 1).unwrap();
    let ready = engine_worker::Task::ready(());
    let (mut deferred, _) = lane
        .try_spawn_after_fallible(
            SpawnOptions::default(),
            &[ready.completion()],
            DependencyPolicy::SuccessOnly,
            || Err::<usize, _>("deferred failure"),
        )
        .unwrap();
    assert_eq!(deferred.status(), None);
    release_send.send(()).unwrap();
    assert_eq!(running.completion().wait().unwrap(), TaskStatus::Succeeded);
    assert_eq!(queued.completion().wait().unwrap(), TaskStatus::Succeeded);
    assert_eq!(
        deferred.completion().wait().unwrap(),
        TaskStatus::ApplicationFailed
    );
    assert_eq!(
        deferred.try_take(),
        Some(Outcome::Success(Err("deferred failure")))
    );
    runtime.shutdown().unwrap();
}

#[test]
fn rejected_submission_returns_uninvoked_capture() {
    let mut runtime = runtime(1, 1);
    let lane = runtime.lane(ExecutionClass::High);
    let (started_send, started_recv) = mpsc::channel();
    let (release_send, release_recv) = mpsc::channel();
    let (running, _) = lane
        .try_spawn(SpawnOptions::default(), move || {
            started_send.send(()).unwrap();
            release_recv.recv_timeout(TIMEOUT).unwrap();
        })
        .unwrap();
    started_recv.recv_timeout(TIMEOUT).unwrap();
    let (queued, _) = lane.try_spawn(SpawnOptions::default(), || ()).unwrap();
    let input = String::from("returned capture");
    let rejected = lane
        .try_spawn(SpawnOptions::default(), move || input)
        .err()
        .expect("full runnable queue rejects");
    assert_eq!(rejected.reason, SpawnError::Full);
    assert_eq!((rejected.operation)(), "returned capture");
    release_send.send(()).unwrap();
    assert_eq!(running.completion().wait().unwrap(), TaskStatus::Succeeded);
    assert_eq!(queued.completion().wait().unwrap(), TaskStatus::Succeeded);
    runtime.shutdown().unwrap();
}

#[test]
fn saturated_inline_rejects_a_different_class_context() {
    let mut runtime = runtime(1, 1);
    let high = runtime.lane(ExecutionClass::High);
    let low = runtime.lane(ExecutionClass::Low);
    let (started_send, started_recv) = mpsc::channel();
    let (release_send, release_recv) = mpsc::channel();
    let (running, _) = high
        .try_spawn(SpawnOptions::default(), move || {
            started_send.send(()).unwrap();
            release_recv.recv_timeout(TIMEOUT).unwrap();
        })
        .unwrap();
    started_recv.recv_timeout(TIMEOUT).unwrap();
    let (queued, _) = high.try_spawn(SpawnOptions::default(), || ()).unwrap();
    let (result, _) = low
        .try_spawn(SpawnOptions::default(), move || {
            high.submit_or_run(caller_eligible(), || 3)
                .err()
                .unwrap()
                .reason
        })
        .unwrap();
    assert_eq!(result.completion().wait().unwrap(), TaskStatus::Succeeded);
    let mut result = result;
    assert_eq!(
        result.try_take(),
        Some(Outcome::Success(SpawnError::InvalidContext))
    );
    release_send.send(()).unwrap();
    assert_eq!(running.completion().wait().unwrap(), TaskStatus::Succeeded);
    assert_eq!(queued.completion().wait().unwrap(), TaskStatus::Succeeded);
    runtime.shutdown().unwrap();
}

#[test]
fn long_cancelled_successor_chain_drains_without_recursion() {
    const LENGTH: usize = 4096;
    let config = RuntimeConfig::new(3, [WorkerConfig::new(1); 3]).unwrap();
    let limits = OwnedLimits::new(LENGTH + 2, LENGTH + 2, [1; 3], [1; 3]).unwrap();
    let mut runtime = Runtime::builder(config)
        .with_owned_limits(limits)
        .build()
        .unwrap();
    let lane = runtime.lane(ExecutionClass::Low);
    let (started_send, started_recv) = mpsc::channel();
    let (release_send, release_recv) = mpsc::channel();
    let (predecessor, _) = lane
        .try_spawn(SpawnOptions::default(), move || {
            started_send.send(()).unwrap();
            release_recv.recv_timeout(TIMEOUT).unwrap();
        })
        .unwrap();
    started_recv.recv_timeout(TIMEOUT).unwrap();
    let mut previous = predecessor.completion();
    let mut tasks = Vec::with_capacity(LENGTH);
    let mut first_control = None;
    for _ in 0..LENGTH {
        let (task, control) = lane
            .try_spawn_after(
                SpawnOptions::default(),
                &[previous],
                DependencyPolicy::SuccessOnly,
                || (),
            )
            .unwrap();
        previous = task.completion();
        if first_control.is_none() {
            first_control = Some(control);
        }
        tasks.push(task);
    }
    first_control.unwrap().cancel();
    assert_eq!(
        tasks
            .last()
            .unwrap()
            .completion()
            .wait_timeout(TIMEOUT)
            .unwrap(),
        Some(TaskStatus::PrerequisiteFailed)
    );
    release_send.send(()).unwrap();
    assert_eq!(
        predecessor.completion().wait().unwrap(),
        TaskStatus::Succeeded
    );
    runtime.shutdown().unwrap();
}
