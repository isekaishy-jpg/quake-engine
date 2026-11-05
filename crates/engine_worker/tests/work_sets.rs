use std::num::NonZeroUsize;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use engine_worker::{
    Cost, DeliveryStatus, DemandError, DiscoveryError, ExecutionClass, ExternalOptions, Limits,
    Outcome, OwnedLimits, OwnerError, Phase, Priority, PumpBudget, Runtime, RuntimeConfig, Shared,
    SpawnError, SpawnOptions, StageOptions, TaskStatus, WorkerConfig,
};

const TIMEOUT: Duration = Duration::from_secs(10);

fn runtime() -> Runtime {
    let config = RuntimeConfig::new(3, [WorkerConfig::new(1); 3]).unwrap();
    let limits = OwnedLimits::new(16, 16, [8; 3], [8; 3]).unwrap();
    Runtime::builder(config)
        .with_owned_limits(limits)
        .with_demand_limits(vec![Priority::new(0), Priority::new(1)], 8)
        .build()
        .unwrap()
}

#[test]
fn seal_preserves_bounded_discovery_and_child_accounting() {
    let mut runtime = runtime();
    let lane = runtime.lane(ExecutionClass::Low);
    let set = runtime.work_set(NonZeroUsize::new(2).unwrap()).unwrap();
    let permit = set.discovery().unwrap();
    let second = permit.fork().unwrap();
    assert!(matches!(permit.fork(), Err(DiscoveryError::Full)));

    set.seal();
    let rejected = set
        .try_spawn(&lane, SpawnOptions::default(), || 1)
        .err()
        .unwrap();
    assert_eq!(rejected.reason, SpawnError::Closed);
    assert!(!set.is_drained());

    let (release_send, release_recv) = mpsc::channel();
    let (child, _) = permit
        .try_spawn(&lane, SpawnOptions::default(), move || {
            release_recv.recv_timeout(TIMEOUT).unwrap();
            7
        })
        .unwrap();
    drop(permit);
    drop(second);
    assert_eq!(set.progress().discovery_permits, 0);
    assert_eq!(set.progress().active_work, 1);
    assert!(!set.is_drained());

    release_send.send(()).unwrap();
    assert_eq!(child.completion().wait().unwrap(), TaskStatus::Succeeded);
    set.wait_drained().unwrap();
    assert!(set.is_drained());
    // The result remains available independently of its producer's set.
    assert_eq!(child.status(), Some(TaskStatus::Succeeded));
    runtime.shutdown().unwrap();
}

#[test]
fn cancellation_rejects_descendants_and_suppresses_unclaimed_owned_work() {
    let mut runtime = runtime();
    let lane = runtime.lane(ExecutionClass::Mid);
    let set = runtime.work_set(NonZeroUsize::new(1).unwrap()).unwrap();
    let permit = set.discovery().unwrap();
    let (started_send, started_recv) = mpsc::channel();
    let (release_send, release_recv) = mpsc::channel();
    let (running, _) = set
        .try_spawn(&lane, SpawnOptions::default(), move || {
            started_send.send(()).unwrap();
            release_recv.recv_timeout(TIMEOUT).unwrap();
        })
        .unwrap();
    started_recv.recv_timeout(TIMEOUT).unwrap();
    let (mut pending, _) = set.try_spawn(&lane, SpawnOptions::default(), || 9).unwrap();

    set.cancel();
    assert!(set.progress().cancelled);
    assert_eq!(pending.completion().wait().unwrap(), TaskStatus::Cancelled);
    assert_eq!(pending.try_take(), Some(Outcome::Cancelled));
    assert!(matches!(permit.fork(), Err(DiscoveryError::Cancelled)));
    let rejected = permit
        .try_spawn(&lane, SpawnOptions::default(), || 10)
        .err()
        .unwrap();
    assert_eq!(rejected.reason, SpawnError::Closed);
    release_send.send(()).unwrap();
    assert_eq!(running.completion().wait().unwrap(), TaskStatus::Succeeded);
    drop(permit);
    set.wait_drained().unwrap();
    runtime.shutdown().unwrap();
}

