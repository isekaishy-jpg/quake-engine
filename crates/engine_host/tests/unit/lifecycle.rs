//! Application-managed representation simulation against actual `engine_host` contracts.
//! Stock/modern/HD labels do not implement a game UI or asset subsystem. Native
//! display state is simulated by TestBackend; renderer reports are fixture input.

use std::sync::{Arc, Mutex};
use std::thread::{self, ThreadId};

use engine_worker::{Runtime, RuntimeConfig, RuntimeState, WorkerConfig};

use crate::backend::{BackendOps, test::TestBackend, winit_input::NormalizedInput};
use crate::host::Core;
use crate::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Representation {
    Stock,
    ModernUi,
    Hd,
}

struct Message {
    representation: Representation,
    value: u8,
    drops: Arc<Mutex<Vec<(u8, ThreadId)>>>,
}

impl Drop for Message {
    fn drop(&mut self) {
        self.drops
            .lock()
            .unwrap()
            .push((self.value, thread::current().id()));
    }
}

struct App {
    representation: Representation,
    // An actual runtime instance, with all worker classes disabled. Direct `engine_worker`
    // work/`engine_cache` cleanup integration is qualified by the independent example.
    runtime: Box<Runtime>,
    posts: Vec<(Representation, u8)>,
    text: Vec<(TextSessionId, String)>,
    display: Vec<DisplayTransition>,
    stopping_calls: usize,
}

impl App {
    fn new() -> Self {
        let config = RuntimeConfig::new(0, [WorkerConfig::new(0); 3]).unwrap();
        Self {
            representation: Representation::Stock,
            runtime: Box::new(Runtime::builder(config).build().unwrap()),
            posts: Vec::new(),
            text: Vec::new(),
            display: Vec::new(),
            stopping_calls: 0,
        }
    }

    fn runtime_address(&self) -> usize {
        std::ptr::from_ref(self.runtime.as_ref()) as usize
    }
}

impl Application for App {
    type Message = Message;
    type LocalEvent = ();

    fn started(&mut self, _: &mut Context<'_, Self>) -> Result<(), Error> {
        Ok(())
    }

    fn stopping(&mut self, _: &mut Context<'_, Self>) -> StopProgress {
        self.stopping_calls += 1;
        StopProgress::Settled
    }

    fn display_transition(&mut self, cx: &mut Context<'_, Self>, event: &DisplayTransition) {
        assert_eq!(cx.phase(), ContextPhase::Display);
        self.display.push(event.clone());
    }
}

fn handler(app: &mut App, _: &mut Context<'_, App>, event: &Event<'_, Message, ()>) -> Propagation {
    match event {
        Event::Posted { message, .. } => {
            assert_eq!(
                message.representation, app.representation,
                "old representation post reached replacement"
            );
            app.posts.push((message.representation, message.value));
        }
        Event::Input(record) => {
            if let InputEvent::Text { session, text } = &record.event {
                app.text.push((*session, text.clone()));
            }
        }
        _ => (),
    }
    Propagation::Continue
}

fn recipient(cx: &mut Context<'_, App>) -> RecipientId {
    let recipient = cx.create_recipient().unwrap();
    cx.subscribe(recipient, EventFilter::All, Priority::default(), handler)
        .unwrap();
    recipient
}

fn caret() -> TextCaret {
    TextCaret {
        position: PhysicalPosition { x: 1.0, y: 2.0 },
        size: PhysicalSize {
            width: 1,
            height: 16,
        },
    }
}

fn text_record(target: WindowTarget, session: TextSessionId, text: &str) -> NormalizedInput {
    NormalizedInput {
        target,
        event: InputEvent::Text {
            session,
            text: String::from(text),
        },
        origin: InputOrigin::NativeWindow,
        source_time: None,
        device: None,
    }
}

