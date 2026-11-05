//! Owned observations of separate host retirement obligations.

use crate::host::Core;
use crate::{Application, Context, Host, HostId, HostState, StopReason};

/// A point-in-time observation, not a provider/GPU completion certificate.
/// Application settlement is its own report. Foreign lease releases and helper
/// progress may change these counts immediately after the snapshot. Normal
/// return still requires the host's independent retirement checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShutdownSnapshot {
    /// Persistent host identity.
    pub host: HostId,
    /// Current owner lifecycle state.
    pub state: HostState,
    /// Latched reason for closing ordinary admission.
    pub stop_reason: Option<StopReason>,
    /// Whether new ordinary work may be accepted.
    pub ordinary_open: bool,
    /// Whether `app.stopping` has reported Settled. This says nothing about
    /// independently retained `engine_host` leases, helpers or native destruction.
    pub application_settled: bool,
    /// Application/handler callbacks presently active on the owner stack.
    pub active_callbacks: usize,
    /// Accepted posts still queued, detached, active or being reclaimed.
    pub accepted_posts: usize,
    /// Owned timers awaiting claim, regardless of whether their deadline is due.
    pub pending_timers: usize,
    /// Timer payloads already claimed and retained through their callbacks.
    pub claimed_timers: usize,
    /// Received owned input awaiting delivery or retirement.
    pub queued_input: usize,
    /// Active service callbacks, including retained removed registrations.
    pub active_services: usize,
    /// Registrations still eligible for required retirement callbacks.
    pub retirement_services: usize,
    /// Window records whose actual native owner roots remain retained.
    pub retained_window_roots: usize,
    /// External access leases observed across those roots.
    pub external_window_leases: usize,
    /// Native roots released but actual Destroyed acknowledgments outstanding.
    pub pending_native_destructions: usize,
    /// Stop requires native input deregistration/confinement confirmation.
    pub native_input_pending: bool,
    /// Accepted shell requests whose call or payload reclamation is unfinished.
    pub shell_requests: usize,
    /// Stop still retains a helper pending its final apartment/thread retirement.
    /// This can remain true with zero shell requests.
    pub shell_helper_retiring: bool,
}

impl<A: Application> Core<A> {
    pub(crate) fn shutdown_snapshot(&self) -> ShutdownSnapshot {
        let stopping = matches!(self.state, HostState::Stopping | HostState::Retiring);
        let native_input_pending = stopping
            && (self.raw_input.confirmed != Some(crate::RawInputPolicy::Never)
                || self.raw_input.failure.is_some()
                || self.windows.iter().any(|(_, record)| {
                    record.input_mode.confirmed.confine_pointer || record.confinement_uncertain
                }));
        let (pending_timers, claimed_timers) = self.timers.counts();
        let (active_services, retirement_services) = self.services.counts();
        ShutdownSnapshot {
            host: self.id,
            state: self.state,
            stop_reason: self.stop_reason,
            ordinary_open: self.ordinary_open(),
            application_settled: self.application_settled,
            active_callbacks: self.active_callbacks,
            accepted_posts: self.transport.accepted(),
            pending_timers,
            claimed_timers,
            queued_input: self.input.queue.len(),
            active_services,
            retirement_services,
            retained_window_roots: self.windows.iter().count(),
            external_window_leases: self.windows.iter().fold(0_usize, |count, (_, record)| {
                count.saturating_add(record.access.external_count())
            }),
            pending_native_destructions: self
                .native_windows
                .iter()
                .filter(|(_, target)| {
                    !self
                        .windows
                        .get(target.id)
                        .is_ok_and(|record| record.generation == target.generation)
                })
                .count(),
            native_input_pending,
            shell_requests: self
                .shell
                .as_ref()
                .map_or(0, crate::shell::ShellHelper::pending),
            shell_helper_retiring: stopping && self.shell.is_some(),
        }
    }
}

impl<A: Application> Context<'_, A> {
    /// Observes separate pending obligations without pumping or changing them.
    /// Application/provider state remains owned by the application.
    pub fn shutdown_snapshot(&self) -> ShutdownSnapshot {
        self.core.shutdown_snapshot()
    }
}

impl<A: Application> Host<A> {
    /// Observes host-owned state outside its borrowed native run.
    pub fn shutdown_snapshot(&self) -> ShutdownSnapshot {
        self.core.shutdown_snapshot()
    }
}