#[test]
fn cancelling_one_consumer_set_preserves_shared_producer_and_other_delivery() {
    let mut runtime = runtime();
    let lane = runtime.lane(ExecutionClass::High);
    let producer_set = runtime.work_set(NonZeroUsize::new(1).unwrap()).unwrap();
    let first_set = runtime.work_set(NonZeroUsize::new(1).unwrap()).unwrap();
    let second_set = runtime.work_set(NonZeroUsize::new(1).unwrap()).unwrap();
    let phase = Phase(5);
    let mut owner = runtime
        .owner(Vec::<i32>::new(), NonZeroUsize::new(2).unwrap())
        .unwrap();
    owner.set_phase(phase).unwrap();

    let (release_send, release_recv) = mpsc::channel();
    let (task, _) = producer_set
        .try_spawn(&lane, SpawnOptions::default(), move || {
            release_recv.recv_timeout(TIMEOUT).unwrap();
            42
        })
        .unwrap();
    let shared = task.into_shared();
    let completion = shared.completion();
    let (first, _) = owner
        .on_ready_in(&first_set, &completion, phase, |values, status| {
            assert_eq!(status, TaskStatus::Succeeded);
            values.push(1);
        })
        .unwrap();
    let (second, _) = owner
        .on_ready_in(&second_set, &completion, phase, |values, status| {
            assert_eq!(status, TaskStatus::Succeeded);
            values.push(2);
        })
        .unwrap();
    let first_demand = completion.demand_in(&first_set, Priority::new(0)).unwrap();
    let second_demand = completion.demand_in(&second_set, Priority::new(1)).unwrap();
    assert_eq!(first_set.progress().active_work, 2);
    assert_eq!(second_set.progress().active_work, 2);
    producer_set.seal();
    second_set.seal();
    first_set.cancel();
    assert_eq!(first_demand.promote(), Err(DemandError::Gone));
    assert!(second_demand.promote().is_ok());
    assert_eq!(first_set.progress().active_work, 1);
    assert!(!first_set.is_drained());
    assert_eq!(first.status(), DeliveryStatus::Waiting);

    let waiting = completion.clone();
    let waiter = thread::spawn(move || waiting.wait().unwrap());
    release_send.send(()).unwrap();
    assert_eq!(waiter.join().unwrap(), TaskStatus::Succeeded);
    let demand_deadline = Instant::now() + TIMEOUT;
    while second_set.progress().active_work != 1 {
        assert!(
            Instant::now() < demand_deadline,
            "consumer demand did not detach"
        );
        thread::yield_now();
    }
    assert!(!second_set.is_drained());
    let deadline = Instant::now() + TIMEOUT;
    let mut invoked = 0;
    let mut suppressed = 0;
    while invoked + suppressed < 2 {
        let report = owner.pump(phase, PumpBudget::new(2)).unwrap();
        invoked += report.invoked;
        suppressed += report.suppressed;
        assert!(Instant::now() < deadline, "owner deliveries did not settle");
        thread::yield_now();
    }
    assert_eq!(invoked, 1);
    assert_eq!(suppressed, 1);
    assert_eq!(first.status(), DeliveryStatus::Suppressed);
    assert_eq!(second.status(), DeliveryStatus::Published);
    assert_eq!(owner.state().as_slice(), &[2]);
    owner.close();
    first_set.wait_drained().unwrap();
    second_set.wait_drained().unwrap();
    producer_set.wait_drained().unwrap();
    assert_eq!(&*shared.try_result().unwrap(), &Outcome::Success(42));
    runtime.shutdown().unwrap();
}

#[test]
fn promised_consumer_delivery_keeps_set_active_through_owner_cleanup() {
    let mut runtime = runtime();
    let set = runtime.work_set(NonZeroUsize::new(1).unwrap()).unwrap();
    let phase = Phase(9);
    let mut owner = runtime
        .owner(0usize, NonZeroUsize::new(1).unwrap())
        .unwrap();
    owner.set_phase(phase).unwrap();
    let prepared = owner
        .prepare_delivery_in(&set, phase, |value| *value += 1)
        .unwrap();
    set.cancel();
    assert!(!set.is_drained());
    prepared.ticket.ready();
    assert_eq!(owner.pump(phase, PumpBudget::new(1)).unwrap().suppressed, 1);
    assert_eq!(prepared.delivery.status(), DeliveryStatus::Suppressed);
    assert_eq!(*owner.state(), 0);
    owner.close();
    set.wait_drained().unwrap();
    runtime.shutdown().unwrap();
}

