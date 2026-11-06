//! Temporary callback capabilities. No application reference is stored here.

use crate::backend::BackendOps;
use crate::dispatch::{
    DispatchReport, Event, EventFilter, Handler, Priority, Propagation, Recipient, RecipientState,
};
use crate::error::{Error, Rejected};
use crate::host::{Application, Core, HostState, StopOutcome, StopReason};
use crate::identity::{HostId, WindowGeneration, WindowTarget};
use crate::identity::{RecipientId, SubscriptionId, TimerId};
#[cfg(test)]
use crate::input::InputOrigin;
use crate::input::{InputDrainReport, InputEvent, InputState, InputStateLayer};
use crate::post::{PostDiscardReason, PostOutcome, Proxy};
use crate::time::{ClockSnapshot, RawDeadline};
use crate::timer::{TimerCancel, TimerPumpReport};
use crate::window::{WindowRecord, WindowSpec, WindowState};
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
#[cfg(test)]
use std::time::Duration;

/// The supported callback context. Later service phases are not implied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextPhase {
    /// Delivery of serialized display progress.
    Display,
    /// The first application callback, with native creation available.
    Startup,
    /// Ordinary owner-local event routing, explicit input drains and timer pumps.
    Event,
    /// A legal bounded owner service boundary; `engine_worker` phases remain application-defined.
    Service(crate::ServicePoint),
    /// Guarded application update/render callback.
    Frame,
    /// Application retirement; queries and stop inspection remain available.
    Retirement,
}

/// Ordinary versus retirement admission observed in this callback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmissionState {
    /// New ordinary window operations are allowed.
    pub ordinary_open: bool,
    /// Host-dependent cleanup is still being serviced.
    pub retirement_open: bool,
}

/// A temporary owner-thread capability borrowing only host mechanisms.
///
/// The context cannot be retained beyond its callback or sent to another
/// thread. Its application type is invariant, including any borrowed data's
/// lifetimes. The host never stores `&mut A` in this context.
pub struct Context<'cx, A: Application> {
    pub(crate) core: &'cx mut Core<A>,
    operations: BackendOps<'cx>,
    phase: ContextPhase,
    application: PhantomData<fn(A) -> A>,
}

impl<'cx, A: Application> Context<'cx, A> {
    pub(crate) fn new(
        core: &'cx mut Core<A>,
        operations: BackendOps<'cx>,
        phase: ContextPhase,
    ) -> Self {
        Self {
            core,
            operations,
            phase,
            application: PhantomData,
        }
    }

    /// The identity of this persistent host.
    pub fn host_id(&self) -> HostId {
        self.core.id
    }
    /// The lifecycle state, not domain or renderer readiness.
    pub fn state(&self) -> HostState {
        self.core.state
    }
    /// The current callback's capabilities.
    pub fn phase(&self) -> ContextPhase {
        self.phase
    }
    /// Admission closes immediately when stop latches.
    pub fn admission(&self) -> AdmissionState {
        AdmissionState {
            ordinary_open: self.core.ordinary_open(),
            retirement_open: self.core.state != HostState::Closed,
        }
    }
    /// The first host fault, retained through application retirement.
    pub fn fault(&self) -> Option<&Error> {
        self.core.failure.as_ref()
    }
    /// Why stop first latched. Later faults do not erase this reason.
    pub fn stop_reason(&self) -> Option<StopReason> {
        self.core.stop_reason
    }
    /// Latches stop without destroying this callback or any window.
    pub fn request_stop(&mut self) -> StopOutcome {
        self.core.request_stop(StopReason::Application)
    }

    pub(crate) fn ordinary(&self) -> Result<(), Error> {
        self.core.check_owner()?;
        if self.core.ordinary_open() {
            Ok(())
        } else {
            Err(Error::AdmissionClosed)
        }
    }

    /// Fresh host-scoped raw clock sample; no application-time scaling is applied.
    pub fn clock(&self) -> ClockSnapshot {
        let raw = self.core.clock.sample();
        ClockSnapshot {
            raw,
            application: self.core.clock.application(raw),
        }
    }

    /// Explicit optional application-time multiplier. Zero pauses the application
    /// clock; changing it preserves continuity and never rescales raw deadlines.
    ///
    /// # Errors
    ///
    /// Returns an error if the multiplier isn't finite and at least zero.
    pub fn rescale_application_time(&mut self, multiplier: f64) -> Result<(), Error> {
        self.ordinary()?;
        self.core.clock.rescale(multiplier)
    }

