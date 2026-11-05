use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use engine_cache::{AccountingDomain, AllocationClass, Backing, CleanupContext};

use crate::backend::{
    BackendOps,
    test::{TestBackend, Trace},
};
use crate::context::{Context, ContextPhase};
use crate::error::{Error, NativeOperation};
use crate::host::{Application, Core, HostState, StopOutcome, StopProgress};
use crate::identity::WindowTarget;
use crate::window::{WindowSpec, WindowState};

enum StartAction {
    Stop,
    FailAcquisition,
    Panic,
    PanicPayload(Arc<AtomicBool>),
}

pub(crate) struct App {
    target: Option<WindowTarget>,
    action: StartAction,
    pending: usize,
    polls: usize,
    startup_returned: bool,
    unexpectedly_destroyed: bool,
}

impl App {
    fn new(action: StartAction, pending: usize) -> Self {
        Self {
            target: None,
            action,
            pending,
            polls: 0,
            startup_returned: false,
            unexpectedly_destroyed: false,
        }
    }
}

impl Application for App {
    type Message = String;
    type LocalEvent = u8;
    fn started(&mut self, cx: &mut Context<'_, Self>) -> Result<(), Error> {
        assert_eq!(cx.phase(), ContextPhase::Startup);
        assert!(cx.admission().ordinary_open);
        self.target = Some(cx.create_window(WindowSpec::default()).unwrap());
        match &self.action {
            StartAction::Stop => {
                assert_eq!(cx.request_stop(), StopOutcome::Requested);
                assert_eq!(cx.request_stop(), StopOutcome::AlreadyRequested);
                assert_eq!(
                    cx.window_state(self.target.unwrap()),
                    Ok(WindowState::Retiring)
                );
                let spec = WindowSpec {
                    title: String::from("unaccepted"),
                    ..WindowSpec::default()
                };
                let (returned, reason) = cx.create_window(spec.clone()).unwrap_err().into_parts();
                assert_eq!(returned, spec);
                assert_eq!(reason, Error::AdmissionClosed);
                self.startup_returned = true;
                Ok(())
            }
            StartAction::FailAcquisition => {
                let spec = WindowSpec {
                    title: String::from("fail"),
                    ..WindowSpec::default()
                };
                let (returned, error) = cx.create_window(spec.clone()).unwrap_err().into_parts();
                assert_eq!(returned, spec);
                self.startup_returned = true;
                Err(error)
            }
            StartAction::Panic => panic!("startup panic after native acquisition"),
            StartAction::PanicPayload(dropped) => {
                std::panic::panic_any(DropPanics(Arc::clone(dropped)))
            }
        }
    }

    fn stopping(&mut self, cx: &mut Context<'_, Self>) -> StopProgress {
        if matches!(
            self.action,
            StartAction::Stop | StartAction::FailAcquisition
        ) {
            assert!(self.startup_returned);
        }
        assert_eq!(cx.phase(), ContextPhase::Retirement);
        assert!(!cx.admission().ordinary_open);
        assert!(cx.admission().retirement_open);
        assert_eq!(
            cx.window_state(self.target.unwrap()),
            if self.unexpectedly_destroyed {
                Err(Error::StaleIdentity)
            } else {
                Ok(WindowState::Retiring)
            }
        );
        assert_eq!(
            cx.create_window(WindowSpec::default())
                .unwrap_err()
                .reason(),
            &Error::AdmissionClosed
        );
        self.polls += 1;
        if self.polls <= self.pending {
            StopProgress::Pending
        } else {
            StopProgress::Settled
        }
    }
}

struct DropPanics(Arc<AtomicBool>);
impl Drop for DropPanics {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
        panic!("panic payload destructor");
    }
}

fn complete_native_retirement(core: &mut Core<App>, backend: &mut TestBackend) {
    assert_eq!(core.state, HostState::Retiring);
    let key = backend
        .complete_destruction()
        .expect("pending native destruction");
    core.native_destroyed(key);
    assert_eq!(core.state, HostState::Closed);
}

#[test]
fn stop_latches_without_reentry_and_waits_for_application_and_native_retirement() {
    let mut core = Core::new().unwrap();
    let host = core.id;
    let mut backend = TestBackend::default();
    let mut app = App::new(StartAction::Stop, 2);
    core.start(&mut app, BackendOps::Test(&mut backend));
    assert_eq!(core.id, host);
    assert_eq!(app.polls, 0);
    core.start(&mut app, BackendOps::Test(&mut backend));
    for _ in 0..2 {
        core.poll_stop(&mut app, BackendOps::Test(&mut backend));
        assert_eq!(core.state, HostState::Stopping);
        assert_eq!(
            backend.trace.borrow().as_slice(),
            &[Trace::Created(String::from("engine_host"))]
        );
    }
    core.poll_stop(&mut app, BackendOps::Test(&mut backend));
    assert_eq!(app.polls, 3);
    assert_eq!(
        backend.trace.borrow().last(),
        Some(&Trace::DestroyRequested(String::from("engine_host")))
    );
    assert_eq!(core.state, HostState::Retiring);
    // A request and an extra visit cannot fabricate native destruction.
    core.poll_stop(&mut app, BackendOps::Test(&mut backend));
    assert_eq!(app.polls, 3);
    complete_native_retirement(&mut core, &mut backend);
    assert_eq!(core.id, host);
}

