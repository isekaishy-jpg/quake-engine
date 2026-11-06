//! What the runtime records about itself.
#![cfg(feature = "diagnostics")]

use engine_worker::diagnostics::Trace;
use engine_worker::{
    CallerEligibility, ExecutionClass, Outcome, Runtime, RuntimeConfig, SpawnOptions, WorkerConfig,
};
use std::sync::{Mutex, mpsc};
use std::time::Duration;
use std::time::Instant;

static TRACE_TESTS: Mutex<()> = Mutex::new(());

#[test]
fn diagnostic_timeline_preserves_settlement_and_can_be_drained() {
    let _guard = TRACE_TESTS.lock().unwrap();
    assert!(Trace::start(Instant::now()));
    assert!(!Trace::start(Instant::now()));
    let mut runtime = Runtime::builder(RuntimeConfig::new(3, [WorkerConfig::new(1); 3]).unwrap())
        .with_owned_limits(engine_worker::OwnedLimits::new(32, 32, [32; 3], [1; 3]).unwrap())
        .with_notification_limits(engine_worker::NotifyLimits {
            routes: 1,
            bindings: 1,
        })
        .build()
        .unwrap();
    let lane = runtime.lane(ExecutionClass::High);
    let group = lane.group().unwrap();
    let mut route = runtime.notification_route(|| Ok(())).unwrap();
    let _binding = route.watch_completion(&group.completion()).unwrap();
    let stamp = route.prepare_wait().unwrap();
    let (mut task, _) = lane
        .try_spawn_in(&group, SpawnOptions::default(), || {
            let identity = Trace::current_job().expect("job identity during invocation");
            assert!(identity.runtime != 0);
            assert_eq!(identity.group, Some(1));
            (identity, 42)
        })
        .unwrap();
    group.seal();
    group.wait_helping().unwrap();
    let first_identity = match task.try_take() {
        Some(Outcome::Success((identity, value))) => {
            assert_eq!(value, 42);
            identity
        }
        other => panic!("unexpected first outcome: {other:?}"),
    };
    runtime.shutdown().unwrap();
    // Completion visibility precedes notification publication. Joined shutdown
    // also settles the publisher, so the route epoch is now safe to inspect.
    assert!(route.changed_since(stamp).unwrap());
    let mut second = Runtime::builder(RuntimeConfig::new(3, [WorkerConfig::new(1); 3]).unwrap())
        .with_owned_limits(engine_worker::OwnedLimits::new(32, 32, [32; 3], [1; 3]).unwrap())
        .build()
        .unwrap();
    let second_lane = second.lane(ExecutionClass::High);
    let second_group = second_lane.group().unwrap();
    let (mut second_task, _) = second_lane
        .try_spawn_in(&second_group, SpawnOptions::default(), || {
            Trace::current_job().expect("second runtime job identity")
        })
        .unwrap();
    second_group.seal();
    second_group.wait_helping().unwrap();
    let second_identity = match second_task.try_take() {
        Some(Outcome::Success(identity)) => identity,
        other => panic!("unexpected second outcome: {other:?}"),
    };
    second.shutdown().unwrap();
    let mut events = Vec::new();
    assert_eq!(Trace::drain(&mut events), 0);
    let invoked = events
        .iter()
        .find(|e| {
            e.event == "job.invoke"
                && e.runtime == first_identity.runtime
                && e.id == first_identity.id
        })
        .unwrap();
    let returned = events
        .iter()
        .find(|e| {
            e.event == "job.body_returned"
                && e.runtime == first_identity.runtime
                && e.id == first_identity.id
        })
        .unwrap();
    let published = events
        .iter()
        .find(|e| {
            e.event == "job.result_publish.end"
                && e.runtime == first_identity.runtime
                && e.id == first_identity.id
        })
        .unwrap();
    let terminal = events
        .iter()
        .find(|e| {
            e.event == "group.terminal_visible"
                && e.runtime == first_identity.runtime
                && Some(e.id) == first_identity.group
        })
        .unwrap();
    assert_eq!(first_identity.id, second_identity.id);
    assert_ne!(invoked.runtime, second_identity.runtime);
    assert!(events.iter().any(|event| {
        event.event == "job.invoke"
            && event.runtime == second_identity.runtime
            && event.id == second_identity.id
    }));
    assert!(events.iter().any(|event| {
        event.event == "group.terminal_visible"
            && event.runtime == second_identity.runtime
            && Some(event.id) == second_identity.group
    }));
    assert!(invoked.elapsed_ns <= returned.elapsed_ns);
    assert!(returned.elapsed_ns <= published.elapsed_ns);
    assert!(published.elapsed_ns <= terminal.elapsed_ns);
    #[cfg(windows)]
    assert!(
        events
            .iter()
            .all(|event| event.native_thread_id.is_some_and(|id| id != 0))
    );
    let mut control = std::collections::HashMap::new();
    for event in &events {
        match event.event {
            "control.request" => {
                assert!(
                    control
                        .insert(event.thread, (event.runtime, event.id, false))
                        .is_none()
                );
            }
            "control.acquired" => {
                let entry = control.get_mut(&event.thread).unwrap();
                assert_eq!((entry.0, entry.1), (event.runtime, event.id));
                assert!(!entry.2);
                entry.2 = true;
            }
            "control.released" => {
                assert_eq!(
                    control.remove(&event.thread),
                    Some((event.runtime, event.id, true))
                );
            }
            _ => {}
        }
    }
    assert!(control.is_empty());
    let visible = events
        .iter()
        .find(|e| e.event == "completion.visible")
        .unwrap();
    assert!(events.iter().any(|e| e.event == "notify.publish.marked"
        && e.runtime == visible.runtime
        && e.id == visible.id
        && e.related == 1
        && e.elapsed_ns >= visible.elapsed_ns));
    events.clear();
    assert_eq!(Trace::drain(&mut events), 0);
    assert!(events.is_empty());
}

