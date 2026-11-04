//! Running the application on `solapp`.

use std::error::Error;
use std::time::Duration;

use solapp::{
    SAApplication, SAContext, SAError, SAEvent, SAEventFilter, SAFrame, SAFramePhase, SAHost,
    SAHostConfig, SAPacingPolicy, SAPhysicalSize, SAPriority, SAPropagation, SAServiceBudget,
    SAServicePoint, SAServicePoints, SAServiceReport, SAServiceRequest, SAServiceSpec,
    SAStopProgress, SAWindowSpec, SAWindowTarget,
};

use crate::error::PlatformError;
use crate::input::{InputEvent, translate};

/// An application error reported back through [`run`].
pub type AppError = Box<dyn Error + Send + Sync>;

/// How often the host rechecks a service in any case. Its own requested time
/// normally comes first.
const SERVICE_FALLBACK: Duration = Duration::from_millis(100);

/// The window to open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowConfig {
    /// The title bar text.
    pub title: String,
    /// Client width in physical pixels.
    pub width: u32,
    /// Client height in physical pixels.
    pub height: u32,
    /// The most frames per second, or `None` when a renderer paces itself.
    pub frame_cap: Option<u32>,
}

/// What the application learns at each frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// Input received since the previous frame, in order.
    pub input: Vec<InputEvent>,
    /// Time since the previous frame, zero on the first.
    pub delta: Duration,
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
    fn started(&mut self) -> Result<(), AppError>;

    /// Called on the simulation's schedule, including while the window is
    /// minimized. `now` is the time since the host started.
    fn service(&mut self, now: Duration) -> ServiceOutcome;

    /// Called for each frame.
    fn frame(&mut self, frame: &Frame) -> FrameOutcome;

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
        input: Vec::new(),
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
    /// Input received since the last frame.
    input: Vec<InputEvent>,
    /// The application's startup error, kept to report it by kind.
    startup_error: Option<String>,
}

/// Queues input for the next frame.
fn on_input<A: PlatformApp>(
    adapter: &mut Adapter<'_, A>,
    _cx: &mut SAContext<'_, Adapter<'_, A>>,
    event: &SAEvent<'_, (), ()>,
) -> SAPropagation {
    if let SAEvent::Input(record) = event
        && Some(record.target) == adapter.target
    {
        adapter.input.extend(translate(&record.event));
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
        let pacing = match self.config.frame_cap {
            Some(rate) => SAPacingPolicy::Forever {
                foreground_rate: rate,
                background_rate: rate,
                foreground: true,
                phase: SAFramePhase::Ordinary,
            },
            None => SAPacingPolicy::RendererManaged,
        };
        cx.set_pacing(Some(target), pacing)?;
        cx.request_frame()?;
        let _service = cx.register_service(SAServiceSpec {
            points: SAServicePoints::only(SAServicePoint::PreUpdate)
                .union(SAServicePoints::ORDINARY),
            budget: SAServiceBudget { records: 1 },
            fallback_interval: SERVICE_FALLBACK,
        })?;
        Ok(())
    }
}

impl<A: PlatformApp> SAApplication for Adapter<'_, A> {
    type Message = ();
    type LocalEvent = ();

    fn started(&mut self, cx: &mut SAContext<'_, Self>) -> Result<(), SAError> {
        self.start(cx)?;
        self.app.started().map_err(|error| {
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
            input: std::mem::take(&mut self.input),
            delta: frame.delta,
        };
        let next = match self.app.frame(&frame) {
            FrameOutcome::Continue => cx.request_frame(),
            FrameOutcome::Stop => Err(SAError::application("stop requested")),
        };
        if next.is_err() {
            cx.request_stop();
        }
    }

    fn stopping(&mut self, _cx: &mut SAContext<'_, Self>) -> SAStopProgress {
        self.app.stopping();
        SAStopProgress::Settled
    }
}
