use super::*;
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn request() -> ShellRequest {
    ShellRequest {
        destination: ShellDestination::Url(String::from("https://example.invalid/a%20b")),
    }
}

fn retire(helper: &mut ShellHelper) {
    helper.close();
    let limit = Instant::now() + Duration::from_secs(3);
    while !helper.poll_closed().unwrap() {
        assert!(Instant::now() < limit, "shell test helper did not retire");
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn encoding_preserves_destination_and_rejects_ambiguous_input() {
    let encoded = request().prepare().unwrap().wide;
    assert_eq!(
        String::from_utf16(&encoded[..encoded.len() - 1]).unwrap(),
        "https://example.invalid/a%20b"
    );
    for url in ["", "relative/file", "1bad:route", "https://x\0y"] {
        assert!(
            ShellRequest {
                destination: ShellDestination::Url(url.into())
            }
            .prepare()
            .is_err()
        );
    }
    assert!(
        ShellRequest {
            destination: ShellDestination::File(PathBuf::from("relative.txt"))
        }
        .prepare()
        .is_err()
    );
    use std::os::windows::ffi::OsStringExt;
    let native = std::ffi::OsString::from_wide(&[67, 58, 92, 0xd800]);
    assert_eq!(
        ShellRequest {
            destination: ShellDestination::File(native.into())
        }
        .prepare()
        .unwrap()
        .wide,
        [67, 58, 92, 0xd800, 0]
    );
}

#[test]
fn bounded_intake_cancellation_and_close_retain_claimed_work() {
    let (started, observed) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let mut helper = ShellHelper::spawn(crate::HostId::allocate().unwrap(), 2, move || {
        move |_: &PreparedRequest| {
            started.send(()).unwrap();
            released.recv_timeout(Duration::from_secs(2)).unwrap();
            Ok(())
        }
    })
    .unwrap();
    let first = helper.try_launch(request()).unwrap();
    observed.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(!first.cancel());
    let queued = helper.try_launch(request()).unwrap();
    assert_ne!(first.id(), queued.id());
    assert_eq!(first.id(), first.clone().id());
    assert!(queued.cancel());
    assert_eq!(
        helper.try_launch(request()).unwrap_err().reason,
        ShellFailure::CapacityFull
    );
    helper.close();
    assert_eq!(
        helper.try_launch(request()).unwrap_err().reason,
        ShellFailure::Closed
    );
    assert_eq!(helper.pending(), 2);
    assert!(!helper.poll_closed().unwrap());
    release.send(()).unwrap();
    retire(&mut helper);
    assert_eq!(first.outcome(), ShellOutcome::Complete(Ok(())));
    assert_eq!(queued.outcome(), ShellOutcome::Cancelled);
    assert_eq!(helper.pending(), 0);
    assert!(
        observed.try_recv().is_err(),
        "cancelled request must not execute"
    );
}

#[test]
fn helper_fault_settles_receipt_without_losing_retirement() {
    let mut helper = ShellHelper::spawn(crate::HostId::allocate().unwrap(), 1, || {
        |_: &PreparedRequest| -> Result<(), ShellFailure> { panic!("injected executor fault") }
    })
    .unwrap();
    let receipt = helper.try_launch(request()).unwrap();
    retire(&mut helper);
    assert_eq!(
        receipt.outcome(),
        ShellOutcome::Complete(Err(ShellFailure::HelperPanicked))
    );
}

#[test]
fn native_apartment_can_start_and_retire_without_launching() {
    thread::spawn(|| {
        drop(crate::backend::windows::shell::Apartment::new().unwrap());
    })
    .join()
    .unwrap();
    let mut helper = ShellHelper::new(crate::HostId::allocate().unwrap(), 1).unwrap();
    retire(&mut helper);
}

#[test]
fn exhausted_identity_rejects_original_owned_request() {
    let host = crate::HostId::allocate().unwrap();
    let mut helper = ShellHelper::spawn(host, 1, || |_: &PreparedRequest| Ok(())).unwrap();
    lock(&helper.shared.state).serial = u64::MAX;
    let original = request();
    let rejected = helper.try_launch(original.clone()).unwrap_err();
    assert_eq!(rejected.request, original);
    assert_eq!(rejected.reason, ShellFailure::IdentityExhausted);
    assert_eq!(helper.pending(), 0);
    retire(&mut helper);
}

#[test]
#[expect(
    clippy::cast_possible_wrap,
    reason = "a COM error is a negative number written in hex"
)]
fn apartment_startup_failure_settles_every_accepted_request() {
    let failure = ShellFailure::Native {
        operation: "initialize shell apartment",
        code: 0x80010106_u32 as i32,
        message: "injected apartment failure".into(),
    };
    let expected = failure.clone();
    let mut helper = ShellHelper::spawn(crate::HostId::allocate().unwrap(), 2, move || {
        move |_: &PreparedRequest| Err(failure.clone())
    })
    .unwrap();
    let first = helper.try_launch(request()).unwrap();
    let second = helper.try_launch(request()).unwrap();
    retire(&mut helper);
    for receipt in [first, second] {
        assert_eq!(
            receipt.outcome(),
            ShellOutcome::Complete(Err(expected.clone()))
        );
    }
    assert_eq!(helper.pending(), 0);
}