#[test]
fn protected_delivery_slot_survives_ordinary_owner_pressure() {
    let config = RuntimeConfig::new(3, [WorkerConfig::new(1); 3]).unwrap();
    let owned = OwnedLimits::new(8, 8, [4; 3], [2; 3]).unwrap();
    let limits = Limits::new(Cost::new(8, 8, 1, 0), Cost::new(1, 0, 1, 0), 1, None).unwrap();
    let mut runtime = Runtime::builder(config)
        .with_owned_limits(owned)
        .with_capacity_limits(limits)
        .build()
        .unwrap();
    let phase = Phase(11);
    let mut owner = runtime
        .owner(Vec::<i32>::new(), NonZeroUsize::new(1).unwrap())
        .unwrap();
    owner.set_phase(phase).unwrap();
    let ordinary = owner
        .prepare_delivery(phase, |values| values.push(1))
        .unwrap();
    assert!(
        owner
            .prepare_delivery(phase, |values| values.push(3))
            .is_err()
    );

    let set = runtime.work_set(NonZeroUsize::new(1).unwrap()).unwrap();
    let reservation = runtime.reserve_required(Cost::new(1, 0, 1, 0)).unwrap();
    let required = owner
        .prepare_delivery_reserved_in(&set, &reservation, phase, |values| values.push(2))
        .unwrap();
    ordinary.ticket.ready();
    let lane = runtime.lane(ExecutionClass::Mid);
    let group = lane.group().unwrap();
    let (task, _) = lane
        .try_spawn_stage(
            StageOptions {
                group: Some(&group),
                work_set: Some(&set),
                reservation: Some(&reservation),
                cost: Cost::new(1, 0, 1, 0),
                delivery: Some(required.ticket),
                ..StageOptions::default()
            },
            || 42,
        )
        .unwrap();
    set.seal();
    group.seal();
    group.wait_helping().unwrap();
    assert_eq!(task.status(), Some(TaskStatus::Succeeded));
    drop(reservation);
    assert_eq!(runtime.capacity_usage().unwrap().required.deliveries, 1);
    assert!(!set.is_drained());
    let report = owner.pump(phase, PumpBudget::new(2)).unwrap();
    assert_eq!(report.invoked, 2);
    assert_eq!(owner.state().as_slice(), &[1, 2]);
    owner.close();
    set.wait_drained().unwrap();
    assert_eq!(runtime.capacity_usage().unwrap().required.deliveries, 0);
    runtime.shutdown().unwrap();
}

