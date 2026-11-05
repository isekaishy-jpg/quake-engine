//! Explicit cap profiles and frame eligibility; rendering remains application-owned.

use crate::{ApplicationTime, Error, RawDeadline, RawTime, WindowTarget};
use std::time::Duration;

/// Explicit Forever phase policy, separate from renderer and window activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FramePhase {
    /// Apply the recovered 60 Hz Glue ceiling.
    Glue,
    /// No additional phase ceiling.
    Ordinary,
}

/// Explicit limiter ownership. Zero rates mean unlimited; finite positive rates
/// below eight become eight. `engine_host` never installs a cap implicitly for a renderer.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PacingPolicy {
    /// No frame scheduling. Independent owner services still progress.
    #[default]
    Disabled,
    /// The application/renderer requests redraw when ready; `engine_host` adds no interval.
    RendererManaged,
    /// Stock selection uses foreground while renderer-active, otherwise the
    /// smaller foreground/background cap; interval is whole milliseconds.
    Stock {
        /// Foreground rate, zero for unlimited.
        foreground_rate: u32,
        /// Background rate, zero for unlimited.
        background_rate: u32,
    },
    /// Forever selects foreground/background explicitly and applies its phase
    /// ceiling, with nanosecond intervals and no catch-up frame loop.
    Forever {
        /// Foreground rate, zero for unlimited.
        foreground_rate: u32,
        /// Background rate, zero for unlimited.
        background_rate: u32,
        /// Explicit application activation policy, not an inferred focus query.
        foreground: bool,
        /// Explicit phase ceiling.
        phase: FramePhase,
    },
}
impl PacingPolicy {
    /// Selected physical cap; `None` means `engine_host` adds no interval limiter.
    pub fn effective_rate(self, renderer_active: bool) -> Option<u32> {
        let rate = |value: u32| (value != 0).then_some(value.max(8));
        match self {
            Self::Disabled | Self::RendererManaged => None,
            Self::Stock {
                foreground_rate,
                background_rate,
            } => {
                let foreground = rate(foreground_rate);
                if renderer_active {
                    foreground
                } else {
                    match (foreground, rate(background_rate)) {
                        (Some(a), Some(b)) => Some(a.min(b)),
                        (a, b) => a.or(b),
                    }
                }
            }
            Self::Forever {
                foreground_rate,
                background_rate,
                foreground,
                phase,
            } => {
                let selected = rate(if foreground {
                    foreground_rate
                } else {
                    background_rate
                });
                if phase == FramePhase::Glue {
                    Some(selected.unwrap_or(60).min(60))
                } else {
                    selected
                }
            }
        }
    }
    fn interval(self, renderer_active: bool) -> Option<Duration> {
        self.effective_rate(renderer_active).map(|rate| match self {
            Self::Stock { .. } => Duration::from_millis(1000 / u64::from(rate)),
            _ => Duration::from_nanos(1_000_000_000 / u64::from(rate)),
        })
    }
}

/// One admitted frame; raw eligibility and application delta are distinct clocks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Frame {
    /// Exact target generation receiving this redraw.
    pub target: WindowTarget,
    /// Checked host-local frame count.
    pub sequence: u64,
    /// Raw claim time, used only for physical pacing.
    pub raw: RawTime,
    /// Cached application timestamp for this frame.
    pub application: ApplicationTime,
    /// Application delta since the last admitted frame, zero on the first frame.
    pub delta: Duration,
}
impl Frame {
    /// Delta seconds for application update, without implying a fixed time step.
    pub fn delta_seconds(self) -> f64 {
        self.delta.as_secs_f64()
    }
}

/// Explicit pacing request and its current application-supplied renderer state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PacingState {
    /// Selected frame target; absent means no native redraw target.
    pub target: Option<WindowTarget>,
    /// Explicit limiter owner/profile.
    pub policy: PacingPolicy,
    /// Supplied by SR/application rather than normalized OS focus.
    pub renderer_active: bool,
    /// Selected cap, independent of application-time rescaling.
    pub effective_rate: Option<u32>,
}

