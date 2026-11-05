use crate::backend::{BackendOps, test::TestBackend, winit_input::NormalizedInput};
use crate::host::Core;
use crate::*;

#[derive(Default)]
struct App {
    stops: usize,
    end_on_key: Option<TextSessionId>,
    events: Vec<InputEvent>,
    platform_focus: Vec<bool>,
}
impl Application for App {
    type Message = ();
    type LocalEvent = ();
    fn started(&mut self, _: &mut Context<'_, Self>) -> Result<(), Error> {
        Ok(())
    }
    fn stopping(&mut self, _: &mut Context<'_, Self>) -> StopProgress {
        self.stops += 1;
        StopProgress::Settled
    }
}
fn handler(app: &mut App, cx: &mut Context<'_, App>, event: &Event<'_, (), ()>) -> Propagation {
    if let Event::Input(record) = event {
        if let InputEvent::Wheel {
            position, scale, ..
        } = record.event
        {
            let delivered = cx
                .input_state(record.target, InputStateLayer::Delivered)
                .unwrap()
                .unwrap();
            assert_eq!(delivered.position, position);
            assert_eq!(delivered.scale, Some(scale));
        }
        app.platform_focus.push(
            cx.input_state(record.target, InputStateLayer::Platform)
                .unwrap()
                .unwrap()
                .focused,
        );
        if matches!(record.event, InputEvent::Key { .. })
            && let Some(session) = app.end_on_key.take()
        {
            cx.end_text(session).unwrap();
        }
        app.events.push(record.event.clone());
    }
    Propagation::Continue
}
fn caret() -> TextCaret {
    TextCaret {
        position: PhysicalPosition { x: 2.25, y: 3.0 },
        size: PhysicalSize {
            width: 1,
            height: 12,
        },
    }
}
fn setup(cx: &mut Context<'_, App>) -> WindowTarget {
    let target = cx.create_window(WindowSpec::default()).unwrap();
    let recipient = cx.create_recipient().unwrap();
    cx.subscribe(recipient, EventFilter::Input, Priority::default(), handler)
        .unwrap();
    target
}
fn record(target: WindowTarget, event: InputEvent) -> NormalizedInput {
    NormalizedInput {
        target,
        event,
        origin: InputOrigin::NativeWindow,
        source_time: None,
        device: None,
    }
}

#[test]
fn complete_batch_platform_state_precedes_callbacks_and_session_end_discards_later_text() {
    let mut core = Core::<App>::new().unwrap();
    let mut backend = TestBackend::default();
    let mut app = App::default();
    let mut cx = Context::new(
        &mut core,
        BackendOps::Test(&mut backend),
        ContextPhase::Startup,
    );
    let target = setup(&mut cx);
    let session = cx.begin_text(target, caret()).unwrap();
    app.end_on_key = Some(session);
    cx.receive_input_batch(
        &mut app,
        vec![
            record(
                target,
                InputEvent::Key {
                    physical: PhysicalKey::ScanCode(30),
                    logical: LogicalKey::Character(String::from("a")),
                    location: KeyLocation::Standard,
                    state: ButtonState::Pressed,
                    repeat: false,
                    modifiers: Modifiers::default(),
                    synthetic: false,
                },
            ),
            record(
                target,
                InputEvent::Text {
                    session,
                    text: String::from("a"),
                },
            ),
            record(target, InputEvent::Focus(false)),
        ],
    )
    .unwrap();
    assert_eq!(app.events.len(), 2);
    assert_eq!(app.platform_focus, [false, false]);
    assert!(matches!(app.events[0], InputEvent::Key { .. }));
    assert_eq!(app.events[1], InputEvent::Focus(false));
    assert!(cx.admission().ordinary_open);
}

