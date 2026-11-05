use crate::backend::{BackendOps, test::TestBackend};
use crate::host::Core;
use crate::*;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Duration;

struct App {
    target: Option<WindowTarget>,
    records: Vec<InputRecord>,
    test_limit: bool,
    session: Option<TextSessionId>,
}
impl Application for App {
    type Message = String;
    type LocalEvent = u8;
    fn started(&mut self, _: &mut Context<'_, Self>) -> Result<(), Error> {
        Ok(())
    }
    fn stopping(&mut self, _: &mut Context<'_, Self>) -> StopProgress {
        StopProgress::Settled
    }
}
fn handler(app: &mut App, cx: &mut Context<'_, App>, event: &Event<'_, String, u8>) -> Propagation {
    if let Event::Input(record) = event {
        assert!(record.stamp.delivery.is_some());
        let delivered = cx
            .input_state(record.target, InputStateLayer::Delivered)
            .unwrap()
            .unwrap();
        if let InputEvent::PointerMoved(position) = record.event {
            assert_eq!(delivered.position, Some(position));
        }
        app.records.push((*record).clone());
    } else if app.test_limit {
        assert_eq!(cx.drain_input(app, 8), Err(Error::NestingLimit));
    }
    Propagation::Continue
}
fn app() -> App {
    App {
        target: None,
        records: Vec::new(),
        test_limit: false,
        session: None,
    }
}
fn setup(core: &mut Core<App>, backend: &mut TestBackend, app: &mut App) -> WindowTarget {
    let mut cx = Context::new(core, BackendOps::Test(backend), ContextPhase::Startup);
    let target = cx.create_window(WindowSpec::default()).unwrap();
    app.target = Some(target);
    app.session = Some(
        cx.begin_text(
            target,
            TextCaret {
                position: PhysicalPosition { x: 0.0, y: 0.0 },
                size: PhysicalSize {
                    width: 1,
                    height: 1,
                },
            },
        )
        .unwrap(),
    );
    let recipient = cx.create_recipient().unwrap();
    cx.subscribe(recipient, EventFilter::All, Priority::default(), handler)
        .unwrap();
    target
}
fn receive(cx: &mut Context<'_, App>, app: &mut App, event: InputEvent) {
    cx.receive_input(
        app,
        app.target.unwrap(),
        event,
        InputOrigin::NativeWindow,
        None,
    )
    .unwrap();
}

#[test]
fn ordinary_immediate_and_growing_deferred_delivery_preserve_payload_coordinates_and_state_layers()
{
    let mut core = Core::<App>::new().unwrap();
    let mut backend = TestBackend::default();
    let mut app = app();
    let target = setup(&mut core, &mut backend, &mut app);
    let session = app.session.unwrap();
    let mut cx = Context::new(
        &mut core,
        BackendOps::Test(&mut backend),
        ContextPhase::Event,
    );
    receive(
        &mut cx,
        &mut app,
        InputEvent::PointerMoved(PhysicalPosition { x: 1.0, y: 2.0 }),
    );
    assert_eq!(app.records.len(), 1);
    cx.with_input_deferred(&mut app, |app, cx| {
        for value in 0..40 {
            receive(
                cx,
                app,
                InputEvent::PointerMoved(PhysicalPosition {
                    x: f64::from(value),
                    y: 0.0,
                }),
            );
        }
        receive(
            cx,
            app,
            InputEvent::MouseButton {
                button: MouseButton::Left,
                state: ButtonState::Pressed,
                position: Some(PhysicalPosition { x: 39.0, y: 0.0 }),
                scale: 1.25,
            },
        );
        receive(
            cx,
            app,
            InputEvent::Text {
                session,
                text: String::from("héllo"),
            },
        );
        assert_eq!(app.records.len(), 1);
        assert_eq!(
            cx.input_state(target, InputStateLayer::Platform)
                .unwrap()
                .unwrap()
                .position
                .unwrap()
                .x,
            39.0
        );
        assert_eq!(
            cx.input_state(target, InputStateLayer::Delivered)
                .unwrap()
                .unwrap()
                .position
                .unwrap()
                .x,
            1.0
        );
        let report = cx.drain_input(app, 8).unwrap();
        assert_eq!(report.consumed, 8);
        assert_eq!(report.remaining, 34);
        assert!(!cx.core.input.enabled);
        receive(
            cx,
            app,
            InputEvent::Text {
                session,
                text: String::from("still deferred"),
            },
        );
        assert_eq!(app.records.len(), 9);
    });
    assert!(cx.core.input.enabled);
    cx.drain_input(&mut app, 128).unwrap();
    assert_eq!(app.records.len(), 44);
    assert!(matches!(
        app.records[41].event,
        InputEvent::MouseButton {
            position: Some(PhysicalPosition { x: 39.0, .. }),
            ..
        }
    ));
    assert_eq!(
        app.records[42].event,
        InputEvent::Text {
            session,
            text: String::from("héllo")
        }
    );
    assert!(
        app.records
            .windows(2)
            .all(|pair| pair[0].stamp.sequence < pair[1].stamp.sequence)
    );
    assert!(
        app.records
            .iter()
            .all(|record| record.stamp.source_time.is_none())
    );
}

