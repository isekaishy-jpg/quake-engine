//! What a cache counts against its budget.

use engine_cache::{AccountingDomain, AccountingError, AllocationClass, PolicyCounter};
use std::sync::{Arc, Barrier};
use std::thread;

#[test]
fn debug_formatters_can_reenter_accounting_without_holding_its_lock() {
    use std::fmt::{self, Debug, Write};
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::time::Duration;

    struct ObservingWriter {
        request: Sender<()>,
        reply: Receiver<u64>,
    }

    impl Write for ObservingWriter {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            self.request.send(()).unwrap();
            // Bound a regression's deadlock, rather than leave the suite hanging.
            match self.reply.recv_timeout(Duration::from_secs(5)) {
                Ok(7) => Ok(()),
                _ => Err(fmt::Error),
            }
        }
    }

    fn check(value: &impl Debug, domain: &AccountingDomain) {
        thread::scope(|scope| {
            let (request, requests) = mpsc::channel();
            let (reply, replies) = mpsc::channel();
            let observer = scope.spawn(move || {
                while requests.recv().is_ok() {
                    let total = domain.snapshot().total_declared_bytes;
                    if reply.send(total).is_err() {
                        break;
                    }
                }
            });
            let mut writer = ObservingWriter {
                request,
                reply: replies,
            };
            let result = write!(&mut writer, "{value:?}");
            // Release the request channel before joining, including on failure.
            drop(writer);
            observer.join().unwrap();
            assert!(
                result.is_ok(),
                "Debug held an accounting lock across the writer callback"
            );
        });
    }

    let source = AccountingDomain::new();
    let charge = source.charge(7, AllocationClass::Resident).unwrap();
    check(&source, &source);
    check(&charge, &source);
    let full = AccountingDomain::new();
    let _full_charge = full.charge(u64::MAX, AllocationClass::Resident).unwrap();
    let failure = charge
        .transfer(&full, AllocationClass::Retiring)
        .unwrap_err();
    check(&failure, &source);
}

#[test]
fn lifecycle_transitions_preserve_complete_capacity() {
    let domain = AccountingDomain::new();
    let mut charge = domain.charge(128, AllocationClass::Temporary).unwrap();
    for class in [
        AllocationClass::Resident,
        AllocationClass::RetainedCapacity,
        AllocationClass::Retiring,
        AllocationClass::Retiring,
    ] {
        charge.transition(class);
        let snapshot = domain.snapshot();
        assert_eq!(charge.class(), class);
        assert_eq!(charge.declared_bytes(), 128);
        assert_eq!(snapshot.total_declared_bytes, 128);
        assert_eq!(
            snapshot.temporary_bytes
                + snapshot.resident_bytes
                + snapshot.retiring_bytes
                + snapshot.retained_capacity_bytes,
            128,
        );
    }
    assert_eq!(domain.snapshot().retiring_bytes, 128);
    drop(charge);
    assert_eq!(domain.snapshot().total_declared_bytes, 0);
}

#[test]
fn domain_handles_do_not_control_charge_lifetime() {
    let domain = AccountingDomain::new();
    let observer = domain.clone();
    let charge = domain.charge(31, AllocationClass::Resident).unwrap();
    drop(domain);
    assert_eq!(observer.snapshot().resident_bytes, 31);
    let moved_charge = charge;
    assert_eq!(observer.snapshot().resident_bytes, 31);
    drop(moved_charge);
    assert_eq!(observer.snapshot().total_declared_bytes, 0);
}

#[test]
fn checked_admission_accepts_zero_and_rejects_total_overflow_unchanged() {
    let domain = AccountingDomain::new();
    let full = domain.charge(u64::MAX, AllocationClass::Temporary).unwrap();
    let zero = domain.charge(0, AllocationClass::Retiring).unwrap();
    let before = domain.snapshot();
    assert_eq!(
        domain.charge(1, AllocationClass::Resident).unwrap_err(),
        AccountingError::Overflow,
    );
    assert_eq!(domain.snapshot(), before);
    drop(zero);
    assert_eq!(domain.snapshot(), before);
    drop(full);
    assert_eq!(domain.snapshot().total_declared_bytes, 0);
}