#[test]
fn reserved_subscriptions_reuse_ready_and_pending_results_through_consumer_cleanup() {
    for case in 0..3 {
        let config = RuntimeConfig::new(3, [WorkerConfig::new(1); 3]).unwrap();
        let mut runtime = Runtime::builder(config)
            .with_owned_limits(OwnedLimits::new(4, 4, [4; 3], [1; 3]).unwrap())
            .with_capacity_limits(
                Limits::new(Cost::new(4, 4, 1, 0), Cost::new(0, 0, 1, 0), 1, None).unwrap(),
            )
            .build()
            .unwrap();
        let lane = runtime.lane(ExecutionClass::Low);
        let group = lane.group().unwrap();
        let (release_send, release_recv) = mpsc::channel();
        let (started_send, started_recv) = mpsc::channel();
        let shared = if case == 0 {
            Shared::ready(42)
        } else {
            let (task, _) = lane
                .try_spawn_in(&group, SpawnOptions::default(), move || {
                    started_send.send(()).unwrap();
                    release_recv.recv_timeout(TIMEOUT).unwrap();
                    42
                })
                .unwrap();
            started_recv.recv_timeout(TIMEOUT).unwrap();
            task.into_shared()
        };
        let phase = Phase(12);
        let mut owner = runtime
            .owner(Vec::<i32>::new(), NonZeroUsize::new(1).unwrap())
            .unwrap();
        owner.set_phase(phase).unwrap();
        owner.try_post(phase, |values| values.push(1)).unwrap();
        let set = runtime.work_set(NonZeroUsize::new(1).unwrap()).unwrap();
        let reserve = runtime.reserve_required(Cost::new(0, 0, 1, 0)).unwrap();
        let completion = shared.completion();
        let retained = shared.clone();
        let callback = move |values: &mut Vec<i32>, status| {
            assert_eq!(status, TaskStatus::Succeeded);
            assert_eq!(&*retained.try_result().unwrap(), &Outcome::Success(42));
            values.push(2);
        };
        let (delivery, _) = if case == 1 {
            let permit = set.discovery().unwrap();
            set.seal();
            owner
                .on_ready_reserved_from(&permit, &reserve, &completion, phase, callback)
                .unwrap()
        } else {
            let result = owner
                .on_ready_reserved_in(&set, &reserve, &completion, phase, callback)
                .unwrap();
            set.seal();
            result
        };
        // The subscription consumes only the reserved delivery, never a CPU
        // record. Failed additional subscription preserves the caller's closure.
        assert_eq!(runtime.capacity_usage().unwrap().required.records, 0);
        let before = reserve.available();
        let rejected = owner
            .on_ready_reserved(&reserve, &completion, phase, |values, _| values.push(9))
            .err()
            .unwrap();
        assert_eq!(rejected.reason, OwnerError::Full);
        assert_eq!(reserve.available(), before);
        let mut scratch = Vec::new();
        (rejected.callback)(&mut scratch, TaskStatus::Succeeded);
        assert_eq!(scratch, [9]);
        if case == 2 {
            set.cancel();
            assert!(!set.is_drained());
            let report = owner.pump(phase, PumpBudget::new(2)).unwrap();
            assert_eq!((report.invoked, report.suppressed), (1, 1));
            assert!(set.is_drained());
            assert_eq!(reserve.available().deliveries, 1);
            assert_eq!(shared.status(), None);
        }
        if case != 0 {
            release_send.send(()).unwrap();
        }
        group.seal();
        group.wait_helping().unwrap();
        if case != 2 {
            assert!(
                owner.state().is_empty(),
                "ready registration must stay deferred"
            );
            assert_eq!(owner.pump(phase, PumpBudget::new(2)).unwrap().invoked, 2);
            assert_eq!(delivery.status(), DeliveryStatus::Published);
            assert_eq!(owner.state().as_slice(), &[1, 2]);
        } else {
            assert_eq!(delivery.status(), DeliveryStatus::Suppressed);
            assert_eq!(owner.state().as_slice(), &[1]);
        }
        assert_eq!(&*shared.try_result().unwrap(), &Outcome::Success(42));
        assert_eq!(reserve.available().deliveries, 1);
        owner.close();
        set.wait_drained().unwrap();
        drop(reserve);
        assert_eq!(runtime.capacity_usage().unwrap().required.deliveries, 0);
        runtime.shutdown().unwrap();
    }
}