#[test]
fn nested_gate_restores_on_panic_and_dispatch_limit_keeps_undelivered_input_queued() {
    let mut core = Core::<App>::new().unwrap();
    core.nesting_limit = 1;
    let mut backend = TestBackend::default();
    let mut app = app();
    setup(&mut core, &mut backend, &mut app);
    let session = app.session.unwrap();
    let mut cx = Context::new(
        &mut core,
        BackendOps::Test(&mut backend),
        ContextPhase::Event,
    );
    cx.with_input_deferred(&mut app, |app, cx| {
        let result = catch_unwind(AssertUnwindSafe(|| {
            cx.with_input_deferred(app, |_, _| panic!("guarded work"))
        }));
        assert!(result.is_err());
        assert!(!cx.core.input.enabled);
        receive(
            cx,
            app,
            InputEvent::Text {
                session,
                text: String::from("retained"),
            },
        );
    });
    app.test_limit = true;
    cx.dispatch_local(&mut app, &0).unwrap();
    assert_eq!(cx.core.input.queue.len(), 1);
    assert!(app.records.is_empty());
    app.test_limit = false;
    cx.drain_input(&mut app, 1).unwrap();
    assert_eq!(app.records.len(), 1);
}

#[test]
fn intake_or_delivered_allocation_failure_faults_admission_without_fabricating_lossless_delivery() {
    for delivery in [false, true] {
        let mut core = Core::<App>::new().unwrap();
        let mut backend = TestBackend::default();
        let mut app = app();
        let target = setup(&mut core, &mut backend, &mut app);
        core.input.fail_next_queue_reservation = !delivery;
        core.input.fail_next_delivery_reservation = delivery;
        let mut cx = Context::new(
            &mut core,
            BackendOps::Test(&mut backend),
            ContextPhase::Event,
        );
        let result = cx.receive_input(
            &mut app,
            target,
            InputEvent::Focus(true),
            InputOrigin::NativeWindow,
            None,
        );
        assert_eq!(result, Err(Error::AllocationFailed));
        assert!(!cx.admission().ordinary_open);
        assert_eq!(cx.fault(), Some(&Error::AllocationFailed));
        assert!(app.records.is_empty());
    }
}

#[test]
fn raw_deadlines_are_host_scoped_and_arithmetic_is_checked() {
    let mut core = Core::<App>::new().unwrap();
    core.clock.manual = Some(Duration::MAX);
    assert_eq!(
        core.clock.sample().checked_add(Duration::from_nanos(1)),
        Err(Error::TimeOverflow)
    );
}

#[test]
fn marker_reservation_failure_preserves_owned_input_before_delivered_state_changes() {
    let mut core = Core::<App>::new().unwrap();
    let mut backend = TestBackend::default();
    let mut app = app();
    let target = setup(&mut core, &mut backend, &mut app);
    let mut cx = Context::new(
        &mut core,
        BackendOps::Test(&mut backend),
        ContextPhase::Event,
    );
    cx.with_input_deferred(&mut app, |app, cx| {
        receive(cx, app, InputEvent::Focus(true))
    });
    cx.core.dispatch.fail_next_marker_reservation = true;
    assert_eq!(cx.drain_input(&mut app, 1), Err(Error::AllocationFailed));
    assert_eq!(cx.core.input.queue.len(), 1);
    assert!(
        cx.input_state(target, InputStateLayer::Delivered)
            .unwrap()
            .is_none()
    );
    assert!(app.records.is_empty());
    assert!(cx.admission().ordinary_open);
    cx.drain_input(&mut app, 1).unwrap();
    assert_eq!(app.records.len(), 1);
}
