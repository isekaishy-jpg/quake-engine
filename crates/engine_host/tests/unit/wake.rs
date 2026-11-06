use super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn close_waits_for_admitted_native_send_and_seals_surviving_clones() {
    let wake = NativeWake::new();
    let (entered, observed) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let released = Mutex::new(released);
    let calls = Arc::new(AtomicUsize::new(0));
    let called = Arc::clone(&calls);
    wake.lock().send = Some(Arc::new(move || {
        called.fetch_add(1, Ordering::SeqCst);
        entered.send(()).unwrap();
        released
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(3))
            .unwrap();
        Ok(())
    }));
    let sending = wake.clone();
    let sender = std::thread::spawn(move || sending.post());
    observed.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(
        wake.state.try_lock().is_err(),
        "send must retain the native final-access lock"
    );
    let closing = wake.clone();
    let (closed, completed) = mpsc::channel();
    let closer = std::thread::spawn(move || {
        closing.close();
        closed.send(()).unwrap();
    });
    assert!(completed.try_recv().is_err());
    release.send(()).unwrap();
    sender.join().unwrap().unwrap();
    completed.recv_timeout(Duration::from_secs(3)).unwrap();
    closer.join().unwrap();
    assert_eq!(wake.post(), Err(Error::AdmissionClosed));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    wake.close();
}

#[test]
fn unwind_guard_closes_before_borrowed_native_owner_is_destroyed() {
    struct Owner {
        wake: NativeWake,
        destroyed: Arc<AtomicBool>,
    }
    impl Drop for Owner {
        fn drop(&mut self) {
            assert!(
                self.wake.lock().closed,
                "owner destruction must follow gate closure"
            );
            self.destroyed.store(true, Ordering::SeqCst);
        }
    }
    let wake = NativeWake::new();
    let destroyed = Arc::new(AtomicBool::new(false));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _owner = Owner {
            wake: wake.clone(),
            destroyed: Arc::clone(&destroyed),
        };
        let _guard = wake.close_guard();
        panic!("injected borrowed-loop panic");
    }));
    assert!(result.is_err());
    assert!(destroyed.load(Ordering::SeqCst));
    assert_eq!(wake.post(), Err(Error::AdmissionClosed));
}
