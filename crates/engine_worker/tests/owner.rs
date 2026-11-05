use std::cell::Cell;
use std::num::NonZeroUsize;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use engine_worker::{
    CancelResult, DeliveryStatus, OwnerError, Phase, PumpBudget, PumpMode, ReadyAccess, Runtime,
    RuntimeConfig, Shared, TaskStatus, WorkerConfig,
};

fn new_runtime() -> Runtime {
    let config = RuntimeConfig::new(3, [WorkerConfig::new(1); 3]).unwrap();
    Runtime::builder(config).build().unwrap()
}

struct CloseOnDrop(engine_worker::OwnerControl);

struct HostilePanic;

impl Drop for HostilePanic {
    fn drop(&mut self) {
        std::panic::panic_any(HostilePanic);
    }
}

#[test]
fn capture_cleanup_panic_settles_faults_and_continues_cancel_or_close() {
    for close in [false, true] {
        let mut runtime = new_runtime();
        let mut owner = runtime.owner((), NonZeroUsize::new(2).unwrap()).unwrap();
        let phase = Phase(1);
        owner.set_phase(phase).unwrap();
        let capture = HostilePanic;
        let (delivery, cancel) = owner.try_post(phase, move |_| drop(capture)).unwrap();
        let (following, _) = owner
            .try_post(phase, |_| panic!("must be suppressed"))
            .unwrap();
        if close {
            owner.close();
        } else {
            cancel.cancel();
            assert_eq!(owner.pump(phase, PumpBudget::new(2)).unwrap().suppressed, 2);
        }
        assert_eq!(delivery.status(), DeliveryStatus::Panicked);
        assert_eq!(following.status(), DeliveryStatus::Suppressed);
        if !close {
            owner.recover().unwrap();
            // Both entitlements returned, even with old status observers alive.
            owner.try_post(phase, |_| {}).unwrap();
            owner.try_post(phase, |_| {}).unwrap();
            assert_eq!(owner.pump(phase, PumpBudget::new(2)).unwrap().invoked, 2);
        }
        owner.close();
        runtime.shutdown().unwrap();
    }
}

impl Drop for CloseOnDrop {
    fn drop(&mut self) {
        self.0.request_close();
    }
}

#[test]
fn local_callbacks_defer_warm_hits_and_clean_non_send_captures() {
    let mut runtime = new_runtime();
    let phase = Phase(7);
    let other = Phase(8);
    let calls = Rc::new(Cell::new(0));
    let mut owner = runtime
        .owner(0usize, NonZeroUsize::new(2).unwrap())
        .unwrap();
    owner.set_phase(phase).unwrap();
    let ready = Shared::ready(3usize);
    let immediate = owner.with_ready(&ready, phase, |state, outcome| {
        *state += 1;
        assert!(matches!(&*outcome, engine_worker::Outcome::Success(3)));
        *state
    });
    assert!(matches!(immediate, ReadyAccess::Ready(1)));

    let calls_in_callback = Rc::clone(&calls);
    let (delivery, _) = owner
        .on_ready(&ready.completion(), phase, move |state, status| {
            assert_eq!(status, TaskStatus::Succeeded);
            calls_in_callback.set(calls_in_callback.get() + 1);
            *state += 2;
        })
        .unwrap();
    assert_eq!(calls.get(), 0);
    assert_eq!(delivery.status(), DeliveryStatus::Ready);
    assert_eq!(
        owner.pump(other, PumpBudget::new(2)),
        Err(OwnerError::WrongPhase)
    );
    let report = owner.pump(phase, PumpBudget::new(2)).unwrap();
    assert_eq!(report.invoked, 1);
    assert_eq!(calls.get(), 1);
    assert_eq!(*owner.state(), 3);
    assert_eq!(delivery.status(), DeliveryStatus::Published);
    owner.close();
    runtime.shutdown().unwrap();
}

