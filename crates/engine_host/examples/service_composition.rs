//! Real `engine_worker` owner publication and independent, borrowed `engine_cache` cleanup. This uses
//! controlled CPU data, not a renderer/provider final-use certification.

#[cfg(test)]
use crate as engine_host;
use engine_cache::{AccountingDomain, AllocationClass, Backing, CleanupContext};
use engine_host::*;
use engine_worker::{
    ExecutionClass, NotifyBinding, NotifyLimits, NotifyRoute, OwnedLimits, Owner, Phase,
    PumpBudget, Runtime, RuntimeConfig, SpawnOptions, TaskStatus, WorkerConfig,
};
use std::{cell::Cell, num::NonZeroUsize, rc::Rc, time::Duration};

fn error(value: impl std::fmt::Debug) -> Error {
    Error::application(format!("{value:?}"))
}
fn runtime() -> Runtime {
    let config = RuntimeConfig::new(
        1,
        [
            WorkerConfig::new(0),
            WorkerConfig::new(0),
            WorkerConfig::new(1),
        ],
    )
    .unwrap();
    Runtime::builder(config)
        .with_owned_limits(OwnedLimits::new(8, 8, [4; 3], [1; 3]).unwrap())
        .with_notification_limits(NotifyLimits {
            routes: 1,
            bindings: 3,
        })
        .build()
        .unwrap()
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkLifecycle {
    Running,
    Draining,
}

struct Demo<'scope> {
    runtime: &'scope mut Runtime,
    cleanup: &'scope CleanupContext<Rc<Cell<usize>>>,
    backing: Option<Backing<'scope, Rc<Cell<usize>>>>,
    domain: AccountingDomain,
    owner: Owner<Vec<u32>>,
    route: Option<NotifyRoute>,
    bindings: Vec<NotifyBinding>,
    work: Option<ServiceId>,
    clean: Option<ServiceId>,
    final_admitted: bool,
    lifecycle: WorkLifecycle,
    release: Option<RawDeadline>,
    cleanup_visits_after_close: usize,
    cleaned: usize,
    joined: bool,
}
impl<'scope> Demo<'scope> {
    fn new(runtime: &'scope mut Runtime, cleanup: &'scope CleanupContext<Rc<Cell<usize>>>) -> Self {
        let domain = AccountingDomain::new();
        let backing = cleanup
            .try_acquire(
                Rc::new(Cell::new(11)),
                &domain,
                8,
                AllocationClass::Resident,
            )
            .unwrap();
        let mut owner = runtime
            .owner(Vec::new(), NonZeroUsize::new(2).unwrap())
            .unwrap();
        owner.set_phase(Phase(1)).unwrap();
        Self {
            runtime,
            cleanup,
            backing: Some(backing),
            domain,
            owner,
            route: None,
            bindings: Vec::new(),
            work: None,
            clean: None,
            final_admitted: false,
            lifecycle: WorkLifecycle::Running,
            release: None,
            cleanup_visits_after_close: 0,
            cleaned: 0,
            joined: false,
        }
    }
    fn begin_draining(&mut self, release: Option<RawDeadline>) {
        if self.lifecycle == WorkLifecycle::Running {
            // Close admission and ordinary publication before closing their route.
            self.lifecycle = WorkLifecycle::Draining;
            self.release = release;
            self.owner.close();
            if let Some(route) = &mut self.route {
                route.close().unwrap();
            }
            self.runtime.begin_shutdown();
        } else if release.is_none() {
            // Host retirement overrides the demonstration's delayed final release.
            self.release = None;
        }
    }

    fn retire(&mut self, cx: &mut Context<'_, Self>, raw: RawTime, budget: usize) -> bool {
        if self
            .release
            .is_none_or(|d| raw.elapsed() >= d.time().elapsed())
        {
            self.backing.take();
        }
        self.cleaned += self.cleanup.drain_budget(budget);
        let route_quiet = self.route.as_ref().is_none_or(NotifyRoute::is_quiescent);
        if route_quiet {
            // A claimed notifier can still signal after close. Keep its `engine_host` wake
            // destination until the route proves all such claims have retired.
            if let Some(work) = self.work.take() {
                cx.remove_service(work).unwrap();
            }
            self.joined = self.runtime.try_shutdown().unwrap();
        }
        self.backing.is_none() && self.cleanup.pending() == 0 && route_quiet && self.joined
    }