#[test]
fn ended_and_replaced_sessions_discard_at_intake_and_deferred_delivery() {
    let mut core = Core::<App>::new().unwrap();
    let mut backend = TestBackend::default();
    let mut app = App::default();
    let mut cx = Context::new(
        &mut core,
        BackendOps::Test(&mut backend),
        ContextPhase::Startup,
    );
    let target = setup(&mut cx);
    let old = cx.begin_text(target, caret()).unwrap();
    cx.with_input_deferred(&mut app, |app, cx| {
        cx.receive_input_batch(
            app,
            vec![record(
                target,
                InputEvent::Text {
                    session: old,
                    text: String::from("old queued"),
                },
            )],
        )
        .unwrap();
        let new = cx.begin_text(target, caret()).unwrap();
        cx.receive_input_batch(
            app,
            vec![
                record(
                    target,
                    InputEvent::Text {
                        session: old,
                        text: String::from("old received"),
                    },
                ),
                record(
                    target,
                    InputEvent::Text {
                        session: new,
                        text: String::from("new"),
                    },
                ),
            ],
        )
        .unwrap();
    });
    assert_eq!(cx.drain_input(&mut app, 10).unwrap().consumed, 2);
    assert_eq!(app.events.len(), 1);
    assert!(matches!(&app.events[0], InputEvent::Text { text, .. } if text == "new"));
    assert!(cx.admission().ordinary_open);
}

#[test]
fn native_partial_mode_failures_keep_request_and_last_confirmations_separate() {
    for (fail_raw, fail_confine) in [(true, false), (false, true)] {
        let mut core = Core::<App>::new().unwrap();
        let mut backend = TestBackend::default();
        backend.fail_raw = fail_raw;
        backend.fail_confine = fail_confine;
        let mut cx = Context::new(
            &mut core,
            BackendOps::Test(&mut backend),
            ContextPhase::Startup,
        );
        let target = setup(&mut cx);
        let request = InputMode {
            relative_motion: true,
            confine_pointer: true,
        };
        assert!(cx.request_input_mode(target, request).is_err());
        let state = cx.input_mode(target).unwrap();
        assert_eq!(state.requested, request);
        assert_eq!(state.confirmed.relative_motion, !fail_raw);
        assert_eq!(state.confirmed.confine_pointer, !fail_confine);
        assert!(state.failure.is_some());
        assert_eq!(
            cx.raw_input_state().confirmed,
            if fail_raw {
                None
            } else {
                Some(RawInputPolicy::WhenFocused)
            }
        );
        assert!(cx.admission().ordinary_open);
    }
}

#[test]
fn raw_recipient_transfer_and_cursor_visibility_are_independent_of_shape_and_confinement() {
    let mut core = Core::<App>::new().unwrap();
    let mut backend = TestBackend::default();
    let mut cx = Context::new(
        &mut core,
        BackendOps::Test(&mut backend),
        ContextPhase::Startup,
    );
    let first = setup(&mut cx);
    let second = cx.create_window(WindowSpec::default()).unwrap();
    cx.select_cursor(first, CursorSelection::System(SystemCursor::Text))
        .unwrap();
    cx.request_cursor_visibility(first, false).unwrap();
    cx.request_input_mode(
        first,
        InputMode {
            relative_motion: true,
            confine_pointer: true,
        },
    )
    .unwrap();
    cx.request_input_mode(
        second,
        InputMode {
            relative_motion: true,
            confine_pointer: false,
        },
    )
    .unwrap();
    assert!(!cx.input_mode(first).unwrap().confirmed.relative_motion);
    assert!(cx.input_mode(first).unwrap().confirmed.confine_pointer);
    assert!(!cx.cursor_state(first).unwrap().requested_visible);
    assert!(!cx.cursor_state(first).unwrap().input_suppressed);
    assert!(matches!(
        cx.cursor_state(first).unwrap().selection,
        CursorSelection::System(SystemCursor::Text)
    ));
    cx.request_input_mode(second, InputMode::default()).unwrap();
    assert_eq!(cx.raw_input_state().confirmed, Some(RawInputPolicy::Never));
    assert_eq!(cx.core.relative_target, None);
}