#[test]
fn descendant_consumers_attach_after_root_closure_without_reservations() {
    for ready in [false, true] {
        for cancel in [false, true] {
            let mut runtime = runtime();
            let mut foreign_runtime = self::runtime();
            let foreign_set = foreign_runtime
                .work_set(NonZeroUsize::new(1).unwrap())
                .unwrap();
            let foreign_permit = foreign_set.discovery().unwrap();
            let set = runtime.work_set(NonZeroUsize::new(1).unwrap()).unwrap();
            let permit = set.discovery().unwrap();
            let phase = Phase(13);
            let mut owner = runtime
                .owner(Vec::<u32>::new(), NonZeroUsize::new(1).unwrap())
                .unwrap();
            owner.set_phase(phase).unwrap();
            let (producer, task, _) = runtime.external::<u32>(ExternalOptions::default()).unwrap();
            let shared = task.into_shared();
            let completion = shared.completion();
            let producer = if ready {
                producer.complete(42).unwrap();
                None
            } else {
                Some(producer)
            };
            runtime.begin_shutdown();
            assert!(set.progress().sealed);
            assert!(matches!(
                completion.demand_in(&set, Priority::new(0)),
                Err(DemandError::Closed)
            ));
            assert_eq!(
                owner
                    .on_ready_in(&set, &completion, phase, |_, _| {})
                    .err()
                    .unwrap()
                    .reason,
                OwnerError::Closed
            );
            assert!(matches!(
                completion.demand_from(&foreign_permit, Priority::new(0)),
                Err(DemandError::Closed)
            ));
            assert_eq!(
                owner
                    .on_ready_from(&foreign_permit, &completion, phase, |_, _| {})
                    .err()
                    .unwrap()
                    .reason,
                OwnerError::Closed
            );
            assert_eq!(foreign_set.progress().active_work, 0);

            let demand = if ready {
                assert!(matches!(
                    completion.demand_from(&permit, Priority::new(0)),
                    Err(DemandError::Gone)
                ));
                None
            } else {
                assert!(matches!(
                    completion.demand_from(&permit, Priority::new(9)),
                    Err(DemandError::UnknownPriority)
                ));
                assert_eq!(set.progress().active_work, 0);
                let occupied: Vec<_> = (0..8)
                    .map(|_| completion.demand(Priority::new(1)).unwrap())
                    .collect();
                assert!(matches!(
                    completion.demand_from(&permit, Priority::new(0)),
                    Err(DemandError::Full)
                ));
                assert_eq!(set.progress().active_work, 0);
                drop(occupied);
                let demand = completion.demand_from(&permit, Priority::new(0)).unwrap();
                assert_eq!(set.progress().active_work, 1);
                drop(demand);
                assert_eq!(set.progress().active_work, 0);
                Some(completion.demand_from(&permit, Priority::new(0)).unwrap())
            };
            assert_eq!(set.progress().active_work, usize::from(!ready));
            let captured = shared.clone();
            let (delivery, _) = owner
                .on_ready_from(&permit, &completion, phase, move |values, status| {
                    assert_eq!(status, TaskStatus::Succeeded);
                    let outcome = captured.try_result().unwrap();
                    let Outcome::Success(value) = &*outcome else {
                        panic!("producer did not succeed");
                    };
                    values.push(**value);
                })
                .unwrap();
            assert!(
                owner.state().is_empty(),
                "ready subscriptions must remain deferred"
            );
            let before = set.progress().active_work;
            let rejected = owner
                .on_ready_from(&permit, &completion, phase, |values, _| values.push(7))
                .err()
                .unwrap();
            assert_eq!(rejected.reason, OwnerError::Full);
            assert_eq!(set.progress().active_work, before);
            let mut scratch = Vec::new();
            (rejected.callback)(&mut scratch, TaskStatus::Succeeded);
            assert_eq!(scratch, [7]);

            if cancel {
                set.cancel();
                if let Some(demand) = &demand {
                    assert_eq!(demand.promote(), Err(DemandError::Gone));
                }
                assert!(matches!(
                    completion.demand_from(&permit, Priority::new(0)),
                    Err(DemandError::Closed)
                ));
                assert_eq!(
                    owner
                        .on_ready_from(&permit, &completion, phase, |_, _| {})
                        .err()
                        .unwrap()
                        .reason,
                    OwnerError::Closed
                );
            }
            assert!(
                !set.is_drained(),
                "owner-local cleanup is still outstanding"
            );
            if let Some(producer) = producer {
                assert_eq!(
                    shared.status(),
                    None,
                    "consumer cancellation must not cancel the producer"
                );
                producer.complete(42).unwrap();
            }
            assert_eq!(set.progress().active_work, 1);
            let report = owner.pump(phase, PumpBudget::new(1)).unwrap();
            assert_eq!(
                (report.invoked, report.suppressed),
                if cancel { (0, 1) } else { (1, 0) }
            );
            assert_eq!(
                delivery.status(),
                if cancel {
                    DeliveryStatus::Suppressed
                } else {
                    DeliveryStatus::Published
                }
            );
            assert_eq!(
                owner.state().as_slice(),
                if cancel { &[][..] } else { &[42][..] }
            );
            assert_eq!(set.progress().active_work, 0);
            assert!(!set.is_drained(), "discovery remains live until released");
            assert_eq!(shared.status(), Some(TaskStatus::Succeeded));
            drop(demand);
            owner.close();
            assert_eq!(
                owner
                    .on_ready_from(&permit, &completion, phase, |_, _| {})
                    .err()
                    .unwrap()
                    .reason,
                OwnerError::Closed
            );
            assert_eq!(set.progress().active_work, 0);
            drop(permit);
            assert!(set.is_drained());
            runtime.shutdown().unwrap();
            drop(foreign_permit);
            foreign_set.seal();
            foreign_runtime.shutdown().unwrap();
        }
    }
}
