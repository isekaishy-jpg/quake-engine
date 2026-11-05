//! Owner-local host state and borrowed application callbacks.

use std::any::Any;
use std::io::Write;
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::thread::{self, ThreadId};
use std::time::Duration;

use crate::backend::{self, BackendOps, NativeWindowKey};
use crate::context::{Context, ContextPhase};
use crate::error::{Error, NativeOperation, Rejected};
use crate::identity::{HostId, WindowSlots};
use crate::window::WindowRecord;

/// Application callbacks borrow state for the host run; `Send` and `'static`
/// are deliberately not required. The host stores no application reference.
pub trait Application: Sized {
    /// Owned messages accepted through the cross-thread proxy.
    type Message: Send + 'static;
    /// Owner-local dispatch/timer payloads, permitted to borrow scoped data.
    type LocalEvent;
    /// Begins application startup with native creation available.
    ///
    /// Returning an error or unwinding still invokes `stopping`, retaining
    /// successfully created windows until domain cleanup reports settled.
    fn started(&mut self, cx: &mut Context<'_, Self>) -> Result<(), Error>;

    /// Performs bounded legal owner work for a registered service. Applications
    /// set/restore `engine_worker` phases and call `engine_worker`/`engine_cache`/providers directly. The default
    /// reports quiescent-for-now with independent maintenance fallback.
    fn service(
        &mut self,
        _: &mut Context<'_, Self>,
        _: crate::ServiceRequest,
    ) -> crate::ServiceReport {
        crate::ServiceReport::Quiescent
    }

    /// Runs application update/render work for an admitted redraw frame. Native
    /// callbacks cannot recursively enter a frame. Input is deferred here; an
    /// explicit service point drains only already received input.
    fn frame(&mut self, _: &mut Context<'_, Self>, _: crate::Frame) {}
    /// Receives owned display progress on the owner thread. Admission,
    /// native application and exact presentation readiness remain distinct.
    fn display_transition(&mut self, _: &mut Context<'_, Self>, _: &crate::DisplayTransition) {}

    /// Performs owner-thread cleanup, including after partial startup.
    ///
    /// Return `Settled` only when external work no longer needs `engine_host` windows,
    /// callbacks or borrowed context. `Pending` keeps the run and windows alive
    /// and retries at the configured interval. No timeout forces destruction.
    /// This callback must not panic: unwinding prevents `engine_host` from establishing
    /// final access and terminates the process through an explicit fail-stop.
    fn stopping(&mut self, cx: &mut Context<'_, Self>) -> StopProgress;
}

/// Host lifecycle, independent of resource or renderer readiness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostState {
    /// Event-loop construction or application startup is in progress.
    Constructing,
    /// Startup succeeded and native event delivery is active.
    Running,
    /// Stop latched; ordinary admission is closed and cleanup remains live.
    Stopping,
    /// Application obligations settled; `engine_host` is retiring native acquisitions.
    Retiring,
    /// Application cleanup and host-owned native retirement completed.
    Closed,
}

/// The application's current host-dependent cleanup result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopProgress {
    /// External final access still needs the host; retry without destroying it.
    Pending,
    /// No external obligation needs host callbacks, windows or borrowed state.
    Settled,
}

/// Outcome of a synchronous stop request; neither outcome means cleanup ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopOutcome {
    /// This request closed ordinary admission.
    Requested,
    /// A prior request already closed ordinary admission.
    AlreadyRequested,
}

/// The first reason stop latched, retained while cleanup continues.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopReason {
    /// The application called `request_stop`.
    Application,
    /// The OS requested closing any host window; stage 1 stops the whole host.
    WindowCloseRequested,
    /// Application startup returned an error.
    StartupFailed,
    /// Application startup panicked and retirement was attempted.
    CallbackPanicked,
    /// The backend could not continue its native run.
    BackendFailed,
}