#[test]
fn cursor_input_validation_differs_from_native_failure_and_keeps_previous_selection() {
    let mut core = Core::<App>::new().unwrap();
    let mut backend = TestBackend::default();
    let mut cx = Context::new(
        &mut core,
        BackendOps::Test(&mut backend),
        ContextPhase::Startup,
    );
    let target = setup(&mut cx);
    cx.select_cursor(target, CursorSelection::System(SystemCursor::Hand))
        .unwrap();
    for width in [0, 1] {
        let input = PreparedCursor {
            rgba: vec![1, 2, 3, 4],
            width,
            height: 1,
            hotspot_x: 0,
            hotspot_y: 0,
        };
        let (returned, error) = cx.create_cursor(input.clone()).unwrap_err().into_parts();
        assert_eq!(returned, input);
        if width == 0 {
            assert!(matches!(error, Error::InvalidInput(_)));
        } else {
            assert!(matches!(
                error,
                Error::Native {
                    operation: NativeOperation::CreateCursor,
                    ..
                }
            ));
        }
        assert!(matches!(
            cx.cursor_state(target).unwrap().selection,
            CursorSelection::System(SystemCursor::Hand)
        ));
    }
}

#[test]
fn deferred_platform_focus_controls_cursor_suppression_before_delivered_state_changes() {
    let mut core = Core::<App>::new().unwrap();
    let mut backend = TestBackend::default();
    let mut app = App::default();
    let mut cx = Context::new(
        &mut core,
        BackendOps::Test(&mut backend),
        ContextPhase::Startup,
    );
    let target = setup(&mut cx);
    cx.request_input_mode(
        target,
        InputMode {
            relative_motion: true,
            confine_pointer: false,
        },
    )
    .unwrap();
    assert!(!cx.cursor_state(target).unwrap().input_suppressed);
    cx.with_input_deferred(&mut app, |app, cx| {
        cx.receive_input_batch(app, vec![record(target, InputEvent::Focus(true))])
            .unwrap();
        assert!(cx.cursor_state(target).unwrap().input_suppressed);
        assert!(
            cx.input_state(target, InputStateLayer::Delivered)
                .unwrap()
                .is_none()
        );
        cx.receive_input_batch(app, vec![record(target, InputEvent::Focus(false))])
            .unwrap();
        assert!(!cx.cursor_state(target).unwrap().input_suppressed);
        cx.receive_input_batch(app, vec![record(target, InputEvent::Focus(true))])
            .unwrap();
        assert!(cx.cursor_state(target).unwrap().input_suppressed);
        cx.receive_input_batch(app, vec![record(target, InputEvent::CaptureLost)])
            .unwrap();
        assert!(cx.cursor_state(target).unwrap().input_suppressed);
        assert!(app.events.is_empty());
    });
    cx.request_stop();
    assert!(!cx.cursor_state(target).unwrap().input_suppressed);
    assert!(cx.cursor_state(target).unwrap().requested_visible);
}

#[test]
fn unexpected_destroyed_suppressed_target_is_invalidated_before_stop_visibility_refresh() {
    let mut core = Core::<App>::new().unwrap();
    let mut backend = TestBackend::default();
    let mut app = App::default();
    let target;
    {
        let mut cx = Context::new(
            &mut core,
            BackendOps::Test(&mut backend),
            ContextPhase::Startup,
        );
        target = setup(&mut cx);
        cx.receive_input_batch(&mut app, vec![record(target, InputEvent::Focus(true))])
            .unwrap();
        cx.request_input_mode(
            target,
            InputMode {
                relative_motion: true,
                confine_pointer: false,
            },
        )
        .unwrap();
        assert!(cx.cursor_state(target).unwrap().input_suppressed);
    }
    core.state = HostState::Running;
    let native = &core.windows.get(target.id).unwrap().native;
    let key = native.key();
    if let crate::backend::NativeWindow::Test(window) = native {
        window.native_alive.set(false);
    }
    let before = backend.trace.borrow().len();
    core.native_destroyed(key);
    assert_eq!(backend.trace.borrow().len(), before);
    assert!(core.windows.get(target.id).is_err());
    assert_eq!(core.state, HostState::Stopping);
    assert!(core.failure.is_some());
    assert_eq!(core.relative_target, None);
}

