//! A simulated provider plus a dependent CPU stage, without blocking a worker.
//! See pubdocs/resources.md. No actual foreign memory access is started here.

use std::fmt::Debug;
use std::io;
use std::num::NonZeroUsize;

use engine_worker::{
    CallerEligibility, Cost, ExecutionClass, ExternalOptions, Limits, Outcome, OwnedLimits,
    Priority, Retained, Runtime, RuntimeConfig, Shared, SpawnOptions, StageOptions, TaskStatus,
    WorkerConfig,
};

const URGENT: Priority = Priority::new(0);
const BACKGROUND: Priority = Priority::new(10);

fn error(value: impl Debug) -> io::Error {
    io::Error::other(format!("{value:?}"))
}

fn load(runtime: &Runtime) -> io::Result<Shared<Retained<u64>>> {
    let low = runtime.lane(ExecutionClass::Low);
    let producers = runtime
        .work_set(NonZeroUsize::new(1).unwrap())
        .map_err(error)?;
    let discovery = producers.discovery().map_err(error)?;
    let consumers = runtime
        .work_set(NonZeroUsize::new(1).unwrap())
        .map_err(error)?;
    let (provider, source, _control) = runtime
        .external::<Vec<u32>>(ExternalOptions {
            work_set: Some(&producers),
            priority: Some(BACKGROUND),
            cost: Cost::new(1, 0, 0, 128),
            retained_bytes: 128,
            ..Default::default()
        })
        .map_err(error)?;
    let source = source.into_shared();
    let demand = source
        .completion()
        .demand_in(&consumers, URGENT)
        .map_err(error)?;
    producers.seal();
    consumers.seal();

    // A live discovery permit permits this child after root closure. The CPU
    // operation waits on a dependency, not on a blocked read inside a worker.
    let group = low.group().map_err(error)?;
    let input = source.clone();
    let prerequisites = [source.completion()];
    let submitted = low.try_spawn_stage(
        StageOptions {
            spawn: SpawnOptions {
                eligibility: CallerEligibility::CallerEligible,
            },
            group: Some(&group),
            prerequisites: &prerequisites,
            discovery: Some(&discovery),
            priority: Some(URGENT),
            cost: Cost::new(1, 1, 0, size_of::<u64>()),
            retained_bytes: size_of::<u64>(),
            ..Default::default()
        },
        move || {
            let outcome = input.try_result().expect("prerequisite ready");
            match &*outcome {
                Outcome::Success(values) => values.iter().map(|&v| u64::from(v)).sum(),
                _ => unreachable!("success-only source prerequisite"),
            }
        },
    );
    group.seal();
    let (prepared, _control) = submitted.map_err(error)?;
    drop(discovery); // No more children can be discovered by this operation.
    while runtime.service_demand(32) {}

    // Simulate a provider callback. In a real adapter this runs only when bytes
    // are safe to observe; foreign writes need independent physical retirement.
    provider
        .complete((0_u32..32).collect())
        .map_err(|_| io::Error::other("provider completion lost cancellation race"))?;
    group.wait_helping().map_err(error)?;
    drop(demand);
    while runtime.service_demand(32) {}
    if prepared.status() != Some(TaskStatus::Succeeded) {
        return Err(io::Error::other("resource preparation failed"));
    }
    assert!(producers.is_drained());
    assert!(consumers.is_drained());
    Ok(prepared.into_shared())
}

fn main() -> io::Result<()> {
    let config = RuntimeConfig::new(3, [WorkerConfig::new(1); 3]).map_err(error)?;
    let mut runtime = Runtime::builder(config)
        .with_owned_limits(OwnedLimits::new(32, 64, [16; 3], [2; 3]).map_err(error)?)
        .with_capacity_limits(
            Limits::new(
                Cost::new(32, 64, 4, 4096),
                Cost::new(4, 4, 1, 0),
                1,
                Some(8192),
            )
            .map_err(error)?,
        )
        .with_demand_limits(vec![URGENT, BACKGROUND], 8)
        .build()
        .map_err(error)?;
    let loaded = load(&runtime);
    // Service remaining control propagation even on a failed admission path.
    while runtime.service_demand(32) {}
    runtime.shutdown().map_err(error)?;
    let shared = loaded?;
    let outcome = shared.try_result().expect("ready retained result");
    let Outcome::Success(value) = &*outcome else {
        return Err(io::Error::other("missing resource output"));
    };
    assert_eq!(**value, 496);
    println!(
        "sum = {}; immutable output remains valid after shutdown",
        **value
    );
    Ok(())
}