/// Validated startup configuration. This foundation has no worker policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostConfig {
    /// Finite interval for retrying pending application retirement.
    /// Must be greater than zero and no longer than one second.
    pub stop_poll_interval: Duration,
    /// Maximum accepted posts, including detached and active routing.
    pub post_capacity: usize,
    /// Maximum accepted association requests, including active native calls.
    pub shell_capacity: usize,
    /// Maximum nested application dispatch or explicit timer pumps.
    pub nesting_limit: usize,
    /// Maximum ordinary post/input/timer records per native service visit.
    pub event_budget: usize,
    /// Finite fallback for durable post intake when native wake fails.
    pub post_recheck_interval: Duration,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            stop_poll_interval: Duration::from_millis(10),
            post_capacity: 256,
            shell_capacity: 16,
            nesting_limit: 32,
            event_budget: 128,
            post_recheck_interval: Duration::from_millis(100),
        }
    }
}

impl HostConfig {
    fn validate(&self) -> Result<(), Error> {
        if self.post_capacity == 0
            || self.shell_capacity == 0
            || self.nesting_limit == 0
            || self.event_budget == 0
        {
            return Err(Error::InvalidInput(
                "capacities, budgets and nesting limit must be positive",
            ));
        }
        if self.post_recheck_interval.is_zero()
            || self.post_recheck_interval > Duration::from_secs(1)
        {
            return Err(Error::InvalidInput(
                "post recheck interval must be in (0, 1 second]",
            ));
        }
        if self.stop_poll_interval.is_zero() || self.stop_poll_interval > Duration::from_secs(1) {
            return Err(Error::InvalidInput(
                "stop poll interval must be in (0, 1 second]",
            ));
        }
        Ok(())
    }
}

/// Normal return proves this run's application-reported and `engine_host`-owned host
/// obligations settled. It is not an independent GPU or provider certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExitReport {
    /// The same host identity used throughout this run.
    pub host: HostId,
    /// The first stop reason.
    pub reason: StopReason,
    /// Number of acquired native windows with observed destruction completion.
    pub windows_retired: usize,
}

/// A one-shot owner-thread host borrowing an application only during `run`.
///
/// Construct and run on the process main thread. The host and callback contexts
/// are neither `Send` nor `Sync`. Dropping an unrun host retires its event-loop
/// acquisition; no application callback has begun in that case. A second
/// event-loop construction in the same process is not promised by winit.
pub struct Host<A: Application> {
    pub(crate) core: Core<A>,
    event_loop: Option<winit::event_loop::EventLoop<()>>,
    config: HostConfig,
    application: PhantomData<fn(A) -> A>,
}

impl<A: Application> Host<A> {
    /// Constructs the native event loop and returns configuration ownership on
    /// validation or native failure. This does not call application startup.
    pub fn new(config: HostConfig) -> Result<Self, Rejected<HostConfig>> {
        if let Err(error) = config.validate() {
            return Err(Rejected::new(config, error));
        }
        let core = match Core::with_config(&config) {
            Ok(core) => core,
            Err(error) => return Err(Rejected::new(config, error)),
        };
        let event_loop = match backend::winit::create_event_loop() {
            Ok(event_loop) => event_loop,
            Err(error) => return Err(Rejected::new(config, error)),
        };
        if let Err(error) = core.native_wake.install(event_loop.create_proxy()) {
            return Err(Rejected::new(config, error));
        }
        Ok(Self {
            core,
            event_loop: Some(event_loop),
            config,
            application: PhantomData,
        })
    }

    /// Inspects host state outside the borrowed run.
    pub fn state(&self) -> HostState {
        self.core.state
    }
    /// The host identity does not change through startup and stop.
    pub fn id(&self) -> HostId {
        self.core.id
    }
    /// Creates an owned cross-thread intake handle; it never borrows the app.
    pub fn proxy(&self) -> crate::Proxy<A::Message> {
        self.core.proxy()
    }