#[test]
fn validation_has_no_allocations_and_preparation_preserves_native_units() {
    use std::os::windows::ffi::OsStringExt;
    let cases = [
        ShellRequest {
            destination: ShellDestination::Url("custom+thing:é/漢/%20?q=🙂".into()),
        },
        ShellRequest {
            destination: ShellDestination::File(PathBuf::from(r"C:\absolute\é.txt")),
        },
        ShellRequest {
            destination: ShellDestination::File(
                std::ffi::OsString::from_wide(&[67, 58, 92, 0xd800]).into(),
            ),
        },
    ];
    for request in cases {
        PREPARATIONS.with(|count| count.set(0));
        let (valid, counts) = crate::allocation_probe::measure(|| request.validate());
        assert_eq!(valid, Ok(()));
        assert_eq!(counts.allocations, 0);
        assert_eq!(counts.reallocations, 0);
        assert_eq!(PREPARATIONS.with(|count| count.get()), 0);
        let (prepared, counts) = crate::allocation_probe::measure(|| request.prepare().unwrap());
        let expected: Vec<_> = match &request.destination {
            ShellDestination::Url(url) => url.encode_utf16().chain(Some(0)).collect(),
            ShellDestination::File(path) => path.as_os_str().encode_wide().chain(Some(0)).collect(),
        };
        assert_eq!(prepared.wide(), expected);
        assert_eq!(counts.allocations, 1);
        assert_eq!(counts.requested_bytes, expected.len() * size_of::<u16>());
        assert_eq!(counts.reallocations, 0);
        assert_eq!(PREPARATIONS.with(|count| count.get()), 1);
        println!(
            "shell validation allocations=0 preparation allocations={} requested_bytes={} preparations=1 units={}",
            counts.allocations,
            counts.requested_bytes,
            expected.len()
        );
    }
    let invalid = ShellRequest {
        destination: ShellDestination::File(
            std::ffi::OsString::from_wide(&[67, 58, 92, 0xd800, 0]).into(),
        ),
    };
    let (result, counts) = crate::allocation_probe::measure(|| invalid.validate());
    assert_eq!(
        result,
        Err(ShellFailure::InvalidInput("file path contains NUL"))
    );
    assert_eq!(counts.allocations, 0);
}

#[test]
fn helper_encodes_accepted_requests_once_and_rejects_full_or_invalid_before_preparation() {
    let (entered, observed) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let mut helper = ShellHelper::spawn(crate::HostId::allocate().unwrap(), 1, move || {
        move |prepared: &PreparedRequest| {
            assert_eq!(prepared.request, request());
            assert_eq!(
                String::from_utf16(
                    &prepared.destination.wide()[..prepared.destination.wide().len() - 1]
                )
                .unwrap(),
                "https://example.invalid/a%20b"
            );
            entered.send(()).unwrap();
            released.recv_timeout(Duration::from_secs(3)).unwrap();
            Ok(())
        }
    })
    .unwrap();
    let accepted = request();
    PREPARATIONS.with(|count| count.set(0));
    VALIDATIONS.with(|count| count.set(0));
    let (receipt, counts) =
        crate::allocation_probe::measure(|| helper.try_launch(accepted).unwrap());
    assert_eq!(PREPARATIONS.with(|count| count.get()), 1);
    assert_eq!(VALIDATIONS.with(|count| count.get()), 1);
    assert_eq!(
        counts.allocations, 2,
        "one destination backing plus one receipt"
    );
    assert_eq!(counts.reallocations, 0);
    observed.recv_timeout(Duration::from_secs(3)).unwrap();
    println!(
        "shell accepted validations=1 preparations=1 admission allocations={} requested_bytes={} (buffer+receipt)",
        counts.allocations, counts.requested_bytes
    );
    let original = request();
    PREPARATIONS.with(|count| count.set(0));
    FAIL_NEXT_PREPARATION.with(|fail| fail.set(true));
    let submitted = original.clone();
    let (rejected, counts) =
        crate::allocation_probe::measure(|| helper.try_launch(submitted).unwrap_err());
    assert_eq!(rejected.request, original);
    assert_eq!(rejected.reason, ShellFailure::CapacityFull);
    assert_eq!(PREPARATIONS.with(|count| count.get()), 0);
    assert!(FAIL_NEXT_PREPARATION.with(|fail| fail.get()));
    assert_eq!(counts.allocations, 0);
    assert_eq!(counts.reallocations, 0);
    let invalid = ShellRequest {
        destination: ShellDestination::Url("relative/path".into()),
    };
    let submitted = invalid.clone();
    let (rejected, counts) =
        crate::allocation_probe::measure(|| helper.try_launch(submitted).unwrap_err());
    assert_eq!(counts.allocations, 0);
    assert_eq!(counts.reallocations, 0);
    assert_eq!(rejected.request, invalid);
    assert!(matches!(rejected.reason, ShellFailure::InvalidInput(_)));
    assert_eq!(PREPARATIONS.with(|count| count.get()), 0);
    helper.close();
    let rejected = helper.try_launch(invalid.clone()).unwrap_err();
    assert_eq!(rejected.request, invalid);
    assert!(matches!(rejected.reason, ShellFailure::InvalidInput(_)));
    let submitted = original.clone();
    let (rejected, counts) =
        crate::allocation_probe::measure(|| helper.try_launch(submitted).unwrap_err());
    assert_eq!(counts.allocations, 0);
    assert_eq!(counts.reallocations, 0);
    assert_eq!(rejected.request, original);
    assert_eq!(rejected.reason, ShellFailure::Closed);
    assert_eq!(PREPARATIONS.with(|count| count.get()), 0);
    FAIL_NEXT_PREPARATION.with(|fail| fail.set(false));
    println!(
        "shell already-full/invalid/closed preparations=0 allocations=0 (CapacityFull precedes injected preparation failure)"
    );
    release.send(()).unwrap();
    retire(&mut helper);
    assert_eq!(receipt.outcome(), ShellOutcome::Complete(Ok(())));
}