#[test]
fn reservation_pressure_cancel_and_close_destroy_locally() {
    let mut runtime = new_runtime();
    let phase = Phase(1);
    let dropped_on = Rc::new(Cell::new(false));
    let mut owner = runtime
        .owner(0usize, NonZeroUsize::new(1).unwrap())
        .unwrap();
    owner.set_phase(phase).unwrap();
    let capture = Rc::clone(&dropped_on);
    let prepared = owner
        .prepare_delivery(phase, move |_: &mut usize| {
            capture.set(true);
        })
        .unwrap();
    assert_eq!(Rc::strong_count(&dropped_on), 2);
    let rejected = owner.try_post(phase, |_| {}).err().unwrap();
    assert_eq!(rejected.reason, OwnerError::Full);
    assert_eq!(prepared.control.cancel(), CancelResult::Requested);
    drop(prepared.ticket);
    assert!(!dropped_on.get());
    assert_eq!(
        owner
            .pump(phase, PumpBudget::new(1).with_duration(Duration::ZERO))
            .unwrap()
            .processed(),
        0
    );
    let report = owner.pump(phase, PumpBudget::new(1)).unwrap();
    assert_eq!(report.suppressed, 1);
    assert!(!dropped_on.get()); // Callback body was suppressed.
    assert_eq!(Rc::strong_count(&dropped_on), 1);
    assert_eq!(prepared.delivery.status(), DeliveryStatus::Suppressed);
    owner.close();
    runtime.shutdown().unwrap();
}

#[test]
fn batch_freezes_entry_frontier_while_live_sees_new_arrivals() {
    for mode in [PumpMode::Batch, PumpMode::Live] {
        let mut runtime = new_runtime();
        let phase = Phase(1);
        let mut owner = runtime
            .owner(Vec::<usize>::new(), NonZeroUsize::new(2).unwrap())
            .unwrap();
        owner.set_phase(phase).unwrap();
        let (start_tx, start_rx) = mpsc::channel();
        let (sent_tx, sent_rx) = mpsc::channel();
        let prepared = owner
            .prepare_delivery(phase, |state: &mut Vec<usize>| state.push(2))
            .unwrap();
        let worker = thread::spawn(move || {
            start_rx.recv().unwrap();
            prepared.ticket.ready();
            sent_tx.send(()).unwrap();
        });
        owner
            .try_post(phase, move |state| {
                state.push(1);
                start_tx.send(()).unwrap();
                sent_rx.recv().unwrap();
            })
            .unwrap();
        let report = owner
            .pump(phase, PumpBudget::new(2).with_mode(mode))
            .unwrap();
        worker.join().unwrap();
        assert_eq!(report.invoked, if mode == PumpMode::Live { 2 } else { 1 });
        if mode == PumpMode::Batch {
            assert_eq!(owner.pump(phase, PumpBudget::new(2)).unwrap().invoked, 1);
        }
        assert_eq!(owner.state(), &vec![1, 2]);
        owner.close();
        runtime.shutdown().unwrap();
    }
}

#[test]
fn callback_teardown_and_panic_suppress_following_publication() {
    // Suppressed capture cleanup has the same teardown checkpoint as a
    // callback returning normally, even within a single Live pump.
    let mut runtime = new_runtime();
    let phase = Phase(1);
    let mut owner = runtime
        .owner(0usize, NonZeroUsize::new(2).unwrap())
        .unwrap();
    owner.set_phase(phase).unwrap();
    let capture = CloseOnDrop(owner.control());
    let (first, cancel) = owner.try_post(phase, move |_| drop(capture)).unwrap();
    let (second, _) = owner.try_post(phase, |state| *state += 1).unwrap();
    assert_eq!(cancel.cancel(), CancelResult::Requested);
    let report = owner.pump(phase, PumpBudget::new(2)).unwrap();
    assert_eq!(report.invoked, 0);
    assert_eq!(report.suppressed, 2);
    assert_eq!(*owner.state(), 0);
    assert_eq!(first.status(), DeliveryStatus::Suppressed);
    assert_eq!(second.status(), DeliveryStatus::Suppressed);
    runtime.shutdown().unwrap();

    let mut runtime = new_runtime();
    let phase = Phase(1);
    let mut owner = runtime
        .owner(0usize, NonZeroUsize::new(2).unwrap())
        .unwrap();
    owner.set_phase(phase).unwrap();
    let control = owner.control();
    owner
        .try_post(phase, move |state| {
            *state += 1;
            control.request_close();
        })
        .unwrap();
    let (second, _) = owner.try_post(phase, |state| *state += 10).unwrap();
    assert_eq!(owner.pump(phase, PumpBudget::new(2)).unwrap().invoked, 1);
    assert_eq!(*owner.state(), 1);
    assert_eq!(second.status(), DeliveryStatus::Suppressed);
    runtime.shutdown().unwrap();

    let mut runtime = new_runtime();
    let mut owner = runtime
        .owner(0usize, NonZeroUsize::new(1).unwrap())
        .unwrap();
    owner.set_phase(phase).unwrap();
    let (pending, _) = owner.try_post(phase, |state| *state += 10).unwrap();
    let sender = owner.sender();
    owner.control().request_close();
    assert_eq!(
        owner.pump(phase, PumpBudget::new(0)).unwrap().processed(),
        0
    );
    assert_eq!(
        sender.try_post(phase, |_| {}).err().unwrap().reason,
        OwnerError::Closed
    );
    assert_eq!(pending.status(), DeliveryStatus::Ready);
    assert_eq!(owner.pump(phase, PumpBudget::new(1)).unwrap().suppressed, 1);
    assert_eq!(pending.status(), DeliveryStatus::Suppressed);
    assert_eq!(*owner.state(), 0);
    runtime.shutdown().unwrap();

    let mut runtime = new_runtime();
    let mut owner = runtime
        .owner(0usize, NonZeroUsize::new(2).unwrap())
        .unwrap();
    owner.set_phase(phase).unwrap();
    let (first, _) = owner.try_post(phase, |_| panic!("owner callback")).unwrap();
    let (second, _) = owner.try_post(phase, |state| *state += 1).unwrap();
    assert_eq!(owner.pump(phase, PumpBudget::new(1)).unwrap().suppressed, 1);
    assert_eq!(first.status(), DeliveryStatus::Panicked);
    assert_eq!(second.status(), DeliveryStatus::Ready);
    assert_eq!(owner.pump(phase, PumpBudget::new(1)).unwrap().suppressed, 1);
    assert_eq!(second.status(), DeliveryStatus::Suppressed);
    assert_eq!(*owner.state(), 0);
    let rejected = owner.try_post(phase, |_| {}).err().unwrap();
    assert_eq!(rejected.reason, OwnerError::Faulted);
    owner.recover().unwrap();
    owner.try_post(phase, |state| *state += 1).unwrap();
    owner.pump(phase, PumpBudget::new(1)).unwrap();
    assert_eq!(*owner.state(), 1);
    owner.close();
    runtime.shutdown().unwrap();
}

