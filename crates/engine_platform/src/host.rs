//! Running the application on `solapp`.

use std::error::Error;
use std::time::Duration;

use engine_core::window::NativeWindow;
use solapp::{
    SAApplication, SAContext, SADisplayRequest, SADisplayTransition, SADisplayTransitionState,
    SAError, SAEvent, SAEventFilter, SAFrame, SAFramePhase, SAHost, SAHostConfig, SAInputMode,
    SAMonitorSnapshot, SAPacingPolicy, SAPhysicalSize, SAPresentationStatus, SAPriority,
    SAPropagation, SAServiceBudget, SAServicePoint, SAServicePoints, SAServiceReport,
    SAServiceRequest, SAServiceSpec, SAStopProgress, SAWindowAccess, SAWindowSpec, SAWindowTarget,
};

use crate::error::PlatformError;
use crate::input::{InputEvent, translate};

/// An application error reported back through [`run`].
pub type AppError = Box<dyn Error + Send + Sync>;

/// How often the host rechecks a service in any case. Its own requested time
/// normally comes first.
const SERVICE_FALLBACK: Duration = Duration::from_millis(100);

/// How the window sits on the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowMode {
    /// Fills its monitor, with no frame, and stays on the desktop.
    Borderless,
    /// An ordinary window of the configured size.
    Windowed,
}

/// The window to open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowConfig {
    /// The title bar text.
    pub title: String,
    /// Client width in physical pixels.
    pub width: u32,
    /// Client height in physical pixels.
    pub height: u32,
    /// How it sits on the screen.
    pub mode: WindowMode,
    /// The most frames per second, or `None` when a renderer paces itself.
    pub frame_cap: Option<u32>,
    /// Frames per second in reduced mode, while the window is minimized.
    pub reduced_frame_rate: u32,
}

/// A lease on the window. The window stays valid while this exists, so a
/// renderer must be dropped before it.
#[derive(Debug)]
pub struct Window {
    /// The host's lease.
    access: SAWindowAccess,
}

impl Window {
    /// The window's operating-system identity.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError::Host`] if the window is gone, or isn't a kind
    /// the engine can draw into.
    pub fn native(&self) -> Result<NativeWindow, PlatformError> {
        let host = |error: SAError| PlatformError::Host(error.to_string());
        let native = self.access.native_ref().map_err(host)?;
        let hwnd = native.hwnd().map_err(host)?.get();
        let hinstance = native
            .hinstance()
            .map_err(host)?
            .map_or(0, |value| value.get());
        Ok(NativeWindow::Win32 { hwnd, hinstance })
    }
}

/// What the application learns at each frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// Time since the previous frame, zero on the first.
    pub delta: Duration,
    /// Time since the host started, on the same clock as
    /// [`PlatformApp::service`].
    pub now: Duration,
    /// Whether the window is minimized, so a small, cheap frame is enough.
    pub reduced: bool,
}

/// Whether to keep running after a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameOutcome {
    /// Request another frame.
    Continue,
    /// Close the window and end [`run`].
    Stop,
}

/// What the application wants after a service visit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceOutcome {
    /// Visit again after this long.
    After(Duration),
    /// Close the window and end [`run`].
    Stop,
}

/// The application the platform runs.
pub trait PlatformApp {
    /// Called once the window exists. An error ends the run, and
    /// [`PlatformApp::stopping`] is still called.
    ///
    /// # Errors
    ///
    /// Any error the application cannot start with.
    fn started(&mut self, window: Window) -> Result<(), AppError>;

    /// Called on the simulation's schedule, including while the window is
    /// minimized. `now` is the time since the host started.
    fn service(&mut self, now: Duration) -> ServiceOutcome;

    /// Called for each input event as it arrives, with no regard for frames.
    /// `time` is when the host received it, since the host started.
    fn input(&mut self, event: &InputEvent, time: Duration);

    /// Called for each frame. While the window is minimized, frames slow to
    /// [`WindowConfig::reduced_frame_rate`] and [`Frame::reduced`] is set.
    fn frame(&mut self, frame: &Frame) -> FrameOutcome;

    /// Whether the mouse should be captured for raw motion. Checked at each
    /// service visit.
    fn mouse_capture(&self) -> bool;

    /// Called with something the user should know, which isn't an error
    /// that stops the run.
    fn notice(&mut self, message: String);

    /// Called when the run ends.
    fn stopping(&mut self);
}

