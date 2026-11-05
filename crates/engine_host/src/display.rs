//! Driver-derived display choices and serialized native/presentation transitions.
mod context;

use std::fmt;

use winit::monitor::{MonitorHandle, VideoModeHandle};
use winit::window::{Fullscreen, Window};

use crate::{Error, HostId, IdentityKind, PhysicalSize, WindowTarget};

/// Position in physical virtual-desktop pixels; negative coordinates are valid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScreenPosition {
    /// Horizontal screen coordinate.
    pub x: i32,
    /// Vertical screen coordinate.
    pub y: i32,
}

/// A validated, positive finite native scale factor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScaleFactor(f64);

impl ScaleFactor {
    /// Validates a scale before exposing it as geometry.
    pub fn new(value: f64) -> Result<Self, Error> {
        if !value.is_finite() || value <= 0.0 {
            return Err(Error::InvalidInput("scale must be positive and finite"));
        }
        Ok(Self(value))
    }

    /// The observed physical-to-logical scale.
    pub fn get(self) -> f64 {
        self.0
    }
}

/// Saved windowed geometry, independent of a fullscreen presentation revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowedPlacement {
    /// Physical outer-window position on the virtual desktop.
    pub position: ScreenPosition,
    /// Requested physical client size, subject to native adjustments.
    pub size: PhysicalSize,
    /// Whether the windowed state should be maximized.
    pub maximized: bool,
}

impl WindowedPlacement {
    pub(crate) fn validate(self) -> Result<(), Error> {
        if self.size.width == 0
            || self.size.height == 0
            || self.size.width > i32::MAX as u32
            || self.size.height > i32::MAX as u32
        {
            return Err(Error::InvalidInput(
                "windowed client size outside native range",
            ));
        }
        Ok(())
    }

    /// Keeps this placement on a currently observed monitor. If its old monitor
    /// vanished, moves it inside the nearest snapshot without changing client size.
    /// Native decoration sizes and subsequent native adjustments remain backend-owned.
    pub fn adjusted_for_monitors(self, monitors: &[MonitorSnapshot]) -> Result<Self, Error> {
        self.validate()?;
        let intersects = |monitor: &MonitorSnapshot| {
            let left = i64::from(self.position.x);
            let top = i64::from(self.position.y);
            let right = left + i64::from(self.size.width);
            let bottom = top + i64::from(self.size.height);
            left < i64::from(monitor.position.x) + i64::from(monitor.size.width)
                && top < i64::from(monitor.position.y) + i64::from(monitor.size.height)
                && right > i64::from(monitor.position.x)
                && bottom > i64::from(monitor.position.y)
        };
        if monitors.iter().any(intersects) {
            return Ok(self);
        }
        let nearest = monitors
            .iter()
            .filter(|monitor| monitor.size.width != 0 && monitor.size.height != 0)
            .min_by_key(|monitor| {
                let x = i128::from(self.position.x) - i128::from(monitor.position.x);
                let y = i128::from(self.position.y) - i128::from(monitor.position.y);
                x * x + y * y
            })
            .ok_or(Error::StaleIdentity)?;
        let x = i64::from(nearest.position.x)
            + i64::from(nearest.size.width.saturating_sub(self.size.width));
        let y = i64::from(nearest.position.y)
            + i64::from(nearest.size.height.saturating_sub(self.size.height));
        Ok(Self {
            position: ScreenPosition {
                x: self
                    .position
                    .x
                    .clamp(nearest.position.x, i32::try_from(x).unwrap_or(i32::MAX)),
                y: self
                    .position
                    .y
                    .clamp(nearest.position.y, i32::try_from(y).unwrap_or(i32::MAX)),
            },
            ..self
        })
    }
}

#[derive(Clone, Eq, PartialEq)]
enum MonitorSelection {
    Native(MonitorHandle),
    #[cfg(test)]
    Simulated(u64),
}

impl MonitorSelection {
    fn native(&self) -> Result<&MonitorHandle, Error> {
        match self {
            Self::Native(monitor) => Ok(monitor),
            #[cfg(test)]
            Self::Simulated(_) => Err(Error::StaleIdentity),
        }
    }
}

/// Host-scoped opaque monitor selection issued by the native driver.
/// Monitor identity may disappear; every apply revalidates its current presence.
#[derive(Clone, Eq, PartialEq)]
pub struct MonitorId {
    host: HostId,
    selection: MonitorSelection,
}