#[test]
fn transferable_sender_routes_send_callback_to_non_send_owner_state() {
    let mut runtime = new_runtime();
    let phase = Phase(4);
    let local_state = Rc::new(Cell::new(0usize));
    let mut owner = runtime
        .owner(Rc::clone(&local_state), NonZeroUsize::new(1).unwrap())
        .unwrap();
    owner.set_phase(phase).unwrap();
    let sender = owner.sender();
    let after_close = sender.clone();
    let delivery = thread::spawn(move || {
        sender
            .try_post(phase, |state| state.set(state.get() + 1))
            .unwrap()
            .0
    })
    .join()
    .unwrap();
    assert_eq!(local_state.get(), 0);
    assert_eq!(owner.pump(phase, PumpBudget::new(1)).unwrap().invoked, 1);
    assert_eq!(local_state.get(), 1);
    assert_eq!(delivery.status(), DeliveryStatus::Published);
    owner.close();
    assert_eq!(
        after_close.try_post(phase, |_| {}).err().unwrap().reason,
        OwnerError::Closed
    );
    runtime.shutdown().unwrap();
}

#[test]
fn immediate_ready_panic_faults_routes_until_cleanup_and_recovery() {
    let mut runtime = new_runtime();
    let phase = Phase(2);
    let mut owner = runtime
        .owner(0usize, NonZeroUsize::new(1).unwrap())
        .unwrap();
    owner.set_phase(phase).unwrap();
    let (waiting, _) = owner.try_post(phase, |state| *state += 10).unwrap();
    let ready = Shared::ready(5usize);
    let result: ReadyAccess<(), _> = owner.with_ready(&ready, phase, |state, _| {
        *state += 1;
        std::panic::panic_any(HostilePanic);
    });
    assert!(matches!(result, ReadyAccess::Panicked));
    assert_eq!(*owner.state(), 1);
    assert_eq!(owner.recover(), Err(OwnerError::Faulted));
    assert_eq!(
        owner.try_post(phase, |_| {}).err().unwrap().reason,
        OwnerError::Faulted
    );
    assert_eq!(owner.pump(phase, PumpBudget::new(1)).unwrap().suppressed, 1);
    assert_eq!(waiting.status(), DeliveryStatus::Suppressed);
    owner.recover().unwrap();
    owner.try_post(phase, |state| *state += 1).unwrap();
    owner.pump(phase, PumpBudget::new(1)).unwrap();
    assert_eq!(*owner.state(), 2);
    owner.close();
    runtime.shutdown().unwrap();
}