#[test]
fn partial_native_acquisition_failure_keeps_successful_window_until_cleanup_settles() {
    let mut core = Core::new().unwrap();
    let mut backend = TestBackend::default();
    backend.fail_title = Some(String::from("fail"));
    let mut app = App::new(StartAction::FailAcquisition, 1);
    core.start(&mut app, BackendOps::Test(&mut backend));
    assert!(matches!(
        core.failure,
        Some(Error::Native {
            operation: NativeOperation::CreateWindow,
            ..
        })
    ));
    core.poll_stop(&mut app, BackendOps::Test(&mut backend));
    assert_eq!(core.native_windows.len(), 1);
    assert_eq!(core.state, HostState::Stopping);
    core.poll_stop(&mut app, BackendOps::Test(&mut backend));
    complete_native_retirement(&mut core, &mut backend);
    assert!(core.failure.is_some());
}

#[test]
fn startup_panics_retire_acquisitions_even_when_payload_destructor_would_panic() {
    for action in [
        StartAction::Panic,
        StartAction::PanicPayload(Arc::new(AtomicBool::new(false))),
    ] {
        let mut core = Core::new().unwrap();
        let mut backend = TestBackend::default();
        let mut app = App::new(action, 0);
        core.start(&mut app, BackendOps::Test(&mut backend));
        assert!(matches!(
            core.failure,
            Some(Error::ApplicationPanicked {
                phase: ContextPhase::Startup,
                ..
            })
        ));
        core.poll_stop(&mut app, BackendOps::Test(&mut backend));
        complete_native_retirement(&mut core, &mut backend);
        if let StartAction::PanicPayload(dropped) = &app.action {
            assert!(!dropped.load(Ordering::Relaxed));
        }
    }
}

#[test]
fn unexpected_destruction_during_stopping_invalidates_generation_without_second_native_access() {
    let mut core = Core::new().unwrap();
    let mut backend = TestBackend::default();
    let mut app = App::new(StartAction::Stop, 1);
    core.start(&mut app, BackendOps::Test(&mut backend));
    core.native_destroyed(core.native_windows[0].0);
    app.unexpectedly_destroyed = true;
    assert!(core.failure.is_some());
    core.poll_stop(&mut app, BackendOps::Test(&mut backend));
    core.poll_stop(&mut app, BackendOps::Test(&mut backend));
    assert_eq!(core.state, HostState::Closed);
    assert!(backend.complete_destruction().is_none());
    assert_eq!(backend.trace.borrow().len(), 1);
}

#[test]
fn invalid_input_and_invalid_context_return_original_spec_before_acquisition() {
    let mut core = Core::new().unwrap();
    let mut backend = TestBackend::default();
    let mut cx = Context::<App>::new(
        &mut core,
        BackendOps::Test(&mut backend),
        ContextPhase::Startup,
    );
    for spec in [
        WindowSpec {
            title: String::from("bad\0title"),
            ..WindowSpec::default()
        },
        WindowSpec {
            size: crate::PhysicalSize {
                width: 0,
                height: 1,
            },
            ..WindowSpec::default()
        },
        WindowSpec {
            size: crate::PhysicalSize {
                width: u32::MAX,
                height: 1,
            },
            ..WindowSpec::default()
        },
    ] {
        let (returned, reason) = cx.create_window(spec.clone()).unwrap_err().into_parts();
        assert_eq!(returned, spec);
        assert!(matches!(reason, Error::InvalidInput(_)));
    }
    let mut cx = Context::<App>::new(
        &mut core,
        BackendOps::Test(&mut backend),
        ContextPhase::Retirement,
    );
    assert_eq!(
        cx.create_window(WindowSpec::default())
            .unwrap_err()
            .reason(),
        &Error::InvalidContext(ContextPhase::Retirement)
    );
    assert!(backend.trace.borrow().is_empty());
}

#[test]
fn borrowed_non_send_application_drains_actual_externally_scoped_sc_cleanup() {
    struct BorrowedApp<'scope> {
        cleanup: &'scope CleanupContext<Rc<Cell<usize>>>,
        backing: Option<Backing<'scope, Rc<Cell<usize>>>>,
        local: Rc<Cell<usize>>,
    }
    impl Application for BorrowedApp<'_> {
        type Message = String;
        type LocalEvent = u8;
        fn started(&mut self, cx: &mut Context<'_, Self>) -> Result<(), Error> {
            self.local.set(1);
            drop(self.backing.take());
            cx.request_stop();
            Err(Error::application("partial domain startup"))
        }
        fn stopping(&mut self, _: &mut Context<'_, Self>) -> StopProgress {
            assert_eq!(self.cleanup.drain_budget(1), 1);
            self.local.set(2);
            StopProgress::Settled
        }
    }
    let cleanup = CleanupContext::new();
    let local = Rc::new(Cell::new(0));
    let domain = AccountingDomain::new();
    let backing = cleanup
        .try_acquire(Rc::clone(&local), &domain, 1, AllocationClass::Resident)
        .unwrap();
    let mut app = BorrowedApp {
        cleanup: &cleanup,
        backing: Some(backing),
        local: Rc::clone(&local),
    };
    let mut core = Core::new().unwrap();
    let mut backend = TestBackend::default();
    core.start(&mut app, BackendOps::Test(&mut backend));
    assert_eq!(cleanup.pending(), 1);
    core.poll_stop(&mut app, BackendOps::Test(&mut backend));
    assert_eq!(core.state, HostState::Closed);
    assert_eq!(cleanup.pending(), 0);
    assert_eq!(local.get(), 2);
    assert_eq!(domain.snapshot().total_declared_bytes, 0);
}
