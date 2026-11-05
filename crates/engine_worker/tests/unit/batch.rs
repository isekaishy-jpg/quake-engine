use super::Batch;
use crate::runtime::Runtime;
use crate::runtime::config::{ExecutionClass, RuntimeConfig, WorkerConfig};
use crate::scheduler::OwnedLimits;
use crate::task::TaskStatus;
use std::sync::Arc;

fn batch() -> (Runtime, Batch) {
    let config = RuntimeConfig::new(3, [WorkerConfig::new(1); 3]).unwrap();
    let limits = OwnedLimits::new(4, 4, [2; 3], [1; 3]).unwrap();
    let runtime = Runtime::builder(config)
        .with_owned_limits(limits)
        .build()
        .unwrap();
    let batch = runtime.lane(ExecutionClass::High).batch();
    (runtime, batch)
}

#[test]
fn exclusive_completed_wave_reuses_allocation_but_not_retained_token() {
    let (_runtime, mut batch) = batch();
    let first = batch.begin().unwrap();
    let address = Arc::as_ptr(&first.inner);
    let old_id = first.inner.id;
    let old = first.completion();
    first.seal();
    assert_eq!(old.status(), Some(TaskStatus::Succeeded));

    let second = batch.begin().unwrap();
    assert_eq!(Arc::as_ptr(&second.inner), address);
    assert_ne!(second.inner.id, old_id);
    assert_eq!(second.completion().status(), None);
    assert_eq!(old.status(), Some(TaskStatus::Succeeded));
    second.seal();
}

#[test]
fn retained_group_handle_keeps_its_old_identity() {
    let (_runtime, mut batch) = batch();
    let first = batch.begin().unwrap().clone();
    let address = Arc::as_ptr(&first.inner);
    let old_id = first.inner.id;
    first.seal();

    let second = batch.begin().unwrap();
    assert_ne!(Arc::as_ptr(&second.inner), address);
    assert!(first.is_complete());
    assert_eq!(first.completion().status(), Some(TaskStatus::Succeeded));
    assert_ne!(second.inner.id, old_id);
    assert_eq!(second.completion().status(), None);
    second.seal();
}