impl fmt::Debug for MonitorId {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output
            .debug_struct("MonitorId")
            .field("host", &self.host)
            .finish_non_exhaustive()
    }
}

/// Opaque driver-derived video mode; callers cannot manufacture native modes.
#[derive(Clone, Eq, PartialEq)]
pub struct VideoMode {
    monitor: MonitorId,
    native: Option<VideoModeHandle>,
    size: PhysicalSize,
    bit_depth: u16,
    refresh_millihertz: u32,
}

impl VideoMode {
    /// The monitor from which this exact mode was issued.
    pub fn monitor(&self) -> &MonitorId {
        &self.monitor
    }

    /// Driver-reported physical pixel dimensions.
    pub fn size(&self) -> PhysicalSize {
        self.size
    }

    /// Driver-reported pixel bit depth.
    pub fn bit_depth(&self) -> u16 {
        self.bit_depth
    }

    /// Driver-reported refresh rate in millihertz, without rounding to integer Hz.
    pub fn refresh_rate_millihertz(&self) -> u32 {
        self.refresh_millihertz
    }
}

impl fmt::Debug for VideoMode {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output
            .debug_struct("VideoMode")
            .field("monitor", &self.monitor)
            .field("size", &self.size)
            .field("bit_depth", &self.bit_depth)
            .field("refresh_millihertz", &self.refresh_millihertz)
            .finish_non_exhaustive()
    }
}

/// One native monitor snapshot. Its choices must be revalidated when applied.
#[derive(Clone, Debug, PartialEq)]
pub struct MonitorSnapshot {
    /// Opaque current monitor choice.
    pub id: MonitorId,
    /// Optional native display name.
    pub name: Option<String>,
    /// Physical virtual-desktop origin.
    pub position: ScreenPosition,
    /// Observed physical monitor size.
    pub size: PhysicalSize,
    /// Validated native DPI scale.
    pub scale: ScaleFactor,
    /// Current driver-derived exclusive choices.
    pub video_modes: Vec<VideoMode>,
}

/// An admitted mode request; Keep/Revert and settings persistence remain caller-owned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisplayRequest {
    /// Return to explicit placement, or the saved pre-fullscreen placement.
    Windowed {
        /// None requests restoration of the saved native windowed geometry.
        placement: Option<WindowedPlacement>,
    },
    /// Borderless fullscreen on an explicitly selected current monitor.
    Borderless {
        /// Driver-derived monitor choice.
        monitor: MonitorId,
    },
    /// Exclusive fullscreen using an explicitly selected driver mode.
    /// Winit's accepted exceptional exclusive failure may panic.
    Exclusive {
        /// Driver-derived mode, revalidated immediately before native mutation.
        mode: VideoMode,
    },
}

/// Mode category reported by the existing native backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayMode {
    /// Native windowed state.
    Windowed,
    /// Backend borderless fullscreen state.
    Borderless,
    /// Backend exclusive fullscreen state.
    Exclusive,
}

impl DisplayRequest {
    /// The requested mode category; this does not certify native application.
    pub fn mode(&self) -> DisplayMode {
        match self {
            Self::Windowed { .. } => DisplayMode::Windowed,
            Self::Borderless { .. } => DisplayMode::Borderless,
            Self::Exclusive { .. } => DisplayMode::Exclusive,
        }
    }

    fn validate(&self, host: HostId) -> Result<(), Error> {
        match self {
            Self::Windowed {
                placement: Some(placement),
            } => placement.validate(),
            Self::Windowed { placement: None } => Ok(()),
            Self::Borderless { monitor } if monitor.host == host => Ok(()),
            Self::Exclusive { mode } if mode.monitor.host == host => Ok(()),
            _ => Err(Error::ForeignHost),
        }
    }

    pub(crate) fn validate_choices(
        &self,
        host: HostId,
        monitors: &[MonitorSnapshot],
    ) -> Result<(), Error> {
        self.validate(host)?;
        match self {
            Self::Windowed { .. } => Ok(()),
            Self::Borderless { monitor } => monitors
                .iter()
                .any(|current| current.id == *monitor)
                .then_some(())
                .ok_or(Error::StaleIdentity),
            Self::Exclusive { mode } => monitors
                .iter()
                .find(|current| current.id == mode.monitor)
                .and_then(|current| current.video_modes.iter().find(|current| *current == mode))
                .map(|_| ())
                .ok_or(Error::StaleIdentity),
        }
    }