#[test]
fn current_job_restores_outer_identity_after_nested_help_and_panic() {
    let _guard = TRACE_TESTS.lock().unwrap();
    assert!(Trace::current_job().is_none());
    let mut runtime = Runtime::builder(RuntimeConfig::new(3, [WorkerConfig::new(1); 3]).unwrap())
        .with_owned_limits(engine_worker::OwnedLimits::new(32, 32, [32; 3], [1; 3]).unwrap())
        .build()
        .unwrap();
    let lane = runtime.lane(ExecutionClass::High);
    let outer_group = lane.group().unwrap();
    let inner_group = lane.group().unwrap();
    let helper_group = inner_group.clone();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (mut outer_task, _) = lane
        .try_spawn_in(&outer_group, SpawnOptions::default(), move || {
            let outer = Trace::current_job().expect("outer job");
            entered_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            assert!(helper_group.help_ready().unwrap());
            assert_eq!(Trace::current_job(), Some(outer));
            outer
        })
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let (mut inner_task, _) = lane
        .try_spawn_in(
            &inner_group,
            SpawnOptions {
                eligibility: CallerEligibility::CallerEligible,
            },
            || {
                let inner = Trace::current_job().expect("helped job");
                assert_eq!(inner.group, Some(2));
                panic!("expected helped-job panic");
            },
        )
        .unwrap();
    inner_group.seal();
    outer_group.seal();
    release_tx.send(()).unwrap();
    outer_group.wait_helping().unwrap();
    inner_group.wait_helping().unwrap();
    let outer = match outer_task.try_take() {
        Some(Outcome::Success(identity)) => identity,
        other => panic!("unexpected outer outcome: {other:?}"),
    };
    assert_eq!(outer.group, Some(1));
    assert_eq!(inner_task.try_take(), Some(Outcome::Panicked));
    assert!(Trace::current_job().is_none());
    runtime.shutdown().unwrap();
}
