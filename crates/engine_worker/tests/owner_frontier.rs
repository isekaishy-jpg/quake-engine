use std::num::NonZeroUsize;

use engine_worker::{
    DeliveryStatus, Phase, PumpBudget, PumpMode, Runtime, RuntimeConfig, WorkerConfig,
};

#[test]
fn batch_preserves_entry_eligible_frontier_when_callback_requests_close() {
    let config = RuntimeConfig::new(3, [WorkerConfig::new(1); 3]).unwrap();
    let mut runtime = Runtime::builder(config).build().unwrap();
    let mut owner = runtime.owner((), NonZeroUsize::new(3).unwrap()).unwrap();
    let current_phase = Phase(1);
    let other_phase = Phase(2);
    owner.set_phase(current_phase).unwrap();

    // Queue order is [other phase, current phase #1, current phase #2].
    // Only the latter two belong to the Batch frontier at pump entry.
    let (outside_frontier, _) = owner.try_post(other_phase, |_| {}).unwrap();
    let close = owner.control();
    let (first, _) = owner
        .try_post(current_phase, move |_| close.request_close())
        .unwrap();
    let (second, _) = owner.try_post(current_phase, |_| {}).unwrap();

    let report = owner
        .pump(current_phase, PumpBudget::new(3).with_mode(PumpMode::Batch))
        .unwrap();
    let after_pump = (first.status(), second.status(), outside_frontier.status());

    // Complete cleanup before asserting so a failing regression does not
    // leave worker threads or outstanding owner obligations behind.
    owner.close();
    runtime.shutdown().unwrap();

    assert_eq!(report.invoked, 1);
    assert_eq!(report.suppressed, 1);
    assert_eq!(
        after_pump,
        (
            DeliveryStatus::Published,
            DeliveryStatus::Suppressed,
            DeliveryStatus::Ready,
        ),
        "Batch must retain the identities eligible at entry; a close request \
         must not replace an entry-frontier delivery with another phase's delivery"
    );
}

#[test]
fn batch_preserves_entry_frontier_when_callback_panics() {
    let config = RuntimeConfig::new(3, [WorkerConfig::new(1); 3]).unwrap();
    let mut runtime = Runtime::builder(config).build().unwrap();
    let mut owner = runtime.owner((), NonZeroUsize::new(3).unwrap()).unwrap();
    let phase = Phase(1);
    owner.set_phase(phase).unwrap();
    let (outside, _) = owner.try_post(Phase(2), |_| {}).unwrap();
    let (first, _) = owner
        .try_post(phase, |_| panic!("fault current batch"))
        .unwrap();
    let (second, _) = owner
        .try_post(phase, |_| panic!("must be suppressed"))
        .unwrap();
    let report = owner
        .pump(phase, PumpBudget::new(3).with_mode(PumpMode::Batch))
        .unwrap();
    assert_eq!(report.invoked, 0);
    assert_eq!(report.suppressed, 2);
    assert_eq!(first.status(), DeliveryStatus::Panicked);
    assert_eq!(second.status(), DeliveryStatus::Suppressed);
    assert_eq!(outside.status(), DeliveryStatus::Ready);
    owner.close();
    assert_eq!(outside.status(), DeliveryStatus::Suppressed);
    runtime.shutdown().unwrap();
}

#[test]
fn next_batch_after_close_can_clean_other_phases_within_its_budget() {
    let config = RuntimeConfig::new(3, [WorkerConfig::new(1); 3]).unwrap();
    let mut runtime = Runtime::builder(config).build().unwrap();
    let mut owner = runtime.owner((), NonZeroUsize::new(3).unwrap()).unwrap();
    let phase = Phase(1);
    owner.set_phase(phase).unwrap();
    let (outside, _) = owner.try_post(Phase(2), |_| {}).unwrap();
    let close = owner.control();
    let (first, _) = owner
        .try_post(phase, move |_| close.request_close())
        .unwrap();
    let (second, _) = owner.try_post(phase, |_| {}).unwrap();
    let one = PumpBudget::new(1).with_mode(PumpMode::Batch);
    assert_eq!(owner.pump(phase, one).unwrap().invoked, 1);
    assert_eq!(first.status(), DeliveryStatus::Published);
    assert_eq!(outside.status(), DeliveryStatus::Ready);
    assert_eq!(second.status(), DeliveryStatus::Ready);
    assert_eq!(owner.pump(phase, one).unwrap().suppressed, 1);
    assert_eq!(outside.status(), DeliveryStatus::Suppressed);
    assert_eq!(second.status(), DeliveryStatus::Ready);
    assert_eq!(owner.pump(phase, one).unwrap().suppressed, 1);
    assert_eq!(second.status(), DeliveryStatus::Suppressed);
    owner.close();
    runtime.shutdown().unwrap();
}