#[test]
fn simulated_stock_modern_hd_reversals_preserve_host_and_real_runtime_and_reject_old_work() {
    let mut core = Core::<App>::new().unwrap();
    let mut backend = TestBackend::default();
    let mut app = App::new();
    let host = core.id;
    let runtime = app.runtime_address();
    let owner = thread::current().id();
    let drops = Arc::new(Mutex::new(Vec::new()));
    let mut cx = Context::new(
        &mut core,
        BackendOps::Test(&mut backend),
        ContextPhase::Startup,
    );
    let target = cx.create_window(WindowSpec::default()).unwrap();
    let mut current_recipient = recipient(&mut cx);
    let mut session = cx.begin_text(target, caret()).unwrap();

    for (index, replacement) in [
        Representation::ModernUi,
        Representation::Stock,
        Representation::Hd,
        Representation::Stock,
    ]
    .into_iter()
    .enumerate()
    {
        let value = u8::try_from(index * 2).unwrap();
        let proxy = cx.proxy();
        let old_recipient = current_recipient;
        let old_session = session;
        let previous_representation = app.representation;
        let payload_drops = Arc::clone(&drops);
        let old_post = thread::spawn(move || {
            proxy
                .try_post(
                    old_recipient,
                    Message {
                        representation: previous_representation,
                        value,
                        drops: payload_drops,
                    },
                )
                .unwrap()
        })
        .join()
        .unwrap();
        (current_recipient, session) = cx.with_input_deferred(&mut app, |app, cx| {
            cx.receive_input_batch(app, vec![text_record(target, old_session, "old queued")])
                .unwrap();
            assert_eq!(
                cx.retire_recipient(old_recipient).unwrap().active_callbacks,
                0
            );
            app.representation = replacement;
            let next_recipient = recipient(cx);
            let next_session = cx.begin_text(target, caret()).unwrap();
            assert_ne!(next_recipient, old_recipient);
            assert_ne!(next_session, old_session);
            cx.receive_input_batch(
                app,
                vec![
                    text_record(target, old_session, "old late intake"),
                    text_record(target, next_session, "fresh é"),
                ],
            )
            .unwrap();
            (next_recipient, next_session)
        });
        let rejected = cx
            .proxy()
            .try_post(
                old_recipient,
                Message {
                    representation: previous_representation,
                    value: 99,
                    drops: Arc::clone(&drops),
                },
            )
            .unwrap_err();
        assert_eq!(rejected.reason(), &Error::StaleIdentity);
        drop(rejected);
        let fresh_post = cx
            .proxy()
            .try_post(
                current_recipient,
                Message {
                    representation: replacement,
                    value: value + 1,
                    drops: Arc::clone(&drops),
                },
            )
            .unwrap();
        assert_eq!(cx.drain_posts(&mut app, 4).unwrap(), 2);
        assert_eq!(
            old_post.outcome(),
            PostOutcome::Discarded(PostDiscardReason::RecipientRetired)
        );
        assert_eq!(
            fresh_post.outcome(),
            PostOutcome::Delivered { callbacks: 1 }
        );
        assert_eq!(cx.drain_input(&mut app, 4).unwrap().consumed, 2);
        assert_eq!(app.text.last(), Some(&(session, String::from("fresh é"))));
        assert_eq!(app.text.len(), index + 1);
        assert_eq!(app.posts.len(), index + 1);
        assert_eq!(
            cx.update_text(old_session, caret()),
            Err(Error::StaleIdentity)
        );
        assert_eq!(cx.host_id(), host);
        assert_eq!(cx.window_state(target), Ok(WindowState::Live));
        assert_eq!(app.runtime_address(), runtime);
        assert_eq!(app.runtime.state(), RuntimeState::Running);
        assert_eq!(app.stopping_calls, 0);
    }
    assert!(
        drops
            .lock()
            .unwrap()
            .iter()
            .all(|(_, thread)| *thread == owner)
    );
    assert_eq!(cx.host_id(), target.id.host());
    app.runtime.shutdown().unwrap();
}