#[test]
fn stop_native_release_retries_preserve_requests_and_do_not_repeat_settled_application_cleanup() {
    let mut core = Core::<App>::new().unwrap();
    let mut backend = TestBackend::default();
    let mut app = App::default();
    let target;
    {
        let mut cx = Context::new(
            &mut core,
            BackendOps::Test(&mut backend),
            ContextPhase::Startup,
        );
        target = setup(&mut cx);
        cx.request_input_mode(
            target,
            InputMode {
                relative_motion: true,
                confine_pointer: true,
            },
        )
        .unwrap();
        cx.request_stop();
    }
    backend.fail_raw = true;
    core.poll_stop(&mut app, BackendOps::Test(&mut backend));
    assert_eq!(app.stops, 1);
    assert_eq!(core.state, HostState::Stopping);
    assert_eq!(core.raw_input.requested, RawInputPolicy::WhenFocused);
    assert_eq!(core.raw_input.confirmed, Some(RawInputPolicy::WhenFocused));
    assert!(matches!(
        core.failure,
        Some(Error::Native {
            operation: NativeOperation::RegisterRawInput,
            ..
        })
    ));
    let mode = &core.windows.get(target.id).unwrap().input_mode;
    assert!(mode.requested.relative_motion && mode.requested.confine_pointer);
    assert!(mode.confirmed.relative_motion);
    assert!(!mode.confirmed.confine_pointer);
    backend.fail_raw = false;
    core.poll_stop(&mut app, BackendOps::Test(&mut backend));
    assert_eq!(app.stops, 1);
    assert_eq!(core.state, HostState::Retiring);
    assert_eq!(core.raw_input.confirmed, Some(RawInputPolicy::Never));
    core.native_destroyed(backend.complete_destruction().unwrap());
    assert_eq!(core.state, HostState::Closed);
}

#[test]
fn failed_registration_after_confirmed_never_requires_fresh_stop_removal() {
    let mut core = Core::<App>::new().unwrap();
    let mut backend = TestBackend::default();
    let mut app = App::default();
    let target;
    {
        let mut cx = Context::new(
            &mut core,
            BackendOps::Test(&mut backend),
            ContextPhase::Startup,
        );
        target = setup(&mut cx);
        cx.request_input_mode(target, InputMode::default()).unwrap();
    }
    backend.fail_raw = true;
    {
        let mut cx = Context::new(
            &mut core,
            BackendOps::Test(&mut backend),
            ContextPhase::Event,
        );
        assert!(
            cx.request_input_mode(
                target,
                InputMode {
                    relative_motion: true,
                    confine_pointer: false
                }
            )
            .is_err()
        );
        assert_eq!(cx.raw_input_state().confirmed, Some(RawInputPolicy::Never));
        cx.request_stop();
    }
    core.poll_stop(&mut app, BackendOps::Test(&mut backend));
    assert_eq!(core.state, HostState::Stopping);
    assert!(core.raw_input.failure.is_some());
    assert_eq!(app.stops, 1);
    backend.fail_raw = false;
    core.poll_stop(&mut app, BackendOps::Test(&mut backend));
    assert_eq!(core.state, HostState::Retiring);
    assert!(core.raw_input.failure.is_none());
    assert_eq!(app.stops, 1);
}

