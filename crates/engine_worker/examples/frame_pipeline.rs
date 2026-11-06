//! Finite CPU example for pubdocs/execution.md and pubdocs/ownership.md.
//! There is no native event loop, provider wait or GPU access in this example.
#![expect(clippy::unwrap_used, reason = "an example keeps to what it shows")]
#![expect(clippy::print_stdout, reason = "an example reports to its console")]

use std::fmt::Debug;
use std::io;
use std::num::NonZeroUsize;

use engine_worker::{
    CallerEligibility, DependencyPolicy, ExecutionClass, Outcome, OwnedLimits, Phase, PumpBudget,
    Runtime, RuntimeConfig, Shared, SpawnOptions, TaskStatus, WorkerConfig,
};

fn error(value: impl Debug) -> io::Error {
    io::Error::other(format!("{value:?}"))
}

fn run_frame(runtime: &Runtime) -> io::Result<()> {
    let high = runtime.lane(ExecutionClass::High);
    let options = SpawnOptions {
        eligibility: CallerEligibility::CallerEligible,
    };
    let mut batches = high.batch();
    let preparation = batches.begin().map_err(error)?;

    // This example's chunks are independent. A failed admission can settle the
    // accepted subset safely; live domain mutation might instead need a gate.
    let admitted = (|| {
        let mut pieces = Vec::<Shared<Vec<u64>>>::new();
        for first in [0_u64, 8, 16, 24] {
            let (task, _control) = high
                .try_spawn_in(preparation, options, move || {
                    (first..first + 8).map(|value| value * value).collect()
                })
                .map_err(error)?;
            pieces.push(task.into_shared());
        }
        Ok::<_, io::Error>(pieces)
    })();
    // A rejected submission does not seal or fail the group for us.
    preparation.seal();
    let pieces = admitted?;

    let finalization = high.group().map_err(error)?;
    let submitted = high.try_spawn_after_in(
        &finalization,
        options,
        &[preparation.completion()],
        DependencyPolicy::SuccessOnly,
        move || {
            let mut ordered = Vec::with_capacity(32);
            // Preserve input order even if preparation completed out of order.
            for piece in pieces {
                let outcome = piece.try_result().expect("successful predecessor ready");
                match &*outcome {
                    Outcome::Success(values) => ordered.extend_from_slice(values),
                    _ => unreachable!("success-only predecessor group"),
                }
            }
            ordered
        },
    );
    finalization.seal();
    let (task, _control) = submitted.map_err(error)?;
    let shared = task.into_shared();

    let phase = Phase(1);
    let mut owner = runtime
        .owner(Vec::<u64>::new(), NonZeroUsize::new(1).unwrap())
        .map_err(error)?;
    owner.set_phase(phase).map_err(error)?;
    let publication_input = shared.clone();
    let (_delivery, _delivery_control) = owner
        .on_ready(&shared.completion(), phase, move |state, status| {
            if status == TaskStatus::Succeeded {
                let outcome = publication_input.try_result().expect("ready result");
                if let Outcome::Success(values) = &*outcome {
                    // The payload is borrowed. Only a small domain summary is copied.
                    state.push(values.iter().sum());
                }
            }
        })
        .map_err(error)?;

    // Both stages have already been admitted. Helping finalization does not
    // recursively help preparation, so help its predecessor explicitly first.
    // These waits are valid here: CPU completion needs no owner/provider pump.
    preparation.wait_helping().map_err(error)?;
    finalization.wait_helping().map_err(error)?;
    if finalization.completion().status() != Some(TaskStatus::Succeeded) {
        return Err(io::Error::other("frame preparation failed"));
    }
    owner.pump(phase, PumpBudget::new(1)).map_err(error)?;

    // A second reader uses the same immutable result without consuming it.
    let outcome = shared.try_result().expect("finalization settled");
    let Outcome::Success(values) = &*outcome else {
        return Err(io::Error::other("missing frame output"));
    };
    let expected = (0_u64..32).map(|value| value * value).sum::<u64>();
    assert_eq!(values.len(), 32);
    assert_eq!(owner.state().as_slice(), &[expected]);
    println!("published {} values; sum = {expected}", values.len());
    owner.close();
    Ok(())
}

fn main() -> io::Result<()> {
    // Illustrative capacities, not a recommended production worker split.
    let config = RuntimeConfig::new(
        4,
        [
            WorkerConfig::new(1),
            WorkerConfig::new(1),
            WorkerConfig::new(2),
        ],
    )
    .map_err(error)?;
    let mut runtime = Runtime::builder(config)
        .with_owned_limits(OwnedLimits::new(32, 64, [16; 3], [2; 3]).map_err(error)?)
        .build()
        .map_err(error)?;
    let result = run_frame(&runtime);
    // All example owners have closed/dropped, and no host-driven providers exist.
    let shutdown = runtime.shutdown().map_err(error);
    result.and(shutdown)
}
