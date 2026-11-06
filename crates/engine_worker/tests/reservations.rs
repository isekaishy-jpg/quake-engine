//! Setting capacity aside before it is needed.
#![expect(clippy::unwrap_used, reason = "a failed test is the intended report")]

use engine_worker::{
    Cost, ExecutionClass, Limits, Outcome, OwnedLimits, Priority, ReservationError, Runtime,
    RuntimeConfig, SpawnError, SpawnOptions, StageOptions, TaskStatus, WorkerConfig,
};
use std::num::NonZeroUsize;
use std::sync::mpsc;
use std::time::Duration;

fn runtime() -> Runtime {
    runtime_with_runnable(8)
}

fn runtime_with_runnable(runnable: usize) -> Runtime {
    let config = RuntimeConfig::new(3, [WorkerConfig::new(1); 3]).unwrap();
    let owned = OwnedLimits::new(8, 8, [runnable; 3], [1; 3]).unwrap();
    let capacity =
        Limits::new(Cost::new(2, 2, 2, 80), Cost::new(2, 2, 2, 0), 1, Some(100)).unwrap();
    Runtime::builder(config)
        .with_owned_limits(owned)
        .with_capacity_limits(capacity)
        .with_demand_limits(vec![Priority::new(0), Priority::new(5)], 4)
        .build()
        .unwrap()
}

#[test]
fn required_stage_precedes_unhanded_background_when_runnable_window_is_full() {
    let mut runtime = runtime_with_runnable(1);
    let lane = runtime.lane(ExecutionClass::High);
    let (started_send, started_recv) = mpsc::channel();
    let (release_send, release_recv) = mpsc::channel();
    let (running, _) = lane
        .try_spawn(SpawnOptions::default(), move || {
            started_send.send(()).unwrap();
            release_recv.recv().unwrap();
            1
        })
        .unwrap();
    started_recv.recv_timeout(Duration::from_secs(10)).unwrap();
    let (order_send, order_recv) = mpsc::channel();
    let background_send = order_send.clone();
    let (queued, _) = lane
        .try_spawn_stage(
            StageOptions {
                priority: Some(Priority::new(5)),
                ..Default::default()
            },
            move || {
                background_send.send("background").unwrap();
                2
            },
        )
        .unwrap();
    let reserve = runtime.reserve_required(Cost::new(1, 0, 0, 0)).unwrap();
    let options = StageOptions {
        reservation: Some(&reserve),
        cost: Cost::new(1, 0, 0, 0),
        priority: Some(Priority::new(5)),
        ..StageOptions::default()
    };
    let (required, _) = lane
        .try_spawn_stage(options, move || {
            order_send.send("urgent").unwrap();
            3
        })
        .unwrap();
    let demand = required.completion().demand(Priority::new(0)).unwrap();
    while runtime.service_demand(1) {}
    release_send.send(()).unwrap();
    let observed: Vec<_> = (0..2)
        .map(|_| order_recv.recv_timeout(Duration::from_secs(10)).unwrap())
        .collect();
    assert_eq!(observed, ["urgent", "background"]);
    assert_eq!(running.completion().wait().unwrap(), TaskStatus::Succeeded);
    assert_eq!(queued.completion().wait().unwrap(), TaskStatus::Succeeded);
    assert_eq!(required.completion().wait().unwrap(), TaskStatus::Succeeded);
    drop((running, queued, required, reserve, demand));
    runtime.shutdown().unwrap();
}

#[test]
fn shared_outcome_keeps_declared_bytes_after_pipeline_finishes() {
    let mut runtime = runtime();
    let lane = runtime.lane(ExecutionClass::Mid);
    let set = runtime.work_set(NonZeroUsize::new(1).unwrap()).unwrap();
    let reserve = runtime.reserve_required(Cost::new(1, 0, 0, 70)).unwrap();
    let options = StageOptions {
        reservation: Some(&reserve),
        work_set: Some(&set),
        cost: Cost::new(1, 0, 0, 70),
        retained_bytes: 70,
        ..StageOptions::default()
    };
    let (task, _) = lane.try_spawn_stage(options, || vec![7_u8; 70]).unwrap();
    let shared = task.into_shared();
    drop(reserve);
    assert_eq!(shared.completion().wait().unwrap(), TaskStatus::Succeeded);
    set.seal();
    set.wait_drained().unwrap();
    let outcome = shared.try_result().unwrap();
    assert!(matches!(&*outcome, Outcome::Success(value) if value.len() == 70));
    drop(shared);
    let usage = runtime.capacity_usage().unwrap();
    assert_eq!(usage.required.bytes, 70);
    assert_eq!(usage.required.records, 0);
    assert_eq!(usage.required_pipelines, 0);
    assert_eq!(
        runtime.reserve_required(Cost::new(1, 0, 0, 40)).err(),
        Some(ReservationError::Full)
    );
    let next = runtime.reserve_required(Cost::new(1, 0, 0, 20)).unwrap();
    drop(next);
    drop(outcome);
    assert_eq!(runtime.capacity_usage().unwrap().required.bytes, 0);
    runtime.shutdown().unwrap();
}

#[test]
fn failed_stage_preserves_reservation_and_uninvoked_operation() {
    let mut runtime = runtime();
    let lane = runtime.lane(ExecutionClass::High);
    let reserve = runtime.reserve_required(Cost::new(1, 0, 0, 30)).unwrap();
    let before = reserve.available();
    let options = StageOptions {
        reservation: Some(&reserve),
        cost: Cost::new(1, 0, 0, 30),
        retained_bytes: 30,
        priority: Some(Priority::new(99)),
        ..StageOptions::default()
    };
    let rejected = lane.try_spawn_stage(options, || 42).err().unwrap();
    assert_eq!(rejected.reason, SpawnError::InvalidPriority);
    assert_eq!(reserve.available(), before);
    assert_eq!((rejected.operation)(), 42);
    drop(rejected.options);
    drop(reserve);
    assert_eq!(runtime.capacity_usage().unwrap().required.bytes, 0);
    runtime.shutdown().unwrap();
}