#[test]
fn native_dead_key_latin_commit_and_fractional_wheel_reversal_survive_deferred_delivery() {
    use winit::event::{DeviceId, ElementState, MouseScrollDelta, TouchPhase, WindowEvent};
    use winit::keyboard::{Key, KeyCode, KeyLocation, PhysicalKey};
    let mut core = Core::<App>::new().unwrap();
    let mut backend = TestBackend::default();
    let mut app = App::default();
    let mut cx = Context::new(
        &mut core,
        BackendOps::Test(&mut backend),
        ContextPhase::Startup,
    );
    let target = setup(&mut cx);
    let session = cx.begin_text(target, caret()).unwrap();
    cx.with_input_deferred(&mut app, |app, cx| {
        for (physical, logical, state, committed) in [
            (
                PhysicalKey::Code(KeyCode::Quote),
                Key::Dead(Some('´')),
                ElementState::Pressed,
                None,
            ),
            (
                PhysicalKey::Code(KeyCode::Quote),
                Key::Dead(Some('´')),
                ElementState::Released,
                None,
            ),
            (
                PhysicalKey::Code(KeyCode::KeyE),
                Key::Character("e".into()),
                ElementState::Pressed,
                Some("é"),
            ),
            (
                PhysicalKey::Code(KeyCode::KeyE),
                Key::Character("e".into()),
                ElementState::Released,
                Some("é"),
            ),
        ] {
            let receipt = cx
                .core
                .input
                .state(target, InputStateLayer::Platform)
                .cloned()
                .unwrap_or_default();
            let batch = cx
                .core
                .native_input
                .key_event(
                    target,
                    physical,
                    &logical,
                    KeyLocation::Standard,
                    state,
                    false,
                    false,
                    committed,
                    &receipt,
                    &cx.core.text,
                )
                .unwrap();
            cx.receive_input_batch(app, batch).unwrap();
        }
        for delta in [0.25_f32, -0.5, 0.125] {
            let receipt = cx
                .core
                .input
                .state(target, InputStateLayer::Platform)
                .cloned()
                .unwrap_or_default();
            let event = WindowEvent::MouseWheel {
                device_id: DeviceId::dummy(),
                delta: MouseScrollDelta::LineDelta(0.0, delta),
                phase: TouchPhase::Moved,
            };
            let batch = cx
                .core
                .native_input
                .window_event(
                    target,
                    &event,
                    &receipt,
                    PhysicalSize {
                        width: 800,
                        height: 600,
                    },
                    1.5,
                    &mut cx.core.text,
                )
                .unwrap();
            cx.receive_input_batch(app, batch).unwrap();
        }
        assert!(app.events.is_empty());
        assert_eq!(cx.core.input.queue.len(), 8);
    });
    cx.drain_input(&mut app, 20).unwrap();
    assert_eq!(app.events.len(), 8);
    assert!(matches!(
        app.events[0],
        InputEvent::Key {
            logical: LogicalKey::Dead(Some('´')),
            ..
        }
    ));
    let text: Vec<_> = app
        .events
        .iter()
        .filter_map(|event| match event {
            InputEvent::Text { session, text } => Some((*session, text.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(text, [(session, "é")]);
    let wheel: Vec<_> = app
        .events
        .iter()
        .filter_map(|event| match event {
            InputEvent::Wheel {
                delta: ScrollDelta::Lines { y, .. },
                ..
            } => Some(*y),
            _ => None,
        })
        .collect();
    assert_eq!(wheel, [0.25, -0.5, 0.125]);
}

#[test]
fn first_native_wheel_applies_associated_metadata_at_receipt_and_before_handlers() {
    let mut core = Core::<App>::new().unwrap();
    let mut backend = TestBackend::default();
    let mut app = App::default();
    let mut cx = Context::new(
        &mut core,
        BackendOps::Test(&mut backend),
        ContextPhase::Startup,
    );
    let target = setup(&mut cx);
    cx.with_input_deferred(&mut app, |app, cx| {
        let event = winit::event::WindowEvent::MouseWheel {
            device_id: winit::event::DeviceId::dummy(),
            delta: winit::event::MouseScrollDelta::LineDelta(0.0, 0.25),
            phase: winit::event::TouchPhase::Moved,
        };
        let batch = cx
            .core
            .native_input
            .window_event(
                target,
                &event,
                &InputState::default(),
                PhysicalSize {
                    width: 800,
                    height: 600,
                },
                1.5,
                &mut cx.core.text,
            )
            .unwrap();
        cx.receive_input_batch(app, batch).unwrap();
        let platform = cx
            .input_state(target, InputStateLayer::Platform)
            .unwrap()
            .unwrap();
        assert_eq!(platform.scale, Some(1.5));
        assert_eq!(platform.position, None);
        assert!(
            cx.input_state(target, InputStateLayer::Delivered)
                .unwrap()
                .is_none()
        );
        assert!(app.events.is_empty());
    });
    cx.drain_input(&mut app, 1).unwrap();
    let delivered = cx
        .input_state(target, InputStateLayer::Delivered)
        .unwrap()
        .unwrap();
    assert_eq!(delivered.scale, Some(1.5));
    assert_eq!(delivered.position, None);
    assert_eq!(app.events.len(), 1);
}