#[test]
fn settled_application_cannot_force_native_retirement_while_renderer_lease_remains() {
    use crate::backend::test::Trace;
    use std::time::Duration;

    let mut core = Core::<App>::new().unwrap();
    let mut backend = TestBackend::default();
    let trace = backend.trace.clone();
    let mut app = App::new();
    let host = core.id;
    let runtime = app.runtime_address();
    core.start(&mut app, BackendOps::Test(&mut backend));
    let target;
    let lease;
    {
        let mut cx = Context::new(
            &mut core,
            BackendOps::Test(&mut backend),
            ContextPhase::Event,
        );
        target = cx
            .create_window(WindowSpec {
                title: String::from("renderer-retained"),
                ..WindowSpec::default()
            })
            .unwrap();
        lease = cx.acquire_window(target).unwrap();
        assert_eq!(lease.target(), target);
        cx.request_stop();
        assert_eq!(cx.window_state(target), Ok(WindowState::Retiring));
        assert!(matches!(
            cx.acquire_window(target),
            Err(Error::AdmissionClosed)
        ));
    }

    // Application fault injection: Settled was reported before the fixture
    // renderer released its lease. `engine_host` must still enforce retained native roots.
    // Manual raw time is deliberately far beyond ordinary fallback deadlines.
    // CPU/application settlement and time advancement are not renderer retirement.
    for visit in 0..16 {
        core.clock.manual = Some(Duration::from_secs(1_000_000 + visit));
        core.pump_retirement(&mut app, BackendOps::Test(&mut backend));
        core.poll_stop(&mut app, BackendOps::Test(&mut backend));
        core.reap_windows();
        assert_ne!(core.state, HostState::Closed);
        assert!(lease.is_native_alive());
        assert!(
            !trace
                .borrow()
                .iter()
                .any(|event| matches!(event, Trace::DestroyRequested(_)))
        );
        assert_eq!(core.id, host);
        assert_eq!(app.runtime_address(), runtime);
        assert_eq!(app.runtime.state(), RuntimeState::Running);
    }
    assert_eq!(app.stopping_calls, 1);
    thread::spawn(move || drop(lease)).join().unwrap();
    core.pump_retirement(&mut app, BackendOps::Test(&mut backend));
    core.poll_stop(&mut app, BackendOps::Test(&mut backend));
    core.reap_windows();
    assert!(trace.borrow().iter().any(
        |event| matches!(event, Trace::DestroyRequested(title) if title == "renderer-retained")
    ));
    assert_eq!(core.state, HostState::Retiring);
    assert_eq!(core.native_windows.len(), 1);
    let destroyed = backend.complete_destruction().unwrap();
    core.native_destroyed(destroyed);
    assert_eq!(core.state, HostState::Closed);
    assert!(core.native_windows.is_empty());
    assert_eq!(app.stopping_calls, 1);
    assert_eq!(app.runtime_address(), runtime);
    assert_eq!(app.runtime.state(), RuntimeState::Running);
    app.runtime.shutdown().unwrap();
}

#[test]
fn unexpected_destroyed_during_host_retirement_invalidates_renderer_held_generation() {
    let mut core = Core::<App>::new().unwrap();
    let mut backend = TestBackend::default();
    let mut app = App::new();
    core.start(&mut app, BackendOps::Test(&mut backend));
    let target;
    let lease;
    {
        let mut cx = Context::new(
            &mut core,
            BackendOps::Test(&mut backend),
            ContextPhase::Event,
        );
        target = cx.create_window(WindowSpec::default()).unwrap();
        lease = cx.acquire_window(target).unwrap();
        cx.request_stop();
    }
    core.poll_stop(&mut app, BackendOps::Test(&mut backend));
    assert_eq!(core.state, HostState::Retiring);
    assert!(lease.is_native_alive());
    let record = core.windows.get(target.id).unwrap();
    let key = record.native.key();
    if let backend::NativeWindow::Test(window) = &record.native {
        // This is a simulated unexpected native destruction, not a normal Drop request.
        window.native_alive.set(false);
    }
    core.native_destroyed(key);
    assert!(!lease.is_native_alive());
    assert!(matches!(lease.native_ref(), Err(Error::StaleIdentity)));
    assert!(core.windows.get(target.id).is_err());
    assert!(core.failure.is_some());
    thread::spawn(move || drop(lease)).join().unwrap();
    assert!(
        backend.complete_destruction().is_none(),
        "stale generation reposted native destruction"
    );
    app.runtime.shutdown().unwrap();
}