pub(crate) struct Pacing {
    pub(crate) state: PacingState,
    pub(crate) next: Option<RawDeadline>,
    pub(crate) requested: bool,
    redraw_armed: bool,
    sequence: u64,
    previous_application: Option<ApplicationTime>,
    pub(crate) active: bool,
    frame_raw: Option<RawTime>,
}
impl Pacing {
    pub(crate) fn new() -> Self {
        Self {
            state: PacingState {
                target: None,
                policy: PacingPolicy::Disabled,
                renderer_active: true,
                effective_rate: None,
            },
            next: None,
            requested: false,
            redraw_armed: false,
            sequence: 0,
            previous_application: None,
            active: false,
            frame_raw: None,
        }
    }
    pub(crate) fn set(&mut self, target: Option<WindowTarget>, policy: PacingPolicy, raw: RawTime) {
        self.state.target = target;
        self.state.policy = policy;
        self.state.effective_rate = policy.effective_rate(self.state.renderer_active);
        self.requested = target.is_some() && policy != PacingPolicy::Disabled;
        self.next = self.requested.then_some(RawDeadline(raw));
        self.redraw_armed = false;
    }
    pub(crate) fn renderer_active(&mut self, active: bool, raw: RawTime) {
        if active != self.state.renderer_active {
            self.state.renderer_active = active;
            self.set(self.state.target, self.state.policy, raw);
        }
    }
    pub(crate) fn request(&mut self, raw: RawTime) -> Result<(), Error> {
        if self.state.policy == PacingPolicy::Disabled || self.state.target.is_none() {
            return Err(Error::InvalidInput("frame scheduling is disabled"));
        }
        self.requested = true;
        if self.state.policy == PacingPolicy::RendererManaged {
            self.next = Some(RawDeadline(raw));
        } else if self.next.is_none() {
            self.next = Some(
                self.frame_raw.unwrap_or(raw).checked_add(
                    self.state
                        .policy
                        .interval(self.state.renderer_active)
                        .unwrap_or(Duration::ZERO),
                )?,
            );
        }
        Ok(())
    }
    pub(crate) fn arm_redraw(&mut self, raw: RawTime) -> Option<WindowTarget> {
        if self.state.policy == PacingPolicy::Disabled
            || self.active
            || self.redraw_armed
            || !self.requested
            || self
                .next
                .is_some_and(|deadline| deadline.time().elapsed > raw.elapsed)
        {
            return None;
        }
        let target = self.state.target?;
        self.redraw_armed = true;
        Some(target)
    }
    pub(crate) fn eligible(&self, target: WindowTarget, raw: RawTime) -> bool {
        !self.active
            && self.state.target == Some(target)
            && self.state.policy != PacingPolicy::Disabled
            && self.requested
            && self
                .next
                .is_none_or(|deadline| deadline.time().elapsed <= raw.elapsed)
    }
    pub(crate) fn claim(
        &mut self,
        target: WindowTarget,
        raw: RawTime,
        application: ApplicationTime,
    ) -> Result<Option<Frame>, Error> {
        if self.state.target != Some(target) {
            return Ok(None);
        }
        self.redraw_armed = false;
        if !self.eligible(target, raw) {
            return Ok(None);
        }
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or(Error::IdentityExhausted(crate::IdentityKind::Frame))?;
        let delta = self
            .previous_application
            .map_or(Duration::ZERO, |previous| {
                application.elapsed.saturating_sub(previous.elapsed)
            });
        self.sequence = sequence;
        self.previous_application = Some(application);
        self.requested = false;
        self.next = None;
        self.active = true;
        self.frame_raw = Some(raw);
        Ok(Some(Frame {
            target,
            sequence,
            raw,
            application,
            delta,
        }))
    }
    pub(crate) fn complete(&mut self, raw: RawTime) -> Result<(), Error> {
        self.active = false;
        if matches!(
            self.state.policy,
            PacingPolicy::Stock { .. } | PacingPolicy::Forever { .. }
        ) && self.state.target.is_some()
            && !self.requested
        {
            self.requested = true;
            // Count application work within the interval. An already late
            // deadline admits one next frame, whose fresh raw claim sets its
            // own interval; never execute a missed-frame catch-up loop.
            self.next = Some(
                self.frame_raw.take().unwrap_or(raw).checked_add(
                    self.state
                        .policy
                        .interval(self.state.renderer_active)
                        .unwrap_or(Duration::ZERO),
                )?,
            );
        }
        Ok(())
    }
    pub(crate) fn deadline(&self) -> Option<RawDeadline> {
        if self.redraw_armed || self.active {
            None
        } else {
            self.next
        }
    }
}