    /// Runs the native loop, borrowing the application until cleanup settles.
    ///
    /// Startup failure returns its typed error only after `stopping` settles and
    /// created windows retire. Startup unwinding is recorded and cleanup still
    /// runs. Retirement panic or backend loss that leaves pending external
    /// obligations is fatal: `engine_host` aborts rather than returning invalid borrowed
    /// state. Dependency panics before app entry can return a native failure.
    /// Only one invocation is accepted, including a failed startup.
    pub fn run(&mut self, app: &mut A) -> Result<ExitReport, Error> {
        self.core.check_owner()?;
        let event_loop = self.event_loop.take().ok_or(Error::AlreadyRun)?;
        let native_result = catch_unwind(AssertUnwindSafe(|| {
            backend::winit::run(event_loop, &mut self.core, app, &self.config)
        }));
        let native_error = match native_result {
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(error),
            Err(payload) => Some(Error::Native {
                operation: NativeOperation::RunEventLoop,
                message: panic_message(payload),
            }),
        };
        if self.core.state != HostState::Closed {
            self.core.fail(
                native_error.clone().unwrap_or_else(|| Error::Native {
                    operation: NativeOperation::RunEventLoop,
                    message: String::from("native loop returned before host retirement"),
                }),
                StopReason::BackendFailed,
            );
            // One final owner-local cleanup opportunity, with windows retained.
            // The unavailable backend cannot safely sustain further pending work.
            if self.core.app_entered {
                self.core.poll_stop(app, BackendOps::Unavailable);
                if self.core.state != HostState::Closed {
                    fail_stop("backend exited with pending application obligations");
                }
            } else {
                self.core.finish();
            }
        }
        if let Some(error) = self.core.failure.take().or(native_error) {
            return Err(error);
        }
        Ok(ExitReport {
            host: self.core.id,
            reason: self.core.stop_reason.unwrap_or(StopReason::BackendFailed),
            windows_retired: self.core.retired_windows,
        })
    }
}

pub(crate) struct Core<A: Application> {
    pub(crate) id: HostId,
    owner: ThreadId,
    pub(crate) state: HostState,
    pub(crate) failure: Option<Error>,
    pub(crate) stop_reason: Option<StopReason>,
    pub(crate) windows: WindowSlots<WindowRecord>,
    pub(crate) native_windows: Vec<(NativeWindowKey, crate::WindowTarget)>,
    retired_windows: usize,
    pub(crate) app_entered: bool,
    pub(crate) application_settled: bool,
    pub(crate) active_callbacks: usize,
    thread_bound: PhantomData<Rc<()>>,
    pub(crate) dispatch: crate::dispatch::Dispatch<A>,
    pub(crate) timers: crate::timer::Timers<A::LocalEvent>,
    pub(crate) input: crate::input::Input,
    pub(crate) native_input: crate::backend::winit_input::NativeInputAdapter,
    pub(crate) text: crate::text::TextState,
    pub(crate) raw_input: crate::RawInputState,
    pub(crate) relative_target: Option<crate::WindowTarget>,
    pub(crate) transport: std::sync::Arc<crate::post::Transport<A::Message>>,
    pub(crate) post_batches: crate::post::BatchCache<A::Message>,
    pub(crate) clock: crate::time::Clock,
    pub(crate) nesting_limit: usize,
    pub(crate) event_budget: usize,
    pub(crate) services: crate::service::Services,
    pub(crate) pacing: crate::pacing::Pacing,
    pub(crate) native_wake: crate::backend::wake::NativeWake,
    pub(crate) display_events: std::collections::VecDeque<crate::DisplayTransition>,
    pub(crate) shell: Option<crate::shell::ShellHelper>,
    pub(crate) shell_capacity: usize,
}

