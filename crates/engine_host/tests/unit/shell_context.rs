use super::*;
use crate::backend::{BackendOps, test::TestBackend};
use crate::{ContextPhase, HostState, ShellDestination, ShellOutcome, StopProgress};
use std::sync::mpsc;
use std::time::{Duration, Instant};

struct App {
    stops: usize,
}
impl Application for App {
    type Message = ();
    type LocalEvent = ();
    fn started(&mut self, cx: &mut Context<'_, Self>) -> Result<(), Error> {
        cx.create_window(crate::WindowSpec::default())
            .map_err(|rejected| rejected.into_parts().1)?;
        Ok(())
    }
    fn stopping(&mut self, _: &mut Context<'_, Self>) -> StopProgress {
        self.stops += 1;
        StopProgress::Settled
    }
}
fn request() -> ShellRequest {
    ShellRequest {
        destination: ShellDestination::Url("https://example.invalid/a%20b".into()),
    }
}

#[test]
fn slow_shell_final_access_keeps_window_after_application_settles() {
    let mut core = Core::<App>::new().unwrap();
    let mut backend = TestBackend::default();
    let mut app = App { stops: 0 };
    core.start(&mut app, BackendOps::Test(&mut backend));
    let (entered, observed) = mpsc::channel();
    let (release, released) = mpsc::channel();
    core.shell = Some(
        ShellHelper::spawn(core.id, 1, move || {
            move |_: &super::super::PreparedRequest| {
                entered.send(()).unwrap();
                released.recv_timeout(Duration::from_secs(3)).unwrap();
                Ok(())
            }
        })
        .unwrap(),
    );
    let receipt = Context::new(&mut core, BackendOps::Unavailable, ContextPhase::Event)
        .request_shell(request())
        .unwrap();
    assert_eq!(receipt.id().host(), core.id);
    observed.recv_timeout(Duration::from_secs(3)).unwrap();
    core.request_stop(StopReason::Application);
    for _ in 0..3 {
        core.poll_stop(&mut app, BackendOps::Test(&mut backend));
        assert_eq!(core.state, HostState::Stopping);
        assert!(backend.complete_destruction().is_none());
    }
    assert_eq!(app.stops, 1);
    let pending = core.shutdown_snapshot();
    assert_eq!(pending.shell_requests, 1);
    assert!(pending.shell_helper_retiring);
    assert!(pending.application_settled);
    assert_eq!(pending.retained_window_roots, 1);
    let mut cx = Context::new(&mut core, BackendOps::Unavailable, ContextPhase::Retirement);
    assert_eq!(cx.shell_pending(), 1);
    let rejected = cx.request_shell(request()).unwrap_err();
    assert_eq!(rejected.request, request());
    assert_eq!(rejected.reason, ShellFailure::Closed);
    assert_eq!(cx.open_shell(&request()), Err(ShellFailure::Closed));
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while core.state == HostState::Stopping {
        assert!(Instant::now() < deadline);
        core.poll_stop(&mut app, BackendOps::Test(&mut backend));
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(receipt.outcome(), ShellOutcome::Complete(Ok(())));
    assert!(core.shell.is_none());
    assert_eq!(app.stops, 1);
    while let Some(key) = backend.complete_destruction() {
        core.native_destroyed(key);
    }
    assert_eq!(core.state, HostState::Closed);
    drop(core);
    assert_eq!(receipt.outcome(), ShellOutcome::Complete(Ok(())));
}

#[test]
fn invalid_shell_request_does_not_start_a_helper_or_consume_input() {
    let mut core = Core::<App>::new().unwrap();
    let mut cx = Context::new(&mut core, BackendOps::Unavailable, ContextPhase::Startup);
    let invalid = ShellRequest {
        destination: ShellDestination::Url("invalid".into()),
    };
    let rejected = cx.request_shell(invalid.clone()).unwrap_err();
    assert_eq!(rejected.request, invalid);
    assert!(matches!(rejected.reason, ShellFailure::InvalidInput(_)));
    assert!(core.shell.is_none());
}

#[test]
fn zero_shell_requests_do_not_release_window_before_executor_final_drop_and_join() {
    struct DropLatch {
        entered: mpsc::Sender<()>,
        release: mpsc::Receiver<()>,
    }
    impl Drop for DropLatch {
        fn drop(&mut self) {
            self.entered.send(()).unwrap();
            self.release.recv_timeout(Duration::from_secs(3)).unwrap();
        }
    }
    let mut core = Core::<App>::new().unwrap();
    let mut backend = TestBackend::default();
    let mut app = App { stops: 0 };
    core.start(&mut app, BackendOps::Test(&mut backend));
    let (entered, observed) = mpsc::channel();
    let (release, released) = mpsc::channel();
    core.shell = Some(
        ShellHelper::spawn(core.id, 1, move || {
            let final_access = DropLatch {
                entered,
                release: released,
            };
            move |_: &super::super::PreparedRequest| {
                let _retained = &final_access;
                Ok(())
            }
        })
        .unwrap(),
    );
    let receipt = Context::new(&mut core, BackendOps::Unavailable, ContextPhase::Event)
        .request_shell(request())
        .unwrap();
    core.request_stop(StopReason::Application);
    core.poll_stop(&mut app, BackendOps::Test(&mut backend));
    observed.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(receipt.outcome(), ShellOutcome::Complete(Ok(())));
    for _ in 0..3 {
        core.poll_stop(&mut app, BackendOps::Test(&mut backend));
        let pending = core.shutdown_snapshot();
        assert!(pending.application_settled);
        assert_eq!(pending.shell_requests, 0);
        assert!(pending.shell_helper_retiring);
        assert_eq!(pending.retained_window_roots, 1);
        assert_eq!(pending.pending_native_destructions, 0);
        assert_eq!(pending.state, HostState::Stopping);
        assert!(backend.complete_destruction().is_none());
    }
    assert_eq!(app.stops, 1);
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while core.state == HostState::Stopping {
        assert!(Instant::now() < deadline);
        core.poll_stop(&mut app, BackendOps::Test(&mut backend));
        std::thread::sleep(Duration::from_millis(1));
    }
    let pending = core.shutdown_snapshot();
    assert!(!pending.shell_helper_retiring);
    assert_eq!(pending.retained_window_roots, 0);
    assert_eq!(pending.pending_native_destructions, 1);
    while let Some(key) = backend.complete_destruction() {
        core.native_destroyed(key);
    }
    assert_eq!(core.state, HostState::Closed);
    assert_eq!(core.shutdown_snapshot().pending_native_destructions, 0);
    assert_eq!(app.stops, 1);
}