    fn work(&mut self, _: &mut Context<'_, Self>, request: ServiceRequest) -> ServiceReport {
        if self.lifecycle == WorkLifecycle::Draining {
            return ServiceReport::Quiescent;
        }
        // `engine_host` has consumed its wake before entry. Arm `engine_worker`, inspect/pump its
        // authoritative state, and recheck the epoch before reporting a wait.
        let route = self.route.as_mut().unwrap();
        let stamp = route.prepare_wait().unwrap();
        let report = self
            .owner
            .pump(Phase(1), PumpBudget::new(request.budget.records))
            .unwrap();
        if !self.final_admitted && self.owner.state().as_slice() == [1] {
            // The intermediate owner publication preceded this final task's
            // admission; watching only its completion could not wake that owner.
            self.final_admitted = true;
            let (task, _) = self
                .runtime
                .lane(ExecutionClass::High)
                .try_spawn(SpawnOptions::default(), || 7_u32)
                .unwrap();
            let shared = task.into_shared();
            let completion = shared.completion();
            self.bindings
                .push(route.watch_completion(&completion).unwrap());
            self.owner
                .on_ready(&completion, Phase(1), move |state, status| {
                    assert_eq!(status, TaskStatus::Succeeded);
                    assert!(shared.try_result().is_some());
                    state.push(7);
                })
                .unwrap();
        }
        if self.owner.state().as_slice() == [1, 7] {
            let release = request.raw.checked_add(Duration::from_millis(35)).unwrap();
            self.begin_draining(Some(release));
            // This source is finished. A separate `engine_cache` source will make progress
            // without `engine_worker` notifications or redraws after a later final release.
            return ServiceReport::Quiescent;
        }
        if route.changed_since(stamp).unwrap() || report.processed() == request.budget.records {
            ServiceReport::Continue
        } else {
            ServiceReport::AwaitWake {
                source: request.id,
                fallback_deadline: request.raw.checked_add(Duration::from_millis(10)).unwrap(),
            }
        }
    }
}
impl Application for Demo<'_> {
    type Message = ();
    type LocalEvent = ();
    fn started(&mut self, cx: &mut Context<'_, Self>) -> Result<(), Error> {
        cx.create_window(WindowSpec {
            visible: false,
            ..WindowSpec::default()
        })
        .map_err(|r| r.into_parts().1)?;
        let spec = ServiceSpec {
            points: ServicePoints::ALL,
            budget: ServiceBudget { records: 1 },
            fallback_interval: Duration::from_millis(10),
        };
        let work = cx.register_service(spec)?;
        self.work = Some(work);
        self.clean = Some(cx.register_service(spec)?);
        let wake = cx.wake(work)?;
        self.route = Some(
            self.runtime
                .notification_route(move || wake.signal().map_err(std::io::Error::other))
                .map_err(error)?,
        );
        // Retain the route and each installed binding before the next fallible
        // step, so startup rollback can still observe notifier quiescence.
        let route = self.route.as_mut().unwrap();
        self.bindings
            .push(route.watch_owner(&self.owner).map_err(error)?);
        self.bindings.push(route.watch_progress().map_err(error)?);
        let prepared = self
            .owner
            .prepare_delivery(Phase(1), |state| state.push(1))
            .map_err(error)?;
        prepared.ticket.ready();
        Ok(())
    }
    fn service(&mut self, cx: &mut Context<'_, Self>, request: ServiceRequest) -> ServiceReport {
        let is_work = Some(request.id) == self.work;
        if !is_work {
            assert_eq!(Some(request.id), self.clean);
        }
        // Native host retirement services can run before the first stopping poll.
        // Either service must close ordinary work before doing anything else.
        if request.point == ServicePoint::Retirement {
            self.begin_draining(None);
        }
        if self.lifecycle == WorkLifecycle::Draining {
            if !is_work {
                self.cleanup_visits_after_close += 1;
            }
            if self.retire(cx, request.raw, request.budget.records) {
                cx.request_stop();
            }
        } else if is_work {
            return self.work(cx, request);
        } else {
            self.cleaned += self.cleanup.drain_budget(request.budget.records);
        }
        if self.cleanup.pending() != 0 {
            ServiceReport::Continue
        } else {
            ServiceReport::Quiescent
        }
    }
    fn stopping(&mut self, cx: &mut Context<'_, Self>) -> StopProgress {
        self.begin_draining(None);
        let raw = cx.clock().raw;
        if self.retire(cx, raw, 1) {
            StopProgress::Settled
        } else {
            StopProgress::Pending
        }
    }
}

#[cfg(not(test))]
fn main() {
    let cleanup = CleanupContext::new();
    let mut runtime = runtime();
    let mut app = Demo::new(&mut runtime, &cleanup);
    let mut host = Host::<Demo<'_>>::new(HostConfig::default()).unwrap();
    let exit = host.run(&mut app).unwrap();
    assert_eq!(app.owner.state().as_slice(), [1, 7]);
    assert_eq!(app.cleaned, 1);
    assert!(app.joined);
    assert_eq!(app.domain.snapshot().total_declared_bytes, 0);
    assert_eq!(exit.windows_retired, 1);
    // `engine_host` observations describe its own obligations. The separate assertions
    // above establish the application's actual `engine_worker` join and `engine_cache` cleanup.
    let shutdown = host.shutdown_snapshot();
    assert_eq!(shutdown.state, engine_host::HostState::Closed);
    assert!(shutdown.application_settled);
    assert_eq!(shutdown.external_window_leases, 0);
    assert_eq!(shutdown.pending_native_destructions, 0);
    println!(
        "engine_worker owner publications [1, 7]; independent engine_cache cleanup {}; native retirement {}; runtime joined",
        app.cleaned, exit.windows_retired
    );
    println!("Host shutdown observations: {shutdown:?}");
}

#[cfg(test)]
#[path = "../tests/unit/composition.rs"]
mod tests;
