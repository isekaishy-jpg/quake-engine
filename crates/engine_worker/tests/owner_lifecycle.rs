use std::cell::Cell;
use std::num::NonZeroUsize;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread::{self, ThreadId};
use std::time::Duration;

use engine_worker::{
    DeliveryStatus, ExecutionClass, OwnedLimits, OwnerError, Phase, PumpBudget, ReadyAccess,
    Runtime, RuntimeConfig, Shared, ShutdownError, SpawnOptions, TaskStatus, WaitError,
    WorkerConfig,
};

const TIMEOUT: Duration = Duration::from_secs(5);

fn runtime() -> Runtime {
    let config = RuntimeConfig::new(3, [WorkerConfig::new(1); 3]).unwrap();
    let limits = OwnedLimits::new(8, 8, [4; 3], [2; 3]).unwrap();
    Runtime::builder(config)
        .with_owned_limits(limits)
        .build()
        .unwrap()
}

#[test]
fn graceful_shutdown_reports_live_owner_until_local_cleanup_finishes() {
    let mut runtime = runtime();
    let phase = Phase(1);
    let mut owner = runtime
        .owner(0usize, NonZeroUsize::new(1).unwrap())
        .unwrap();
    owner.set_phase(phase).unwrap();
    let (delivery, _) = owner.try_post(phase, |state| *state += 1).unwrap();

    assert_eq!(runtime.shutdown(), Err(ShutdownError::LiveOwners));
    assert_eq!(
        engine_worker::Task::ready(1usize)
            .completion()
            .wait_timeout(Duration::ZERO),
        Err(WaitError::ExecutionContext),
    );
    assert_eq!(delivery.status(), DeliveryStatus::Ready);

    owner.close();
    assert_eq!(delivery.status(), DeliveryStatus::Suppressed);
    assert_eq!(*owner.state(), 0);
    runtime.shutdown().unwrap();
}

struct DropThread {
    dropped_on: Rc<Cell<Option<ThreadId>>>,
}

impl Drop for DropThread {
    fn drop(&mut self) {
        self.dropped_on.set(Some(thread::current().id()));
    }
}

#[test]
fn dropping_owner_cleans_local_capture_before_late_worker_completion() {
    let mut runtime = runtime();
    let lane = runtime.lane(ExecutionClass::Low);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (task, _) = lane
        .try_spawn(SpawnOptions::default(), move || {
            entered_tx.send(()).unwrap();
            release_rx.recv_timeout(TIMEOUT).unwrap();
            3usize
        })
        .unwrap();
    entered_rx.recv_timeout(TIMEOUT).unwrap();

    let phase = Phase(2);
    let dropped_on = Rc::new(Cell::new(None));
    let probe = DropThread {
        dropped_on: Rc::clone(&dropped_on),
    };
    let mut owner = runtime
        .owner(0usize, NonZeroUsize::new(1).unwrap())
        .unwrap();
    owner.set_phase(phase).unwrap();
    let (delivery, _) = owner
        .on_ready(&task.completion(), phase, move |_, _| {
            let _probe = probe;
            panic!("closed owner callback must not run");
        })
        .unwrap();

    drop(owner);
    assert_eq!(dropped_on.get(), Some(thread::current().id()));
    assert_eq!(delivery.status(), DeliveryStatus::Suppressed);

    release_tx.send(()).unwrap();
    assert_eq!(
        task.completion().wait_timeout(TIMEOUT).unwrap(),
        Some(TaskStatus::Succeeded)
    );
    assert_eq!(delivery.status(), DeliveryStatus::Suppressed);
    runtime.shutdown().unwrap();
}

#[test]
fn runtime_abandonment_closes_routes_but_leaves_capture_cleanup_with_owner() {
    let mut runtime = runtime();
    let phase = Phase(3);
    let dropped_on = Rc::new(Cell::new(None));
    let probe = DropThread {
        dropped_on: Rc::clone(&dropped_on),
    };
    let mut owner = runtime
        .owner(0usize, NonZeroUsize::new(2).unwrap())
        .unwrap();
    owner.set_phase(phase).unwrap();
    let sender = owner.sender();
    let prepared = owner
        .prepare_delivery(phase, move |_: &mut usize| {
            let _probe = probe;
        })
        .unwrap();

    runtime.abandon();
    assert_eq!(dropped_on.get(), None);
    assert_eq!(
        owner.try_post(phase, |_| {}).err().unwrap().reason,
        OwnerError::Closed
    );
    assert_eq!(
        sender.try_post(phase, |_| {}).err().unwrap().reason,
        OwnerError::Closed
    );
    assert!(matches!(
        owner.with_ready(&Shared::ready(1usize), phase, |_, _| ()),
        ReadyAccess::Rejected(rejected) if rejected.reason == OwnerError::Closed
    ));
    assert_eq!(prepared.delivery.status(), DeliveryStatus::Waiting);

    assert_eq!(owner.pump(phase, PumpBudget::new(1)).unwrap().suppressed, 1);
    assert_eq!(dropped_on.get(), Some(thread::current().id()));
    assert_eq!(prepared.delivery.status(), DeliveryStatus::Suppressed);
    drop(prepared.ticket);
    drop(owner);
}