impl<A: Application> Core<A> {
    #[cfg(test)]
    pub(crate) fn new() -> Result<Self, Error> {
        Self::with_config(&HostConfig::default())
    }
    pub(crate) fn with_config(config: &HostConfig) -> Result<Self, Error> {
        let id = HostId::allocate()?;
        let native_wake = crate::backend::wake::NativeWake::new();
        Ok(Self {
            id,
            owner: thread::current().id(),
            state: HostState::Constructing,
            failure: None,
            stop_reason: None,
            windows: WindowSlots::new(id),
            native_windows: Vec::new(),
            retired_windows: 0,
            app_entered: false,
            application_settled: false,
            active_callbacks: 0,
            thread_bound: PhantomData,
            dispatch: crate::dispatch::Dispatch::new(),
            timers: crate::timer::Timers::new(id),
            input: crate::input::Input::new(),
            native_input: crate::backend::winit_input::NativeInputAdapter::new(),
            text: crate::text::TextState::new(),
            raw_input: crate::RawInputState::default(),
            relative_target: None,
            transport: std::sync::Arc::new(crate::post::Transport::new(
                id,
                config.post_capacity,
                native_wake.clone(),
            )?),
            post_batches: crate::post::BatchCache::new(),
            clock: crate::time::Clock::new(id),
            nesting_limit: config.nesting_limit,
            event_budget: config.event_budget,
            services: crate::service::Services::new(),
            pacing: crate::pacing::Pacing::new(),
            native_wake,
            display_events: std::collections::VecDeque::new(),
            shell: None,
            shell_capacity: config.shell_capacity,
        })
    }

    pub(crate) fn check_owner(&self) -> Result<(), Error> {
        if self.owner != thread::current().id() {
            Err(Error::WrongThread)
        } else {
            Ok(())
        }
    }

    pub(crate) fn ordinary_open(&self) -> bool {
        matches!(self.state, HostState::Constructing | HostState::Running)
    }
    pub(crate) fn proxy(&self) -> crate::Proxy<A::Message> {
        crate::Proxy {
            transport: std::sync::Arc::clone(&self.transport),
        }
    }

    pub(crate) fn refresh_cursor_suppression(&mut self) {
        let ordinary = self.ordinary_open();
        for (id, record) in self.windows.iter_mut() {
            let target = crate::WindowTarget {
                id,
                generation: record.generation,
            };
            let suppress = ordinary
                && self.relative_target == Some(target)
                && record.input_mode.confirmed.relative_motion
                && self
                    .input
                    .state(target, crate::InputStateLayer::Platform)
                    .is_some_and(|state| state.focused);
            if record.cursor.input_suppressed != suppress {
                record.cursor.input_suppressed = suppress;
                record
                    .native
                    .cursor_visible(record.cursor.requested_visible && !suppress);
            }
        }
    }