#[test]
fn simulated_partial_display_and_acquisition_failures_do_not_orphan_old_access_or_retarget_work() {
    let mut core = Core::<App>::new().unwrap();
    let mut backend = TestBackend::default();
    backend.fail_title = Some(String::from("rejected replacement"));
    let mut app = App::new();
    let host = core.id;
    let runtime = app.runtime_address();
    core.start(&mut app, BackendOps::Test(&mut backend));
    let old;
    let old_access;
    let old_session;
    let old_recipient;
    let failed_display;
    {
        let mut cx = Context::new(
            &mut core,
            BackendOps::Test(&mut backend),
            ContextPhase::Event,
        );
        old = cx
            .create_window(WindowSpec {
                title: String::from("old representation"),
                ..WindowSpec::default()
            })
            .unwrap();
        old_access = cx.acquire_window(old).unwrap();
        old_session = cx.begin_text(old, caret()).unwrap();
        old_recipient = recipient(&mut cx);
        let monitors = cx.monitors(old).unwrap();
        failed_display = cx
            .request_display(
                old,
                DisplayRequest::Borderless {
                    monitor: monitors[0].id.clone(),
                },
            )
            .unwrap();
    }
    if let backend::NativeWindow::Test(window) = &core.windows.get(old.id).unwrap().native {
        window.fail_display.set(true);
    }
    core.pump_display(&mut app, BackendOps::Test(&mut backend));
    let old_revert;
    {
        let mut cx = Context::new(
            &mut core,
            BackendOps::Test(&mut backend),
            ContextPhase::Event,
        );
        // The fake driver mutates mode and then fails: observed state stays
        // truthful even though the operation never became presentation-ready.
        assert_eq!(
            cx.display_observed(old).unwrap().backend_mode,
            DisplayMode::Borderless
        );
        assert!(matches!(
            cx.display_transition(failed_display).unwrap().state,
            DisplayTransitionState::Failed(Error::Native {
                operation: NativeOperation::DisplayTransition,
                ..
            })
        ));
        assert!(old_access.is_native_alive());
        assert_eq!(cx.window_state(old), Ok(WindowState::Live));
        let spec = WindowSpec {
            title: String::from("rejected replacement"),
            ..WindowSpec::default()
        };
        let (returned, failure) = cx.create_window(spec.clone()).unwrap_err().into_parts();
        assert_eq!(returned, spec);
        assert!(matches!(
            failure,
            Error::Native {
                operation: NativeOperation::CreateWindow,
                ..
            }
        ));
        assert_eq!(cx.window_state(old), Ok(WindowState::Live));
        cx.update_text(old_session, caret()).unwrap();
        old_revert = cx
            .request_display(old, DisplayRequest::Windowed { placement: None })
            .unwrap();
    }
    if let backend::NativeWindow::Test(window) = &core.windows.get(old.id).unwrap().native {
        window.fail_display.set(false);
    }
    core.pump_display(&mut app, BackendOps::Test(&mut backend));
    let drops = Arc::new(Mutex::new(Vec::new()));
    let new;
    let fresh_display;
    {
        let mut cx = Context::new(
            &mut core,
            BackendOps::Test(&mut backend),
            ContextPhase::Event,
        );
        let old_post = cx
            .proxy()
            .try_post(
                old_recipient,
                Message {
                    representation: Representation::Stock,
                    value: 1,
                    drops: Arc::clone(&drops),
                },
            )
            .unwrap();
        (new, fresh_display) = cx.with_input_deferred(&mut app, |app, cx| {
            cx.receive_input_batch(
                app,
                vec![text_record(
                    old,
                    old_session,
                    "old native generation queued",
                )],
            )
            .unwrap();
            let new = cx
                .create_window(WindowSpec {
                    title: String::from("new representation"),
                    ..WindowSpec::default()
                })
                .unwrap();
            assert_ne!(new.id, old.id);
            let fresh_recipient = recipient(cx);
            let fresh_session = cx.begin_text(new, caret()).unwrap();
            cx.retire_recipient(old_recipient).unwrap();
            cx.request_close(old).unwrap();
            app.representation = Representation::ModernUi;
            assert!(matches!(
                cx.acquire_window(old),
                Err(Error::AdmissionClosed)
            ));
            assert_eq!(cx.end_text(old_session), Err(Error::StaleIdentity));
            cx.receive_input_batch(app, vec![text_record(new, fresh_session, "fresh target é")])
                .unwrap();
            cx.proxy()
                .try_post(
                    fresh_recipient,
                    Message {
                        representation: Representation::ModernUi,
                        value: 2,
                        drops: Arc::clone(&drops),
                    },
                )
                .unwrap();
            let receipt = cx
                .request_display(new, DisplayRequest::Windowed { placement: None })
                .unwrap();
            (new, receipt)
        });
        assert_eq!(cx.drain_posts(&mut app, 4).unwrap(), 2);
        assert_eq!(
            old_post.outcome(),
            PostOutcome::Discarded(PostDiscardReason::RecipientRetired)
        );
        assert_eq!(cx.drain_input(&mut app, 4).unwrap().consumed, 1);
        assert_eq!(app.posts, [(Representation::ModernUi, 2)]);
        assert_eq!(app.text.len(), 1);
        assert_eq!(app.text[0].1, "fresh target é");
    }
    core.pump_display(&mut app, BackendOps::Test(&mut backend));
    {
        let mut cx = Context::new(
            &mut core,
            BackendOps::Test(&mut backend),
            ContextPhase::Event,
        );
        assert_eq!(
            cx.report_presentation(old_revert, PresentationStatus::Ready),
            Err(Error::StaleIdentity)
        );
        assert_eq!(
            cx.report_presentation(failed_display, PresentationStatus::Ready),
            Err(Error::StaleIdentity)
        );
        assert_eq!(
            cx.display_transition(fresh_display).unwrap().state,
            DisplayTransitionState::AwaitingPresentation
        );
        cx.report_presentation(fresh_display, PresentationStatus::Ready)
            .unwrap();
        assert_eq!(cx.window_state(old), Ok(WindowState::Retiring));
        assert_eq!(cx.window_state(new), Ok(WindowState::Live));
        assert_eq!(cx.host_id(), host);
    }
    core.reap_closing_windows(BackendOps::Test(&mut backend));
    assert!(backend.complete_destruction().is_none());
    assert!(old_access.is_native_alive());
    thread::spawn(move || drop(old_access)).join().unwrap();
    core.reap_closing_windows(BackendOps::Test(&mut backend));
    {
        let cx = Context::new(
            &mut core,
            BackendOps::Test(&mut backend),
            ContextPhase::Event,
        );
        assert_eq!(cx.window_state(old), Ok(WindowState::Retiring));
    }
    core.native_destroyed(backend.complete_destruction().unwrap());
    {
        let mut cx = Context::new(
            &mut core,
            BackendOps::Test(&mut backend),
            ContextPhase::Event,
        );
        assert_eq!(cx.window_state(old), Err(Error::StaleIdentity));
        assert_eq!(cx.window_state(new), Ok(WindowState::Live));
        assert_eq!(
            cx.report_presentation(old_revert, PresentationStatus::Ready),
            Err(Error::StaleIdentity)
        );
        let request = DisplayRequest::Windowed { placement: None };
        let (returned, failure) = cx
            .request_display(old, request.clone())
            .unwrap_err()
            .into_parts();
        assert_eq!(returned, request);
        assert_eq!(failure, Error::StaleIdentity);
    }
    assert_eq!(core.state, HostState::Running);
    assert!(core.failure.is_none());
    assert_eq!(core.id, host);
    assert_eq!(app.runtime_address(), runtime);
    assert_eq!(app.runtime.state(), RuntimeState::Running);
    assert_eq!(app.stopping_calls, 0);
    app.runtime.shutdown().unwrap();
}