    /// Registers an owner callback opportunity with its own finite source recheck.
    ///
    /// # Errors
    ///
    /// Returns an error if the service is invalid, there's no room, or the host is closing.
    pub fn register_service(
        &mut self,
        spec: crate::ServiceSpec,
    ) -> Result<crate::ServiceId, Error> {
        self.ordinary()?;
        self.core.services.register(
            self.core.id,
            spec,
            self.core.clock.sample(),
            self.core.native_wake.clone(),
        )
    }

    /// Removes future visits immediately, retaining an active callback record.
    /// Removal does not certify an external route/provider/domain has retired.
    ///
    /// # Errors
    ///
    /// Returns an error if the service isn't current.
    pub fn remove_service(&mut self, id: crate::ServiceId) -> Result<crate::ServiceState, Error> {
        self.core.check_owner()?;
        self.core.services.remove(self.core.id, id)
    }

    /// Queries generation and active callback retention.
    ///
    /// # Errors
    ///
    /// Returns an error if the service isn't current.
    pub fn service_state(&self, id: crate::ServiceId) -> Result<crate::ServiceState, Error> {
        self.core.check_owner()?;
        let service = self.core.services.get(self.core.id, id)?;
        Ok(crate::ServiceState {
            retired: !service.alive,
            active: service.active,
            report: service.report,
        })
    }

    /// Creates an owned signal destination for this exact source generation.
    ///
    /// # Errors
    ///
    /// Returns an error if the service isn't current.
    pub fn wake(&self, id: crate::ServiceId) -> Result<crate::Wake, Error> {
        self.core.check_owner()?;
        let service = self.core.services.get(self.core.id, id)?;
        if !service.alive {
            return Err(Error::StaleIdentity);
        }
        Ok(service.wake.clone())
    }

    /// Services bounded eligible callbacks without polling winit or entering a
    /// recursive frame. Active services are skipped; other services may progress.
    /// An explicit point may drain already received input while ordinary admission
    /// is open. Owner waits requiring fresh OS/provider progress must yield a
    /// continuation to the native loop. Previous context/input phase is restored.
    ///
    /// # Errors
    ///
    /// Returns an error if services can't be run from here.
    pub fn service_pending(
        &mut self,
        app: &mut A,
        point: crate::ServicePoint,
        callback_budget: usize,
    ) -> Result<crate::ServiceVisit, Error> {
        self.core.check_owner()?;
        if point == crate::ServicePoint::Retirement {
            if self.core.state != HostState::Stopping || self.core.application_settled {
                return Err(Error::InvalidContext(self.phase));
            }
        } else {
            self.ordinary()?;
        }
        if self.core.services.depth >= self.core.nesting_limit
            || self.core.dispatch.depth >= self.core.nesting_limit
        {
            return Err(Error::NestingLimit);
        }
        self.core.services.depth += 1;
        let old_phase = self.phase;
        let old_gate = self.core.input.enabled;
        self.phase = ContextPhase::Service(point);
        self.core.input.enabled = false;
        let mut callbacks = 0;
        let mut failure = None;
        if point == crate::ServicePoint::Explicit
            && let Err(error) = self.drain_input(app, self.core.event_budget)
        {
            failure = Some(error);
        }
        while failure.is_none()
            && callbacks < callback_budget
            && (self.core.ordinary_open() || point == crate::ServicePoint::Retirement)
        {
            let sample = self.core.clock.sample();
            let Some(request) = self.core.services.next(self.core.id, point, sample) else {
                break;
            };
            self.core.active_callbacks += 1;
            let result = catch_unwind(AssertUnwindSafe(|| app.service(self, request)));
            self.core.active_callbacks -= 1;
            callbacks += 1;
            match result {
                Ok(report) => {
                    if let Err(error) =
                        self.core
                            .services
                            .release(self.core.id, request, Some(report))
                    {
                        self.core.fail(error.clone(), StopReason::BackendFailed);
                        failure = Some(error);
                    }
                }
                Err(payload) => {
                    let _ = self.core.services.release(self.core.id, request, None);
                    if point == crate::ServicePoint::Retirement {
                        std::mem::forget(payload);
                        crate::host::fail_stop(
                            "required retirement service panicked before final access settled",
                        );
                    }
                    let error = Error::ApplicationPanicked {
                        phase: self.phase,
                        message: crate::host::panic_message(payload),
                    };
                    self.core.fail(error.clone(), StopReason::CallbackPanicked);
                    failure = Some(error);
                }
            }
        }
        self.phase = old_phase;
        self.core.input.enabled = old_gate;
        self.core.services.depth -= 1;
        if let Some(error) = failure {
            return Err(error);
        }
        Ok(crate::ServiceVisit {
            callbacks,
            continuation: self.core.services.pending(point, self.core.clock.sample()),
        })
    }