    pub(crate) fn dispose<T>(&mut self, value: T) {
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(value))) {
            self.fail(
                Error::ApplicationPanicked {
                    phase: crate::ContextPhase::Event,
                    message: panic_message(payload),
                },
                StopReason::CallbackPanicked,
            );
        }
    }

    fn settle_pending(&mut self) {
        self.transport.close();
        while let Some(post) = self.transport.pop() {
            post.receipt.settle(crate::PostOutcome::Discarded(
                crate::PostDiscardReason::HostStopped,
            ));
            self.dispose(post.message);
            self.transport.settled();
        }
        while let Some(event) = self.timers.cancel_next() {
            self.dispose(event);
        }
        self.input.queue.clear();
    }

    pub(crate) fn pump_ordinary(&mut self, app: &mut A, operations: BackendOps<'_>) {
        if self.state != HostState::Running {
            return;
        }
        let budget = self.event_budget;
        let mut cx = Context::new(self, operations, ContextPhase::Event);
        let result = (|| {
            if cx.core.input.enabled {
                cx.drain_input(app, budget)?;
            }
            if cx.core.ordinary_open() {
                cx.drain_posts(app, budget)?;
            }
            if cx.core.ordinary_open() {
                cx.service_timers(app, budget)?;
            }
            if cx.core.ordinary_open() {
                cx.service_pending(app, crate::ServicePoint::Maintenance, budget)?;
            }
            Ok::<(), Error>(())
        })();
        if let Err(error) = result {
            cx.core.fail(error, StopReason::CallbackPanicked);
        }
    }

    pub(crate) fn ordinary_pending(&self) -> bool {
        !self.display_events.is_empty()
            || self.windows.iter().any(|(_, record)| {
                !record.closing
                    && record.display.pending().is_some()
                    && record.display.active().is_none()
            })
            || !self.input.queue.is_empty()
            || self.transport.len() > 0
            || self.timers.due(self.clock.sample())
            || self
                .services
                .pending(crate::ServicePoint::Maintenance, self.clock.sample())
    }

    pub(crate) fn pump_retirement(&mut self, app: &mut A, operations: BackendOps<'_>) {
        if self.state != HostState::Stopping || self.application_settled {
            return;
        }
        self.settle_pending();
        let budget = self.event_budget;
        let mut cx = Context::new(self, operations, ContextPhase::Retirement);
        if let Err(error) = cx.service_pending(app, crate::ServicePoint::Retirement, budget) {
            cx.core.fail(error, StopReason::BackendFailed);
        }
    }

    pub(crate) fn prepare_redraw(&mut self) {
        if self.state != HostState::Running {
            return;
        }
        if let Some(target) = self.pacing.arm_redraw(self.clock.sample()) {
            if let Ok(record) = self.windows.get(target.id)
                && record.generation == target.generation
                && !record.closing
            {
                record.native.request_redraw();
            } else {
                self.pacing
                    .set(None, crate::PacingPolicy::Disabled, self.clock.sample());
            }
        }
    }

    pub(crate) fn frame(
        &mut self,
        app: &mut A,
        operations: BackendOps<'_>,
        target: crate::WindowTarget,
    ) {
        if self.state != HostState::Running {
            return;
        }
        if !self.pacing.eligible(target, self.clock.sample()) {
            return;
        }
        let budget = self.event_budget;
        let mut cx = Context::new(self, operations, ContextPhase::Event);
        if let Err(error) = cx.service_pending(app, crate::ServicePoint::PreUpdate, budget) {
            cx.core.fail(error, StopReason::BackendFailed);
            return;
        }
        if !cx.core.ordinary_open() {
            return;
        }
        let raw = cx.core.clock.sample();
        let frame = cx
            .core
            .clock
            .application(raw)
            .and_then(|application| cx.core.pacing.claim(target, raw, application));
        let frame = match frame {
            Ok(Some(frame)) => frame,
            Ok(None) => return,
            Err(error) => {
                cx.core.fail(error, StopReason::BackendFailed);
                return;
            }
        };
        let previous = cx.core.input.enabled;
        cx.core.input.enabled = false;
        let result = cx.frame_callback(app, frame);
        cx.core.input.enabled = previous;
        let completion = cx.core.pacing.complete(cx.core.clock.sample());
        if let Err(error) = result {
            cx.core.fail(error, StopReason::CallbackPanicked);
        }
        if let Err(error) = completion {
            cx.core.fail(error, StopReason::BackendFailed);
        }
    }

    pub(crate) fn request_stop(&mut self, reason: StopReason) -> StopOutcome {
        if self.stop_reason.is_some() {
            return StopOutcome::AlreadyRequested;
        }
        self.stop_reason = Some(reason);
        self.transport.close();
        if let Some(helper) = &self.shell {
            helper.close();
        }
        self.state = HostState::Stopping;
        self.refresh_cursor_suppression();
        StopOutcome::Requested
    }

    pub(crate) fn fail(&mut self, error: Error, reason: StopReason) {
        if self.failure.is_none() {
            self.failure = Some(error);
        }
        self.request_stop(reason);
    }

    pub(crate) fn start(&mut self, app: &mut A, operations: BackendOps<'_>) {
        if self.app_entered || self.state != HostState::Constructing {
            return;
        }
        self.app_entered = true;
        self.active_callbacks += 1;
        let result = catch_unwind(AssertUnwindSafe(|| {
            app.started(&mut Context::new(self, operations, ContextPhase::Startup))
        }));
        self.active_callbacks -= 1;
        match result {
            Ok(Ok(())) => {
                if self.state == HostState::Constructing {
                    self.state = HostState::Running;
                }
            }
            Ok(Err(error)) => self.fail(error, StopReason::StartupFailed),
            Err(payload) => self.fail(
                Error::ApplicationPanicked {
                    phase: ContextPhase::Startup,
                    message: panic_message(payload),
                },
                StopReason::CallbackPanicked,
            ),
        }
    }

    fn release_native_input(&mut self, operations: &mut BackendOps<'_>) -> bool {
        let mut settled = true;
        let mut failure = None;
        if self.raw_input.confirmed != Some(crate::RawInputPolicy::Never)
            || self.raw_input.failure.is_some()
        {
            let result = operations.raw_input(crate::RawInputPolicy::Never);
            self.raw_input.failure = result.as_ref().err().cloned();
            match result {
                Ok(()) => self.raw_input.confirmed = Some(crate::RawInputPolicy::Never),
                Err(error) => {
                    settled = false;
                    failure = Some(error);
                }
            }
        }
        for (_, record) in self.windows.iter_mut() {
            if record.input_mode.confirmed.confine_pointer || record.confinement_uncertain {
                match record.native.confine(false) {
                    Ok(()) => {
                        record.input_mode.confirmed.confine_pointer = false;
                        record.confinement_uncertain = false;
                    }
                    Err(error) => {
                        settled = false;
                        record.input_mode.failure = Some(error.clone());
                        failure.get_or_insert(error);
                    }
                }
            }
            if self.raw_input.confirmed == Some(crate::RawInputPolicy::Never) {
                record.input_mode.confirmed.relative_motion = false;
            }
        }
        if self.raw_input.confirmed == Some(crate::RawInputPolicy::Never) {
            self.relative_target = None;
        }
        if let Some(error) = failure {
            self.fail(error, StopReason::BackendFailed);
        }
        settled
    }

    pub(crate) fn poll_stop(&mut self, app: &mut A, mut operations: BackendOps<'_>) {
        if self.state == HostState::Retiring {
            self.reap_windows();
            return;
        }
        if self.state != HostState::Stopping {
            return;
        }
        self.settle_pending();
        let shell_settled = self.poll_shell_retirement();
        let native_input_settled = self.release_native_input(&mut operations) && shell_settled;
        if self.application_settled {
            if native_input_settled {
                self.finish();
            }
            return;
        }
        self.active_callbacks += 1;
        let progress = catch_unwind(AssertUnwindSafe(|| {
            app.stopping(&mut Context::new(
                self,
                operations,
                ContextPhase::Retirement,
            ))
        }));
        self.active_callbacks -= 1;
        match progress {
            Ok(StopProgress::Settled) => {
                self.application_settled = true;
                self.services.close_all();
                if native_input_settled {
                    self.finish();
                }
            }
            Ok(StopProgress::Pending) => (),
            Err(payload) => {
                // A user-defined panic payload destructor must not prevent abort.
                std::mem::forget(payload);
                fail_stop("application retirement panicked before final access settled");
            }
        }
    }

    fn finish(&mut self) {
        self.state = HostState::Retiring;
        self.input.retire_all();
        self.text.clear();
        self.services.close_all();
        self.native_input.clear();
        self.relative_target = None;
        for (_, record) in self.windows.iter_mut() {
            record.closing = true;
            record.access.begin_retirement();
            let _ = record.display.begin_retirement();
            if let Some(active) = record.display.active().cloned() {
                let _ = record.display.fail(active.receipt, Error::AdmissionClosed);
            }
        }
        self.display_events.clear();
        self.reap_windows();
        // Winit posts native destruction; dropping roots is not completion.
        if self.native_windows.is_empty() {
            self.state = HostState::Closed;
            self.post_batches.clear();
        }
    }

    pub(crate) fn reap_windows(&mut self) {
        while let Some(record) = self
            .windows
            .remove_where(|record| record.closing && record.access.external_count() == 0)
        {
            drop(record);
        }
        if self.state == HostState::Retiring && self.native_windows.is_empty() {
            self.state = HostState::Closed;
            self.post_batches.clear();
        }
    }

    pub(crate) fn native_destroyed(&mut self, key: NativeWindowKey) {
        if let Some(index) = self
            .native_windows
            .iter()
            .position(|(current, _)| *current == key)
        {
            let target = self.native_windows[index].1;
            let record = self.windows.get(target.id);
            if record.as_ref().is_ok_and(|record| {
                record.generation == target.generation && record.native.key() == key
            }) {
                self.input.retire(target);
                self.text.retire(target);
                self.native_input.retire(target);
                if self.relative_target == Some(target) {
                    self.relative_target = None;
                }
                if let Ok(record) = self.windows.remove(target.id) {
                    record.access.invalidate_and_retain_native_root();
                    record.native.already_destroyed();
                }
                self.fail(
                    Error::Native {
                        operation: NativeOperation::RunEventLoop,
                        message: String::from("native window destroyed before host retirement"),
                    },
                    StopReason::BackendFailed,
                );
            } else if record.is_ok() {
                // An inconsistent old association is not authority to invalidate
                // a different live acquisition. Retain its roots for stop/drain.
                self.fail(
                    Error::Native {
                        operation: NativeOperation::RunEventLoop,
                        message: String::from(
                            "native destruction ledger does not match live window",
                        ),
                    },
                    StopReason::BackendFailed,
                );
            } else if target.generation == crate::WindowGeneration::INITIAL {
                self.windows.cancel_reservation(target.id);
            }
            self.native_windows.remove(index);
            self.retired_windows += 1;
            if self.state == HostState::Retiring && self.native_windows.is_empty() {
                self.state = HostState::Closed;
                self.post_batches.clear();
            }
        }
    }
}