    pub(crate) fn native_fullscreen(&self, window: &Window) -> Result<Option<Fullscreen>, Error> {
        match self {
            Self::Windowed { .. } => Ok(None),
            Self::Borderless { monitor } => {
                let selected = monitor.selection.native()?;
                let current = window
                    .available_monitors()
                    .find(|monitor| monitor == selected)
                    .ok_or(Error::StaleIdentity)?;
                Ok(Some(Fullscreen::Borderless(Some(current))))
            }
            Self::Exclusive { mode } => {
                let selected = mode.monitor.selection.native()?;
                let monitor = window
                    .available_monitors()
                    .find(|monitor| monitor == selected)
                    .ok_or(Error::StaleIdentity)?;
                let selected_mode = mode.native.as_ref().ok_or(Error::StaleIdentity)?;
                let current = monitor
                    .video_modes()
                    .find(|mode| mode == selected_mode)
                    .ok_or(Error::StaleIdentity)?;
                Ok(Some(Fullscreen::Exclusive(current)))
            }
        }
    }
}

/// Queried native geometry, including minimized zero-size client areas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisplayGeometry {
    /// Physical outer position, absent if the native query was unavailable.
    pub position: Option<ScreenPosition>,
    /// Observed physical client size; zero is valid while minimized.
    pub size: PhysicalSize,
    /// Validated observed native scale.
    pub scale: ScaleFactor,
    /// Backend-observed minimized state, if available.
    pub minimized: Option<bool>,
    /// Backend-observed maximized state.
    pub maximized: bool,
}

/// Backend-observed state, separate from requested settings and SR readiness.
/// `backend_mode` comes from winit's stored fullscreen state. It does not
/// independently certify hardware scanout, exclusive acceptance, or GPU completion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisplayObserved {
    /// Existing backend's reported fullscreen category.
    pub backend_mode: DisplayMode,
    /// Independently queried native geometry/DPI snapshot.
    pub geometry: DisplayGeometry,
}

impl DisplayObserved {
    pub(crate) fn query(window: &Window) -> Result<Self, Error> {
        let size = window.inner_size();
        let position = window.outer_position().ok().map(|position| ScreenPosition {
            x: position.x,
            y: position.y,
        });
        Ok(Self {
            backend_mode: match window.fullscreen() {
                None => DisplayMode::Windowed,
                Some(Fullscreen::Borderless(_)) => DisplayMode::Borderless,
                Some(Fullscreen::Exclusive(_)) => DisplayMode::Exclusive,
            },
            geometry: DisplayGeometry {
                position,
                size: PhysicalSize {
                    width: size.width,
                    height: size.height,
                },
                scale: ScaleFactor::new(window.scale_factor())?,
                minimized: window.is_minimized(),
                maximized: window.is_maximized(),
            },
        })
    }

    fn windowed_placement(self) -> Option<WindowedPlacement> {
        let placement = WindowedPlacement {
            position: self.geometry.position?,
            size: self.geometry.size,
            maximized: self.geometry.maximized,
        };
        placement.validate().ok()?;
        Some(placement)
    }
}

pub(crate) fn monitor_snapshots(
    host: HostId,
    window: &Window,
) -> Result<Vec<MonitorSnapshot>, Error> {
    let mut monitors = Vec::new();
    for native in window.available_monitors() {
        monitors
            .try_reserve(1)
            .map_err(|_| Error::AllocationFailed)?;
        let id = MonitorId {
            host,
            selection: MonitorSelection::Native(native.clone()),
        };
        let mut modes = Vec::new();
        for mode in native.video_modes() {
            modes.try_reserve(1).map_err(|_| Error::AllocationFailed)?;
            let size = mode.size();
            modes.push(VideoMode {
                monitor: id.clone(),
                size: PhysicalSize {
                    width: size.width,
                    height: size.height,
                },
                bit_depth: mode.bit_depth(),
                refresh_millihertz: mode.refresh_rate_millihertz(),
                native: Some(mode),
            });
        }
        let position = native.position();
        let size = native.size();
        monitors.push(MonitorSnapshot {
            id,
            name: native.name(),
            position: ScreenPosition {
                x: position.x,
                y: position.y,
            },
            size: PhysicalSize {
                width: size.width,
                height: size.height,
            },
            scale: ScaleFactor::new(native.scale_factor())?,
            video_modes: modes,
        });
    }
    Ok(monitors)
}