    /// Selects a frame target and explicit limiter owner/profile. Disabled is the
    /// default; renderer-managed mode adds no physical interval limiter.
    ///
    /// # Errors
    ///
    /// Returns an error if the target or the limiter isn't valid.
    pub fn set_pacing(
        &mut self,
        target: Option<WindowTarget>,
        policy: crate::PacingPolicy,
    ) -> Result<(), Error> {
        self.ordinary()?;
        if let Some(target) = target {
            self.require_live_window(target)?;
        }
        if target.is_none() && policy != crate::PacingPolicy::Disabled {
            return Err(Error::InvalidInput("enabled pacing needs a window target"));
        }
        self.core
            .pacing
            .set(target, policy, self.core.clock.sample());
        Ok(())
    }

    /// Supplies graphics activation independently of normalized OS focus.
    ///
    /// # Errors
    ///
    /// Returns an error if the host is closing.
    pub fn set_renderer_active(&mut self, active: bool) -> Result<(), Error> {
        self.ordinary()?;
        self.core
            .pacing
            .renderer_active(active, self.core.clock.sample());
        Ok(())
    }

    /// Requests the selected target's next frame without bypassing a finite cap.
    ///
    /// # Errors
    ///
    /// Returns an error if no frame can be asked for now.
    pub fn request_frame(&mut self) -> Result<(), Error> {
        self.ordinary()?;
        self.core.pacing.request(self.core.clock.sample())
    }

    /// Queries selected profile and application-supplied graphics activation.
    pub fn pacing_state(&self) -> crate::PacingState {
        self.core.pacing.state
    }

    pub(crate) fn frame_callback(&mut self, app: &mut A, frame: crate::Frame) -> Result<(), Error> {
        let previous = self.phase;
        self.phase = ContextPhase::Frame;
        self.core.active_callbacks += 1;
        let result = catch_unwind(AssertUnwindSafe(|| app.frame(self, frame)));
        self.core.active_callbacks -= 1;
        self.phase = previous;
        result.map_err(|payload| Error::ApplicationPanicked {
            phase: ContextPhase::Frame,
            message: crate::host::panic_message(payload),
        })
    }

    /// Creates an owned proxy without borrowing application or cleanup state.
    pub fn proxy(&self) -> Proxy<A::Message> {
        self.core.proxy()
    }

    /// Creates an owner-local recipient generation, mirrored in post admission.
    ///
    /// # Errors
    ///
    /// Returns an error if there's no room, or the host is closing.
    pub fn create_recipient(&mut self) -> Result<RecipientId, Error> {
        self.ordinary()?;
        let key = self.core.dispatch.recipients.reserve()?;
        let id = RecipientId {
            host: self.core.id,
            key,
        };
        self.core.transport.register(id)?;
        self.core.dispatch.recipients.insert(
            key,
            Recipient {
                alive: true,
                active: 0,
            },
        );
        Ok(id)
    }

    /// Inspects active callback retention; a fully reclaimed generation is stale.
    ///
    /// # Errors
    ///
    /// Returns an error if the recipient isn't current.
    pub fn recipient_state(&self, recipient: RecipientId) -> Result<RecipientState, Error> {
        let record = self.core.dispatch.recipient(self.core.id, recipient)?;
        Ok(RecipientState {
            retired: !record.alive,
            active_callbacks: record.active,
        })
    }

    /// Immediately removes future calls/admission while active callbacks finish.
    /// The result does not authorize destroying state an active callback uses.
    ///
    /// # Errors
    ///
    /// Returns an error if the recipient isn't current.
    pub fn retire_recipient(&mut self, recipient: RecipientId) -> Result<RecipientState, Error> {
        self.core.check_owner()?;
        let state = self
            .core
            .dispatch
            .retire_recipient(self.core.id, recipient)?;
        self.core.transport.retire(recipient);
        Ok(state)
    }

    /// Registers a function pointer in descending priority/newest equal order.
    /// Insertions before an active traversal's cursor wait for its next traversal.
    ///
    /// # Errors
    ///
    /// Returns an error if there's no room, or the priority isn't valid.
    pub fn subscribe(
        &mut self,
        recipient: RecipientId,
        filter: EventFilter,
        priority: Priority,
        handler: Handler<A>,
    ) -> Result<SubscriptionId, Error> {
        self.ordinary()?;
        self.core
            .dispatch
            .subscribe(self.core.id, recipient, filter, priority, handler)
    }

    /// Excludes future calls immediately; self-removal retains the active record.
    ///
    /// # Errors
    ///
    /// Returns an error if the subscription isn't current.
    pub fn unsubscribe(&mut self, subscription: SubscriptionId) -> Result<(), Error> {
        self.core.check_owner()?;
        self.core.dispatch.unsubscribe(self.core.id, subscription)
    }