/// Runs `app` in a window until it stops or the window is closed.
///
/// Must be called on the process's main thread, once per process.
///
/// # Errors
///
/// Returns [`PlatformError::Host`] when the host cannot start or fails, and
/// [`PlatformError::Startup`] when [`PlatformApp::started`] fails.
pub fn run<A: PlatformApp>(config: WindowConfig, app: &mut A) -> Result<(), PlatformError> {
    let mut host = SAHost::new(SAHostConfig::default())
        .map_err(|rejected| PlatformError::Host(rejected.into_parts().1.to_string()))?;
    let mut adapter = Adapter {
        app,
        config,
        target: None,
        minimized: false,
        captured: false,
        startup_error: None,
    };
    let result = host.run(&mut adapter);
    if let Some(message) = adapter.startup_error.take() {
        return Err(PlatformError::Startup(message));
    }
    result
        .map(|_report| ())
        .map_err(|error| PlatformError::Host(error.to_string()))
}

/// The `solapp` application wrapping a [`PlatformApp`].
struct Adapter<'app, A> {
    /// The wrapped application.
    app: &'app mut A,
    /// The window to open.
    config: WindowConfig,
    /// The window, once created.
    target: Option<SAWindowTarget>,
    /// Whether the window is minimized, so frames are in reduced mode.
    minimized: bool,
    /// Whether the mouse is captured, or was last asked to be.
    captured: bool,
    /// The application's startup error, kept to report it by kind.
    startup_error: Option<String>,
}

/// Passes input straight to the application.
fn on_input<A: PlatformApp>(
    adapter: &mut Adapter<'_, A>,
    _cx: &mut SAContext<'_, Adapter<'_, A>>,
    event: &SAEvent<'_, (), ()>,
) -> SAPropagation {
    if let SAEvent::Input(record) = event
        && Some(record.target) == adapter.target
        && let Some(input) = translate(&record.event)
    {
        adapter.app.input(&input, record.stamp.receipt.elapsed());
    }
    SAPropagation::Continue
}

impl<A: PlatformApp> Adapter<'_, A> {
    /// Creates the window, subscribes to input, and starts frames and the
    /// simulation service.
    fn start(&mut self, cx: &mut SAContext<'_, Self>) -> Result<(), SAError> {
        let spec = SAWindowSpec {
            title: self.config.title.clone(),
            size: SAPhysicalSize {
                width: self.config.width,
                height: self.config.height,
            },
            visible: true,
        };
        let target = cx
            .create_window(spec)
            .map_err(|rejected| rejected.into_parts().1)?;
        self.target = Some(target);
        let recipient = cx.create_recipient()?;
        cx.subscribe(
            recipient,
            SAEventFilter::Input,
            SAPriority::new(0.0)?,
            on_input::<A>,
        )?;
        cx.set_pacing(Some(target), self.pacing())?;
        cx.request_frame()?;
        if self.config.mode == WindowMode::Borderless {
            Self::enter_borderless(cx, target)?;
        }
        let _service = cx.register_service(SAServiceSpec {
            points: SAServicePoints::only(SAServicePoint::PreUpdate)
                .union(SAServicePoints::ORDINARY),
            budget: SAServiceBudget { records: 1 },
            fallback_interval: SERVICE_FALLBACK,
        })?;
        Ok(())
    }

    /// Asks for borderless fullscreen on the monitor the window opened on.
    fn enter_borderless(
        cx: &mut SAContext<'_, Self>,
        target: SAWindowTarget,
    ) -> Result<(), SAError> {
        let monitors = cx.monitors(target)?;
        let position = cx.display_observed(target)?.geometry.position;
        let holds_window = |monitor: &&SAMonitorSnapshot| {
            position.is_some_and(|at| {
                let right = i64::from(monitor.position.x) + i64::from(monitor.size.width);
                let bottom = i64::from(monitor.position.y) + i64::from(monitor.size.height);
                at.x >= monitor.position.x
                    && at.y >= monitor.position.y
                    && i64::from(at.x) < right
                    && i64::from(at.y) < bottom
            })
        };
        let monitor = monitors
            .iter()
            .find(holds_window)
            .or_else(|| monitors.first());
        if let Some(monitor) = monitor {
            let request = SADisplayRequest::Borderless {
                monitor: monitor.id.clone(),
            };
            let _receipt = cx
                .request_display(target, request)
                .map_err(|rejected| rejected.into_parts().1)?;
        }
        Ok(())
    }

    /// The pacing for a window that can be seen.
    fn pacing(&self) -> SAPacingPolicy {
        match self.config.frame_cap {
            Some(rate) => SAPacingPolicy::Forever {
                foreground_rate: rate,
                background_rate: rate,
                foreground: true,
                phase: SAFramePhase::Ordinary,
            },
            None => SAPacingPolicy::RendererManaged,
        }
    }

    /// The pacing for a minimized window.
    fn reduced_pacing(&self) -> SAPacingPolicy {
        let rate = self.config.reduced_frame_rate;
        SAPacingPolicy::Forever {
            foreground_rate: rate,
            background_rate: rate,
            foreground: true,
            phase: SAFramePhase::Ordinary,
        }
    }

    /// Enters reduced mode while minimized, and leaves it on restore.
    fn follow_minimized(&mut self, cx: &mut SAContext<'_, Self>, target: SAWindowTarget) {
        let Ok(observed) = cx.display_observed(target) else {
            return;
        };
        let size = observed.geometry.size;
        let minimized =
            observed.geometry.minimized == Some(true) || size.width == 0 || size.height == 0;
        if minimized != self.minimized {
            self.minimized = minimized;
            let pacing = if minimized {
                self.reduced_pacing()
            } else {
                self.pacing()
            };
            let result = cx
                .set_pacing(Some(target), pacing)
                .and_then(|()| cx.request_frame());
            if let Err(error) = result {
                self.app.notice(format!("frame pacing: {error}"));
            }
        }
    }

    /// Captures or releases the mouse if the application changed its mind.
    fn follow_capture(&mut self, cx: &mut SAContext<'_, Self>, target: SAWindowTarget) {
        let wanted = self.app.mouse_capture();
        if wanted == self.captured {
            return;
        }
        self.captured = wanted;
        let mode = SAInputMode {
            relative_motion: wanted,
            confine_pointer: wanted,
        };
        let result = cx
            .request_input_mode(target, mode)
            .and_then(|()| cx.request_cursor_visibility(target, !wanted));
        if let Err(error) = result {
            self.app.notice(format!("mouse capture: {error}"));
        }
    }
}