#[cfg(test)]
pub(crate) fn simulated_monitors(host: HostId) -> Vec<MonitorSnapshot> {
    let id = MonitorId {
        host,
        selection: MonitorSelection::Simulated(1),
    };
    vec![MonitorSnapshot {
        id: id.clone(),
        name: Some(String::from("simulated native display")),
        position: ScreenPosition { x: 0, y: 0 },
        size: PhysicalSize {
            width: 1920,
            height: 1080,
        },
        scale: ScaleFactor(1.0),
        video_modes: vec![VideoMode {
            monitor: id,
            native: None,
            size: PhysicalSize {
                width: 1920,
                height: 1080,
            },
            bit_depth: 32,
            refresh_millihertz: 59940,
        }],
    }]
}

/// Exact target and serial of one admitted display transition.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DisplayTransitionId {
    target: WindowTarget,
    serial: u64,
}

impl DisplayTransitionId {
    /// The exact native generation to which this transition belongs.
    pub fn target(self) -> WindowTarget {
        self.target
    }
}

/// Renderer revision, independent of native generation and transition identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PresentationRevision(u64);

/// Admission receipt; neither native application nor rendering readiness is implied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisplayReceipt {
    /// Exact admitted transition.
    pub transition: DisplayTransitionId,
    /// Exact renderer revision required by the acknowledgment.
    pub presentation_revision: PresentationRevision,
}

/// Progress of an admitted transition. Readiness is not user confirmation or persistence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisplayTransitionState {
    /// Pending native application; this request alone may be superseded.
    Requested,
    /// Native application has started and current obligations must settle.
    Applying,
    /// Backend application returned; the exact SR revision awaits acknowledgment.
    AwaitingPresentation,
    /// Backend mode was observed and SR/application reported exact revision ready.
    /// This acknowledgment is an external contract, not `engine_host`'s proof of GPU completion.
    Ready,
    /// Typed native or renderer failure, retaining its diagnostic.
    Failed(Error),
    /// Replaced or retired before application began.
    Superseded,
}

/// Owned transition status suitable for owner delivery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayTransition {
    /// Exact transition and renderer revision.
    pub receipt: DisplayReceipt,
    /// Owned application-supplied requested settings.
    pub request: DisplayRequest,
    /// Current transition progress.
    pub state: DisplayTransitionState,
}

/// Application/SR report for an exact renderer revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresentationStatus {
    /// SR has made this revision ready while retaining actual required window access.
    Ready,
    /// SR failed this revision; application owns any revert policy.
    Failed(Error),
}

pub(crate) struct DisplayAdmission {
    pub(crate) receipt: DisplayReceipt,
    pub(crate) superseded: Option<DisplayTransition>,
}

pub(crate) struct DisplayApply {
    pub(crate) transition: DisplayTransition,
    // Implicit maximized restoration must use winit's saved WINDOWPLACEMENT;
    // observed maximized geometry is not the hidden normal restore rectangle.
    pub(crate) windowed_placement: Option<WindowedPlacement>,
}

pub(crate) struct DisplayState {
    target: WindowTarget,
    next_serial: u64,
    next_revision: u64,
    accepting: bool,
    observed: DisplayObserved,
    saved_windowed: Option<WindowedPlacement>,
    pending: Option<DisplayTransition>,
    active: Option<DisplayTransition>,
    settled: Option<DisplayTransition>,
}

impl DisplayState {
    pub(crate) fn new(target: WindowTarget, observed: DisplayObserved) -> Self {
        Self {
            target,
            next_serial: 1,
            next_revision: 1,
            accepting: true,
            observed,
            saved_windowed: None,
            pending: None,
            active: None,
            settled: None,
        }
    }