    /// Dispatches a borrowed owner-local event. Each nested call starts at the
    /// head with its own insertion frontier; no registry borrow crosses a handler.
    ///
    /// # Errors
    ///
    /// Returns an error if dispatch can't start from here, or nests too deep.
    pub fn dispatch_local(
        &mut self,
        app: &mut A,
        event: &A::LocalEvent,
    ) -> Result<DispatchReport, Error> {
        self.dispatch_event(app, &Event::Local(event), None)
    }

    fn dispatch_event(
        &mut self,
        app: &mut A,
        event: &Event<'_, A::Message, A::LocalEvent>,
        target: Option<RecipientId>,
    ) -> Result<DispatchReport, Error> {
        self.ordinary()?;
        if self.core.dispatch.depth >= self.core.nesting_limit {
            return Err(Error::NestingLimit);
        }
        if let Some(target) = target {
            self.core.dispatch.live_recipient(self.core.id, target)?;
        }
        let marker = self.core.dispatch.begin()?;
        self.core.dispatch.depth += 1;
        let old_phase = self.phase;
        self.phase = ContextPhase::Event;
        let mut report = DispatchReport {
            callbacks: 0,
            propagation: Propagation::Continue,
        };
        let mut failure = None;
        while self.core.ordinary_open() {
            let Some(key) = self.core.dispatch.next(marker, target, event.filter()) else {
                break;
            };
            let (handler, recipient) = self.core.dispatch.claim(key);
            self.core.active_callbacks += 1;
            let result = catch_unwind(AssertUnwindSafe(|| handler(app, self, event)));
            self.core.active_callbacks -= 1;
            self.core.dispatch.release(key, recipient);
            match result {
                Ok(propagation) => {
                    report.callbacks += 1;
                    report.propagation = propagation;
                    if propagation == Propagation::Stop {
                        break;
                    }
                }
                Err(payload) => {
                    let error = Error::ApplicationPanicked {
                        phase: ContextPhase::Event,
                        message: crate::host::panic_message(payload),
                    };
                    self.core.fail(error.clone(), StopReason::CallbackPanicked);
                    failure = Some(error);
                    break;
                }
            }
        }
        self.phase = old_phase;
        self.core.dispatch.depth -= 1;
        self.core.dispatch.end(marker);
        if let Some(error) = failure {
            Err(error)
        } else {
            Ok(report)
        }
    }

    /// Temporarily defers input during application update/preparation work.
    /// Nested scopes restore the previous gate on return or unwinding; this does
    /// not recursively poll winit or start a frame.
    pub fn with_input_deferred<R>(
        &mut self,
        app: &mut A,
        work: impl FnOnce(&mut A, &mut Self) -> R,
    ) -> R {
        let previous = self.core.input.enabled;
        self.core.input.enabled = false;
        self.core.active_callbacks += 1;
        let result = catch_unwind(AssertUnwindSafe(|| work(app, self)));
        self.core.active_callbacks -= 1;
        self.core.input.enabled = previous;
        match result {
            Ok(value) => value,
            Err(payload) => resume_unwind(payload),
        }
    }

    /// Checks window generation before inspecting a named state layer.
    ///
    /// # Errors
    ///
    /// Returns an error if the window isn't current.
    pub fn input_state(
        &self,
        target: WindowTarget,
        layer: InputStateLayer,
    ) -> Result<Option<&InputState>, Error> {
        self.window_state(target)?;
        Ok(self.core.input.state(target, layer))
    }

