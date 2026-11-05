//! An owner-thread Windows x64 application host.
//!
//! The host borrows application state for one run; applications may contain
//! non-`Send` values and externally scoped cleanup contexts. Startup means the
//! application callback ran, not that a renderer or resource domain is ready.
//! Stop closes ordinary admission and retains windows until the application
//! reports its host-dependent obligations settled.
//!
//! Owner-local dispatch, bounded owned posts and checked raw-deadline timers
//! are supported. Native keyboard, mouse and session-stamped text pass through
//! the growing input gate. Prepared native cursors and independent input modes
//! are supported. Source-specific service continuations, owned wake hints,
//! checked application time and explicitly selected frame pacing are supported.
//! Retained native access and serialized display changes support direct renderer
//! composition; renderer completion remains the application's contract. Topology,
//! explicit worker planning and shell association services are available.
//! Shutdown observations distinguish pending obligations without certifying
//! provider completion. The private winit backend owns the native event loop.
//! A second event loop in the same process is not promised.
#![deny(unsafe_code)]
#![deny(missing_docs)]

#[cfg(not(all(target_os = "windows", target_arch = "x86_64")))]
compile_error!("engine_host currently supports Windows x64 only");

mod backend;
mod context;
mod cursor;
mod dispatch;
mod display;
mod error;
mod host;
mod identity;
mod input;
mod input_mode;
mod pacing;
mod post;
mod service;
mod shell;
mod shutdown;
mod text;
mod time;
mod timer;
mod topology;
mod window;
mod window_access;
#[cfg(feature = "engine_worker")]
mod worker;

pub use context::{AdmissionState, Context, ContextPhase};
pub use cursor::{Cursor, CursorSelection, PreparedCursor, SystemCursor};
pub use dispatch::{
    DispatchReport, Event, EventFilter, Handler, Priority, Propagation, RecipientState,
};
pub use display::{
    DisplayGeometry, DisplayMode, DisplayObserved, DisplayReceipt, DisplayRequest,
    DisplayTransition, DisplayTransitionId, DisplayTransitionState, MonitorId, MonitorSnapshot,
    PresentationRevision, PresentationStatus, ScaleFactor, ScreenPosition, VideoMode,
    WindowedPlacement,
};
pub use error::{Error, IdentityKind, NativeOperation, Rejected};
pub use host::{
    Application, ExitReport, Host, HostConfig, HostState, StopOutcome, StopProgress, StopReason,
};
pub use identity::{HostId, WindowGeneration, WindowId, WindowTarget};
pub use identity::{PostId, RecipientGeneration, RecipientId, SubscriptionId, TimerId};
pub use input::{
    ButtonState, InputDeviceId, InputDrainReport, InputEvent, InputOrigin, InputRecord, InputStamp,
    InputState, InputStateLayer, KeyLocation, LogicalKey, Modifiers, MouseButton, NamedKey,
    PhysicalKey, PhysicalPosition, ScrollDelta,
};
pub use input_mode::{CursorState, InputMode, InputModeState, RawInputPolicy, RawInputState};
pub use pacing::{Frame, FramePhase, PacingPolicy, PacingState};
pub use post::{PostDiscardReason, PostOutcome, PostReceipt, Proxy};
pub use service::{
    ServiceBudget, ServiceId, ServicePoint, ServicePoints, ServiceReport, ServiceRequest,
    ServiceSpec, ServiceState, ServiceVisit, Wake,
};
pub use shell::{
    ShellDestination, ShellFailure, ShellOutcome, ShellReceipt, ShellRejected, ShellRequest,
    ShellRequestId,
};
pub use shutdown::ShutdownSnapshot;
pub use text::{TextCaret, TextSessionId};
pub use time::{ApplicationTime, ClockSnapshot, RawDeadline, RawTime};
pub use timer::{TimerCancel, TimerPumpReport};
pub use topology::{CoreAffinity, CpuCore, CpuTopology, TopologyError};
pub use window::{PhysicalSize, WindowSpec, WindowState};
pub use window_access::{NativeWindowRef, WindowAccess};
#[cfg(feature = "engine_worker")]
pub use worker::{WorkerPlan, WorkerRequest};

#[cfg(test)]
#[path = "../examples/service_composition.rs"]
mod composition_tests;
#[cfg(test)]
#[path = "../tests/unit/dispatch.rs"]
mod dispatch_tests;
#[cfg(test)]
#[path = "../tests/unit/fatal_retirement.rs"]
mod fatal_retirement_tests;
#[cfg(test)]
#[path = "../tests/unit/foundation.rs"]
mod foundation_tests;
#[cfg(test)]
#[path = "../tests/unit/input.rs"]
mod input_tests;
#[cfg(test)]
#[path = "../tests/unit/lifecycle.rs"]
mod lifecycle_tests;
#[cfg(test)]
#[path = "../tests/unit/native_integration.rs"]
mod native_integration_tests;
#[cfg(test)]
#[path = "../tests/unit/pacing.rs"]
mod pacing_tests;
#[cfg(test)]
#[path = "../tests/unit/post_cache_replacement.rs"]
mod post_cache_replacement_tests;
#[cfg(test)]
#[path = "../tests/unit/posts.rs"]
mod post_tests;
#[cfg(test)]
#[path = "../tests/unit/services.rs"]
mod service_tests;
#[cfg(test)]
#[path = "../tests/unit/timers.rs"]
mod timer_tests;
#[cfg(test)]
#[path = "../tests/unit/window_driver.rs"]
mod window_driver_tests;

#[cfg(test)]
#[path = "../tests/unit/shutdown.rs"]
mod shutdown_tests;
#[cfg(test)]
#[path = "../tests/unit/workloads.rs"]
mod workload_tests;

#[cfg(test)]
#[path = "../tests/unit/allocation_probe.rs"]
mod allocation_probe;