impl<A: PlatformApp> SAApplication for Adapter<'_, A> {
    type Message = ();
    type LocalEvent = ();

    fn started(&mut self, cx: &mut SAContext<'_, Self>) -> Result<(), SAError> {
        self.start(cx)?;
        let target = self.target.ok_or(SAError::application("no window"))?;
        let window = Window {
            access: cx.acquire_window(target)?,
        };
        self.app.started(window).map_err(|error| {
            let message = error.to_string();
            self.startup_error = Some(message.clone());
            SAError::application(message)
        })
    }

    fn service(
        &mut self,
        cx: &mut SAContext<'_, Self>,
        request: SAServiceRequest,
    ) -> SAServiceReport {
        if let Some(target) = self.target {
            self.follow_minimized(cx, target);
            self.follow_capture(cx, target);
        }
        match self.app.service(request.raw.elapsed()) {
            ServiceOutcome::After(delay) => match request.raw.checked_add(delay) {
                Ok(deadline) => SAServiceReport::WaitUntil(deadline),
                Err(_overflow) => SAServiceReport::Quiescent,
            },
            ServiceOutcome::Stop => {
                cx.request_stop();
                SAServiceReport::Quiescent
            }
        }
    }

    fn frame(&mut self, cx: &mut SAContext<'_, Self>, frame: SAFrame) {
        let frame = Frame {
            delta: frame.delta,
            now: frame.raw.elapsed(),
            reduced: self.minimized,
        };
        let wants_more = self.app.frame(&frame) == FrameOutcome::Continue;
        if !wants_more || cx.request_frame().is_err() {
            cx.request_stop();
        }
    }

    fn display_transition(
        &mut self,
        cx: &mut SAContext<'_, Self>,
        transition: &SADisplayTransition,
    ) {
        match &transition.state {
            // The renderer rebuilds from the resize that follows, so nothing
            // needs holding back here.
            SADisplayTransitionState::AwaitingPresentation => {
                let result =
                    cx.report_presentation(transition.receipt, SAPresentationStatus::Ready);
                if let Err(error) = result {
                    self.app.notice(format!("display: {error}"));
                }
            }
            SADisplayTransitionState::Failed(error) => {
                self.app.notice(format!("display: {error}"));
            }
            _ => {}
        }
    }

    fn stopping(&mut self, _cx: &mut SAContext<'_, Self>) -> SAStopProgress {
        self.app.stopping();
        SAStopProgress::Settled
    }
}