impl<A: Application> Drop for Core<A> {
    fn drop(&mut self) {
        self.native_wake.close();
        if self
            .windows
            .iter()
            .any(|(_, record)| record.access.external_count() != 0)
        {
            fail_stop("host dropped while external native window access remained");
        }
        // The owner root, never a last foreign-thread proxy, destroys accepted
        // payloads. Proxies may outlive this root only with a closed empty queue.
        self.settle_pending();
        self.services.close_all();
    }
}

impl<A: Application> Drop for Host<A> {
    fn drop(&mut self) {
        // An unrun host still owns the hidden native wake window. Seal all
        // external destinations before automatic event-loop destruction.
        self.core.native_wake.close();
    }
}

pub(crate) fn panic_message(payload: Box<dyn Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => match payload.downcast::<&'static str>() {
            Ok(message) => String::from(*message),
            Err(payload) => {
                // Arbitrary user Drop code could panic again and bypass cleanup.
                // Deliberately leak this fault-only payload until process exit.
                std::mem::forget(payload);
                String::from("non-string panic payload (retained until process exit)")
            }
        },
    }
}

pub(crate) fn fail_stop(reason: &str) -> ! {
    let _ = writeln!(
        std::io::stderr(),
        "engine_host fatal retirement failure: {reason}"
    );
    std::process::abort()
}

#[cfg(test)]
#[path = "../tests/unit/window_retirement.rs"]
mod window_retirement_tests;