#[test]
fn preparation_failure_returns_original_without_receipt_identity_or_queue_commit() {
    let mut helper = ShellHelper::spawn(crate::HostId::allocate().unwrap(), 1, || {
        |_: &PreparedRequest| Ok(())
    })
    .unwrap();
    let original = request();
    FAIL_NEXT_PREPARATION.with(|fail| fail.set(true));
    let rejected = helper.try_launch(original.clone()).unwrap_err();
    assert_eq!(rejected.request, original);
    assert_eq!(rejected.reason, ShellFailure::AllocationFailed);
    let state = lock(&helper.shared.state);
    assert_eq!(state.serial, 0);
    assert_eq!(state.accepted, 0);
    assert!(state.queue.is_empty());
    drop(state);
    let receipt = helper.try_launch(original).unwrap();
    assert_eq!(receipt.id.serial, 1);
    retire(&mut helper);
    assert_eq!(receipt.outcome(), ShellOutcome::Complete(Ok(())));
}

#[test]
fn final_admission_rechecks_open_capacity_and_identity_after_outside_lock_preparation() {
    let original = request();
    for reason in [
        ShellFailure::Closed,
        ShellFailure::CapacityFull,
        ShellFailure::IdentityExhausted,
    ] {
        // A closed helper is never reopened: its waiting thread may already
        // have observed closure, including after a spurious condvar wake.
        let mut helper = ShellHelper::spawn(crate::HostId::allocate().unwrap(), 1, || {
            |_: &PreparedRequest| Ok(())
        })
        .unwrap();
        assert_eq!(lock(&helper.shared.state).admission_failure(), None);
        let prepared = PreparedRequest {
            request: original.clone(),
            destination: original.prepare().unwrap(),
        };
        {
            let mut state = lock(&helper.shared.state);
            match reason {
                ShellFailure::Closed => state.open = false,
                ShellFailure::CapacityFull => state.accepted = state.capacity,
                ShellFailure::IdentityExhausted => state.serial = u64::MAX,
                _ => unreachable!(),
            }
        }
        let (rejected, counts) =
            crate::allocation_probe::measure(|| helper.admit(prepared).unwrap_err());
        assert_eq!(rejected.request, original);
        assert_eq!(rejected.reason, reason);
        assert_eq!(counts.allocations, 0);
        assert_eq!(
            counts.deallocations, 1,
            "private encoded backing disposed, original request retained"
        );
        {
            let mut state = lock(&helper.shared.state);
            assert!(state.queue.is_empty());
            // Clear only the fixture's synthetic accepted count.
            state.accepted = 0;
        }
        retire(&mut helper);
    }
    let mut helper = ShellHelper::spawn(crate::HostId::allocate().unwrap(), 1, || {
        |_: &PreparedRequest| Ok(())
    })
    .unwrap();
    let receipt = helper.try_launch(original).unwrap();
    assert_eq!(receipt.id.serial, 1);
    retire(&mut helper);
    assert_eq!(receipt.outcome(), ShellOutcome::Complete(Ok(())));
}

#[test]
fn helper_init_panic_still_settles_accepted_owned_preparation_and_retires() {
    let mut helper = ShellHelper::spawn(crate::HostId::allocate().unwrap(), 1, || {
        panic!("injected helper init panic");
        #[expect(unreachable_code, reason = "the call before it never returns")]
        |_: &PreparedRequest| Ok(())
    })
    .unwrap();
    let receipt = helper.try_launch(request()).unwrap();
    retire(&mut helper);
    assert_eq!(
        receipt.outcome(),
        ShellOutcome::Complete(Err(ShellFailure::HelperPanicked))
    );
    assert_eq!(helper.pending(), 0);
}