    /// Delivers at most this many queued records in receipt order, temporarily
    /// allowing delivery even inside a deferred scope. Native input not yet
    /// dispatched by winit is unavailable to this explicit drain.
    ///
    /// # Errors
    ///
    /// Returns an error if input can't be drained from here.
    pub fn drain_input(&mut self, app: &mut A, budget: usize) -> Result<InputDrainReport, Error> {
        self.ordinary()?;
        if self.core.dispatch.depth >= self.core.nesting_limit {
            return Err(Error::NestingLimit);
        }
        // Guarantee marker storage before taking an owned record or advancing
        // delivered state. Nested calls cannot shrink the reserved capacity.
        self.core.dispatch.prepare_marker()?;
        let previous = self.core.input.enabled;
        self.core.input.enabled = true;
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut consumed = 0;
            while consumed < budget && self.core.ordinary_open() {
                let Some(mut record) = self.core.input.queue.pop_front() else {
                    break;
                };
                consumed += 1;
                if self.window_state(record.target).is_err() || !self.live_text(&record.event) {
                    continue;
                }
                let sample = self.core.clock.sample();
                if let Err(error) = self.core.input.deliver(&mut record, sample) {
                    self.core.fail(error.clone(), StopReason::BackendFailed);
                    return Err(error);
                }
                self.dispatch_event(app, &Event::Input(&record), None)?;
            }
            Ok(InputDrainReport {
                consumed,
                remaining: self.core.input.queue.len(),
            })
        }));
        self.core.input.enabled = previous;
        match result {
            Ok(result) => result,
            Err(payload) => resume_unwind(payload),
        }
    }

    #[cfg(test)]
    pub(crate) fn receive_input(
        &mut self,
        app: &mut A,
        target: WindowTarget,
        event: InputEvent,
        origin: InputOrigin,
        source_time: Option<Duration>,
    ) -> Result<(), Error> {
        self.receive_input_batch(
            app,
            vec![crate::backend::winit_input::NormalizedInput {
                target,
                event,
                origin,
                source_time,
                device: None,
            }],
        )
    }

    fn live_text(&self, event: &InputEvent) -> bool {
        match event {
            InputEvent::Text { session, .. } | InputEvent::Preedit { session, .. } => {
                self.core.text.is_live(*session)
            }
            _ => true,
        }
    }

    pub(crate) fn receive_input_batch(
        &mut self,
        app: &mut A,
        batch: Vec<crate::backend::winit_input::NormalizedInput>,
    ) -> Result<(), Error> {
        self.ordinary()?;
        for record in batch {
            self.require_live_window(record.target)?;
            if !self.live_text(&record.event) {
                continue;
            }
            if let InputEvent::Text { session, .. } | InputEvent::Preedit { session, .. } =
                &record.event
                && session.target() != record.target
            {
                return Err(Error::StaleIdentity);
            }
            let sample = self.core.clock.sample();
            if let Err(error) = self.core.input.receive(
                record.target,
                record.event,
                record.origin,
                sample,
                record.source_time,
                record.device,
            ) {
                self.core.fail(error.clone(), StopReason::BackendFailed);
                return Err(error);
            }
        }
        self.core.refresh_cursor_suppression();
        if self.core.input.enabled {
            self.drain_input(app, self.core.event_budget)?;
        }
        Ok(())
    }

    /// Schedules an owned local event on this host's checked raw clock.
    /// Rejection returns the payload; no `Send` or `'static` bound is added.
    ///
    /// # Errors
    ///
    /// Returns an error if the recipient isn't current, or there's no room.
    pub fn schedule_timer(
        &mut self,
        recipient: RecipientId,
        deadline: RawDeadline,
        event: A::LocalEvent,
    ) -> Result<TimerId, Rejected<A::LocalEvent>> {
        if let Err(error) = self
            .ordinary()
            .and_then(|()| self.core.dispatch.live_recipient(self.core.id, recipient))
        {
            return Err(Rejected::new(event, error));
        }
        self.core
            .timers
            .schedule(recipient, deadline, event)
            .map_err(|(event, error)| Rejected::new(event, error))
    }

    /// Removes an unclaimed timer and returns its owned payload. A claimed
    /// callback is retained; a stale token cannot cancel a reused slot.
    ///
    /// # Errors
    ///
    /// Returns an error if the timer isn't current.
    pub fn cancel_timer(&mut self, timer: TimerId) -> Result<TimerCancel<A::LocalEvent>, Error> {
        self.core.check_owner()?;
        self.core.timers.cancel(timer)
    }

    /// Samples raw time once, claims before callbacks, and rechecks the heap
    /// after each callback. Newly added due timers can run in this same pump.
    /// A nested explicit pump samples again and has its own checked budget/depth.
    ///
    /// # Errors
    ///
    /// Returns an error if timers can't be serviced from here.
    pub fn service_timers(&mut self, app: &mut A, budget: usize) -> Result<TimerPumpReport, Error> {
        self.ordinary()?;
        if self.core.dispatch.depth >= self.core.nesting_limit {
            return Err(Error::NestingLimit);
        }
        self.core.dispatch.prepare_marker()?;
        if self.core.timers.depth >= self.core.nesting_limit {
            return Err(Error::NestingLimit);
        }
        let sample = self.core.clock.sample();
        self.core.timers.depth += 1;
        let mut claimed = 0;
        let mut failure = None;
        while claimed < budget && self.core.ordinary_open() {
            let Some(timer) = self.core.timers.claim(sample) else {
                break;
            };
            claimed += 1;
            let result = if self
                .core
                .dispatch
                .live_recipient(self.core.id, timer.recipient)
                .is_ok()
            {
                self.dispatch_event(
                    app,
                    &Event::Timer {
                        id: timer.id,
                        deadline: timer.deadline,
                        sample,
                        event: &timer.payload,
                    },
                    Some(timer.recipient),
                )
            } else {
                Ok(DispatchReport {
                    callbacks: 0,
                    propagation: Propagation::Continue,
                })
            };
            self.core.timers.release(timer.id);
            self.core.dispose(timer.payload);
            if let Err(error) = result {
                failure = Some(error);
                break;
            }
        }
        self.core.timers.depth -= 1;
        if let Some(error) = failure {
            return Err(error);
        }
        Ok(TimerPumpReport {
            sample,
            claimed,
            due_remaining: self.core.timers.due(sample),
        })
    }

    /// Routes a detached intake frontier of at most `budget` posts. New arrivals
    /// wait for a later explicit drain; acceptance capacity includes active posts.
    /// Every accepted post receives a terminal receipt, including obsolete targets.
    ///
    /// # Errors
    ///
    /// Returns an error if posts can't be drained from here.
    pub fn drain_posts(&mut self, app: &mut A, budget: usize) -> Result<usize, Error> {
        self.ordinary()?;
        let mut batch = self
            .core
            .transport
            .detach(budget, &mut self.core.post_batches)?;
        let mut consumed = 0;
        let mut failure = None;
        for post in batch.by_ref() {
            consumed += 1;
            let result = if !self.core.ordinary_open() {
                post.receipt
                    .settle(PostOutcome::Discarded(PostDiscardReason::HostStopped));
                Ok(())
            } else if self
                .core
                .dispatch
                .live_recipient(self.core.id, post.recipient)
                .is_err()
            {
                post.receipt
                    .settle(PostOutcome::Discarded(PostDiscardReason::RecipientRetired));
                Ok(())
            } else {
                match self.dispatch_event(
                    app,
                    &Event::Posted {
                        receipt: &post.receipt,
                        message: &post.message,
                    },
                    Some(post.recipient),
                ) {
                    Ok(report) => {
                        post.receipt.settle(PostOutcome::Delivered {
                            callbacks: report.callbacks,
                        });
                        Ok(())
                    }
                    Err(error) => {
                        post.receipt.settle(PostOutcome::Faulted(error.clone()));
                        Err(error)
                    }
                }
            };
            self.core.dispose(post.message);
            self.core.transport.settled();
            if let Err(error) = result {
                failure.get_or_insert(error);
            }
        }
        // A nested service can finish closure before this drain returns.
        self.core
            .post_batches
            .complete(batch, self.core.state != HostState::Closed);
        if let Some(error) = failure {
            Err(error)
        } else {
            Ok(consumed)
        }
    }

    /// Starts/replaces this window's exact text target and supplies the native caret.
    /// Native candidate UI uses the backend's Windows IMM fallback.
    ///
    /// # Errors
    ///
    /// Returns an error if the window isn't current, or the caret is out of range.
    pub fn begin_text(
        &mut self,
        target: WindowTarget,
        caret: crate::TextCaret,
    ) -> Result<crate::TextSessionId, Error> {
        self.ordinary()?;
        self.require_live_window(target)?;
        let id = self.core.text.begin(target, caret)?;
        self.core.windows.get(target.id)?.native.text(Some(caret));
        Ok(id)
    }

    /// Updates an exact live session's physical caret rectangle.
    ///
    /// # Errors
    ///
    /// Returns an error if the session isn't current, or the caret is out of range.
    pub fn update_text(
        &mut self,
        session: crate::TextSessionId,
        caret: crate::TextCaret,
    ) -> Result<(), Error> {
        self.ordinary()?;
        self.require_live_window(session.target())?;
        self.core.text.update(session, caret)?;
        self.core
            .windows
            .get(session.target().id)?
            .native
            .text(Some(caret));
        Ok(())
    }

    /// Ends the exact session; already queued commits/preedit become stale.
    ///
    /// # Errors
    ///
    /// Returns an error if the session isn't current.
    pub fn end_text(&mut self, session: crate::TextSessionId) -> Result<(), Error> {
        self.core.check_owner()?;
        self.window_state(session.target())?;
        self.core.text.end(session)?;
        self.core
            .windows
            .get(session.target().id)?
            .native
            .text(None);
        Ok(())
    }

    /// Queries the caret of an exact live text session.
    ///
    /// # Errors
    ///
    /// Returns an error if the session isn't current.
    pub fn text_caret(&self, session: crate::TextSessionId) -> Result<crate::TextCaret, Error> {
        self.window_state(session.target())?;
        self.core.text.caret(session)
    }

    /// Creates a native resource from already prepared pixels, preserving input on rejection.
    ///
    /// # Errors
    ///
    /// Returns an error if the pixels aren't valid, or the native cursor can't be made.
    pub fn create_cursor(
        &mut self,
        input: crate::PreparedCursor,
    ) -> Result<crate::Cursor, Rejected<crate::PreparedCursor>> {
        if let Err(error) = self.ordinary() {
            return Err(Rejected::new(input, error));
        }
        self.operations.create_cursor(self.core.id, input)
    }

    /// Retains a window-local selection; the existing native cursor owner applies it
    /// only when this window controls the client/capture cursor.
    ///
    /// # Errors
    ///
    /// Returns an error if the window or the cursor isn't current.
    pub fn select_cursor(
        &mut self,
        target: WindowTarget,
        selection: crate::CursorSelection,
    ) -> Result<(), Rejected<crate::CursorSelection>> {
        let result = self
            .ordinary()
            .and_then(|()| self.require_live_window(target))
            .and_then(|_| {
                if let crate::CursorSelection::Custom(cursor) = &selection
                    && cursor.host != self.core.id
                {
                    return Err(Error::ForeignHost);
                }
                let record = self.core.windows.get_mut(target.id)?;
                record.native.cursor(&selection);
                record.cursor.selection = selection.clone();
                Ok(())
            });
        result.map_err(|error| Rejected::new(selection, error))
    }

    /// Requests visibility independently of input suppression. Native visibility
    /// requests do not certify actual visible cursor pixels or a hardware plane.
    ///
    /// # Errors
    ///
    /// Returns an error if the window isn't current.
    pub fn request_cursor_visibility(
        &mut self,
        target: WindowTarget,
        visible: bool,
    ) -> Result<(), Error> {
        self.ordinary()?;
        self.require_live_window(target)?;
        let record = self.core.windows.get_mut(target.id)?;
        record.cursor.requested_visible = visible;
        record
            .native
            .cursor_visible(visible && !record.cursor.input_suppressed);
        Ok(())
    }

    /// Inspects retained selection and independent visibility/suppression policy.
    ///
    /// # Errors
    ///
    /// Returns an error if the window isn't current.
    pub fn cursor_state(&self, target: WindowTarget) -> Result<&crate::CursorState, Error> {
        self.window_state(target)?;
        Ok(&self.core.windows.get(target.id)?.cursor)
    }

    /// Requests independently applied confinement and relative-motion selection.
    /// Partial failure retains the request and each last successful native result.
    /// Selecting relative delivery transfers the single raw recipient from any
    /// previous window; routing still requires receipt-time platform focus.
    ///
    /// # Errors
    ///
    /// Returns an error if the window isn't current, or the mode can't be applied.
    pub fn request_input_mode(
        &mut self,
        target: WindowTarget,
        mode: crate::InputMode,
    ) -> Result<(), Error> {
        self.ordinary()?;
        self.require_live_window(target)?;
        self.core.windows.get_mut(target.id)?.input_mode.requested = mode;
        let confine = self
            .core
            .windows
            .get(target.id)?
            .native
            .confine(mode.confine_pointer);
        self.core.windows.get_mut(target.id)?.confinement_uncertain = confine.is_err();
        if confine.is_ok() {
            self.core
                .windows
                .get_mut(target.id)?
                .input_mode
                .confirmed
                .confine_pointer = mode.confine_pointer;
        }
        let policy =
            if mode.relative_motion || self.core.relative_target.is_some_and(|old| old != target) {
                crate::RawInputPolicy::WhenFocused
            } else {
                crate::RawInputPolicy::Never
            };
        self.core.raw_input.requested = policy;
        let raw = self.operations.raw_input(policy);
        self.core.raw_input.failure = raw.as_ref().err().cloned();
        if raw.is_ok() {
            self.core.raw_input.confirmed = Some(policy);
            if mode.relative_motion {
                if let Some(old) = self.core.relative_target.replace(target)
                    && old != target
                {
                    let old = self.core.windows.get_mut(old.id)?;
                    old.input_mode.confirmed.relative_motion = false;
                }
            } else if self.core.relative_target == Some(target) {
                self.core.relative_target = None;
            }
            let record = self.core.windows.get_mut(target.id)?;
            record.input_mode.confirmed.relative_motion = mode.relative_motion;
        }
        self.core.refresh_cursor_suppression();
        let failure = confine.err().or_else(|| raw.err());
        self.core.windows.get_mut(target.id)?.input_mode.failure = failure.clone();
        failure.map_or(Ok(()), Err)
    }

    /// Queries this window's requests and last successful mode operations.
    ///
    /// # Errors
    ///
    /// Returns an error if the window isn't current.
    pub fn input_mode(&self, target: WindowTarget) -> Result<&crate::InputModeState, Error> {
        self.window_state(target)?;
        Ok(&self.core.windows.get(target.id)?.input_mode)
    }

    /// Queries the single existing backend registrar's last request/result.
    pub fn raw_input_state(&self) -> &crate::RawInputState {
        &self.core.raw_input
    }

    /// Creates a window synchronously while ordinary admission is open.
    ///
    /// Validation and `engine_host` storage reservation precede native construction.
    /// Success means a native window is retained, not renderer readiness. Every
    /// rejection returns the complete supplied spec. Stopping closes admission.
    ///
    /// # Errors
    ///
    /// Returns an error if windows can't be made now, or the native call fails.
    pub fn create_window(
        &mut self,
        spec: WindowSpec,
    ) -> Result<WindowTarget, Rejected<WindowSpec>> {
        match self.create_window_inner(&spec) {
            Ok(target) => Ok(target),
            Err(error) => Err(Rejected::new(spec, error)),
        }
    }

    fn create_window_inner(&mut self, spec: &WindowSpec) -> Result<WindowTarget, Error> {
        self.core.check_owner()?;
        if !self.core.ordinary_open() {
            return Err(Error::AdmissionClosed);
        }
        if matches!(
            self.phase,
            ContextPhase::Retirement | ContextPhase::Service(crate::ServicePoint::Retirement)
        ) {
            return Err(Error::InvalidContext(self.phase));
        }
        spec.validate()?;
        let id = self.core.windows.reserve()?;
        if self.core.native_windows.try_reserve(1).is_err() {
            self.core.windows.cancel_reservation(id);
            return Err(Error::AllocationFailed);
        }
        let native = match self.operations.create_window(spec) {
            Ok(native) => native,
            Err(error) => {
                self.core.windows.cancel_reservation(id);
                return Err(error);
            }
        };
        let generation = WindowGeneration::INITIAL;
        let target = WindowTarget { id, generation };
        self.core.native_windows.push((native.key(), target));
        // A failed observation drops native ownership, but its reserved identity
        // remains tied to this ledger entry until Destroyed acknowledges it.
        let observed = native.display_observed()?;
        let wake = self.core.native_wake.clone();
        let wake: std::sync::Arc<dyn Fn() + Send + Sync> = std::sync::Arc::new(move || {
            let _ = wake.post();
        });
        let access = match &native {
            crate::backend::NativeWindow::Winit(window) => {
                crate::window_access::WindowAnchor::new(target, window.clone(), wake)
            }
            #[cfg(test)]
            crate::backend::NativeWindow::Test(_) => {
                crate::window_access::WindowAnchor::simulated(target, wake)
            }
        };
        self.core.windows.insert(
            id,
            WindowRecord {
                generation,
                native,
                access,
                closing: false,
                display: crate::display::DisplayState::new(target, observed),
                input_mode: crate::InputModeState::default(),
                confinement_uncertain: false,
                cursor: crate::CursorState::default(),
            },
        );
        Ok(target)
    }

    /// Checks both host identity and expected native generation before query.
    /// Pending application retirement retains this generation's window.
    ///
    /// # Errors
    ///
    /// Returns an error if the host or the generation doesn't match.
    pub fn window_state(&self, target: WindowTarget) -> Result<WindowState, Error> {
        self.core.check_owner()?;
        if self
            .core
            .native_windows
            .iter()
            .any(|(_, current)| *current == target)
            && self.core.windows.get(target.id).is_err()
        {
            return Ok(WindowState::Retiring);
        }
        let record = self.core.windows.get(target.id)?;
        if target.generation != record.generation {
            return Err(Error::StaleIdentity);
        }
        if !self
            .core
            .native_windows
            .iter()
            .any(|(key, current)| *key == record.native.key() && *current == target)
        {
            return Err(Error::StaleIdentity);
        }
        Ok(if self.core.ordinary_open() && !record.closing {
            WindowState::Live
        } else {
            WindowState::Retiring
        })
    }

    /// Retains the actual current native generation through external final use.
    /// Native extraction is available only on this host's owner thread.
    ///
    /// # Errors
    ///
    /// Returns an error if the window isn't current.
    pub fn acquire_window(&mut self, target: WindowTarget) -> Result<crate::WindowAccess, Error> {
        self.ordinary()?;
        self.require_live_window(target)?;
        self.core.windows.get(target.id)?.access.acquire()
    }

    pub(crate) fn require_live_window(&self, target: WindowTarget) -> Result<(), Error> {
        if self.window_state(target)? == WindowState::Live {
            Ok(())
        } else {
            Err(Error::AdmissionClosed)
        }
    }
}