    pub(crate) fn request(&mut self, request: DisplayRequest) -> Result<DisplayAdmission, Error> {
        if !self.accepting {
            return Err(Error::AdmissionClosed);
        }
        request.validate(self.target.id.host())?;
        let next_serial = self
            .next_serial
            .checked_add(1)
            .ok_or(Error::IdentityExhausted(IdentityKind::DisplayTransition))?;
        let next_revision = self
            .next_revision
            .checked_add(1)
            .ok_or(Error::IdentityExhausted(IdentityKind::Presentation))?;
        let receipt = DisplayReceipt {
            transition: DisplayTransitionId {
                target: self.target,
                serial: self.next_serial,
            },
            presentation_revision: PresentationRevision(self.next_revision),
        };
        let mut superseded = self.pending.replace(DisplayTransition {
            receipt,
            request,
            state: DisplayTransitionState::Requested,
        });
        if let Some(previous) = &mut superseded {
            previous.state = DisplayTransitionState::Superseded;
        }
        self.next_serial = next_serial;
        self.next_revision = next_revision;
        Ok(DisplayAdmission {
            receipt,
            superseded,
        })
    }

    pub(crate) fn start_next(&mut self) -> Option<DisplayApply> {
        if self.active.is_some() {
            return None;
        }
        let mut transition = self.pending.take()?;
        if self.observed.backend_mode == DisplayMode::Windowed
            && transition.request.mode() != DisplayMode::Windowed
        {
            self.saved_windowed = self.observed.windowed_placement();
        }
        let windowed_placement = match &transition.request {
            DisplayRequest::Windowed { placement } => {
                placement.or(self.saved_windowed.filter(|saved| !saved.maximized))
            }
            _ => None,
        };
        transition.state = DisplayTransitionState::Applying;
        self.active = Some(transition.clone());
        Some(DisplayApply {
            transition,
            windowed_placement,
        })
    }

    pub(crate) fn applied(
        &mut self,
        receipt: DisplayReceipt,
        observed: DisplayObserved,
    ) -> Result<(), Error> {
        let active = self
            .active
            .as_mut()
            .filter(|transition| {
                transition.receipt == receipt
                    && transition.state == DisplayTransitionState::Applying
            })
            .ok_or(Error::StaleIdentity)?;
        self.observed = observed;
        active.state = DisplayTransitionState::AwaitingPresentation;
        if active.request.mode() == DisplayMode::Windowed
            && observed.backend_mode == DisplayMode::Windowed
        {
            self.saved_windowed = None;
        }
        Ok(())
    }

    pub(crate) fn fail(
        &mut self,
        receipt: DisplayReceipt,
        reason: Error,
    ) -> Result<DisplayTransition, Error> {
        if !self
            .active
            .as_ref()
            .is_some_and(|transition| transition.receipt == receipt)
        {
            return Err(Error::StaleIdentity);
        }
        let mut transition = self.active.take().unwrap();
        transition.state = DisplayTransitionState::Failed(reason);
        self.settled = Some(transition.clone());
        Ok(transition)
    }

    pub(crate) fn report(
        &mut self,
        receipt: DisplayReceipt,
        status: PresentationStatus,
    ) -> Result<DisplayTransition, Error> {
        let active = self
            .active
            .as_ref()
            .filter(|transition| {
                transition.receipt == receipt
                    && transition.state == DisplayTransitionState::AwaitingPresentation
            })
            .ok_or(Error::StaleIdentity)?;
        if matches!(status, PresentationStatus::Ready)
            && self.observed.backend_mode != active.request.mode()
        {
            return Err(Error::InvalidInput(
                "backend mode has not matched this request",
            ));
        }
        let mut transition = self.active.take().unwrap();
        transition.state = match status {
            PresentationStatus::Ready => DisplayTransitionState::Ready,
            PresentationStatus::Failed(reason) => DisplayTransitionState::Failed(reason),
        };
        self.settled = Some(transition.clone());
        Ok(transition)
    }

    pub(crate) fn observe(&mut self, observed: DisplayObserved) {
        self.observed = observed;
    }
    pub(crate) fn observed(&self) -> DisplayObserved {
        self.observed
    }
    pub(crate) fn pending(&self) -> Option<&DisplayTransition> {
        self.pending.as_ref()
    }
    pub(crate) fn active(&self) -> Option<&DisplayTransition> {
        self.active.as_ref()
    }
    pub(crate) fn settled(&self) -> Option<&DisplayTransition> {
        self.settled.as_ref()
    }

    pub(crate) fn begin_retirement(&mut self) -> Option<DisplayTransition> {
        self.accepting = false;
        let mut pending = self.pending.take()?;
        pending.state = DisplayTransitionState::Superseded;
        Some(pending)
    }
}

#[cfg(test)]
#[path = "../tests/unit/display_transitions.rs"]
mod tests;