#[test]
fn transfers_move_one_charge_between_domains_or_classes() {
    let source = AccountingDomain::new();
    let destination = AccountingDomain::new();
    let charge = source.charge(23, AllocationClass::Temporary).unwrap();
    let charge = charge
        .transfer(&source.clone(), AllocationClass::Resident)
        .unwrap();
    assert_eq!(source.snapshot().resident_bytes, 23);
    let charge = charge
        .transfer(&destination, AllocationClass::RetainedCapacity)
        .unwrap();
    assert_eq!(source.snapshot().total_declared_bytes, 0);
    assert_eq!(destination.snapshot().retained_capacity_bytes, 23);
    drop(charge);
    assert_eq!(destination.snapshot().total_declared_bytes, 0);
}

#[test]
fn failed_transfer_preserves_original_charge_for_retry() {
    let source = AccountingDomain::new();
    let destination = AccountingDomain::new();
    let charge = source.charge(7, AllocationClass::Temporary).unwrap();
    let full = destination
        .charge(u64::MAX, AllocationClass::Resident)
        .unwrap();
    let source_before = source.snapshot();
    let destination_before = destination.snapshot();
    let failure = charge
        .transfer(&destination, AllocationClass::Retiring)
        .unwrap_err();
    assert_eq!(failure.error(), AccountingError::Overflow);
    assert_eq!(source.snapshot(), source_before);
    assert_eq!(destination.snapshot(), destination_before);
    let (error, charge) = failure.into_parts();
    assert_eq!(error, AccountingError::Overflow);
    assert_eq!(charge.class(), AllocationClass::Temporary);
    assert_eq!(charge.declared_bytes(), 7);
    drop(full);
    let charge = charge
        .transfer(&destination, AllocationClass::Retiring)
        .unwrap();
    assert_eq!(source.snapshot().total_declared_bytes, 0);
    assert_eq!(destination.snapshot().retiring_bytes, 7);
    drop(charge);
    assert_eq!(destination.snapshot().total_declared_bytes, 0);
}

#[test]
fn opposite_direction_transfers_and_shared_domain_charges_remain_coherent() {
    let left = AccountingDomain::new();
    let right = AccountingDomain::new();
    let left_charge = left.charge(11, AllocationClass::Resident).unwrap();
    let right_charge = right.charge(17, AllocationClass::Resident).unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let handles: Vec<_> = [(left_charge, right.clone()), (right_charge, left.clone())]
        .into_iter()
        .map(|(charge, destination)| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let charge = charge
                    .transfer(&destination, AllocationClass::Retiring)
                    .unwrap();
                let temporary = destination.charge(5, AllocationClass::Temporary).unwrap();
                barrier.wait();
                barrier.wait();
                drop(temporary);
                charge
            })
        })
        .collect();
    barrier.wait();
    barrier.wait();
    assert_eq!(left.snapshot().retiring_bytes, 17);
    assert_eq!(right.snapshot().retiring_bytes, 11);
    assert_eq!(left.snapshot().temporary_bytes, 5);
    assert_eq!(right.snapshot().temporary_bytes, 5);
    barrier.wait();
    let charges: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert_eq!(left.snapshot().total_declared_bytes, 17);
    assert_eq!(right.snapshot().total_declared_bytes, 11);
    drop(charges);
    assert_eq!(left.snapshot().total_declared_bytes, 0);
    assert_eq!(right.snapshot().total_declared_bytes, 0);
}

#[test]
fn policy_release_and_duplicate_read_counts_are_independent_of_backing() {
    let domain = AccountingDomain::new();
    let mut policy = PolicyCounter::new(40);
    let mut charge = domain.charge(40, AllocationClass::Resident).unwrap();
    policy.try_sub(40).unwrap();
    charge.transition(AllocationClass::Retiring);
    assert_eq!(policy.value(), 0);
    assert_eq!(domain.snapshot().retiring_bytes, 40);
    drop(charge);
    assert_eq!(domain.snapshot().total_declared_bytes, 0);

    let charge = domain.charge(40, AllocationClass::Resident).unwrap();
    policy.try_add(40).unwrap();
    policy.try_add(40).unwrap();
    assert_eq!(policy.value(), 80);
    assert_eq!(domain.snapshot().resident_bytes, 40);
    drop(charge);
    assert_eq!(domain.snapshot().total_declared_bytes, 0);
    assert_eq!(policy.value(), 80);
}
