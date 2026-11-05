//! A finite owned wave with explicit partial-admission handling.
use engine_worker::{
    BatchSpawnOptions, ExecutionClass, Outcome, OwnedLimits, Runtime, RuntimeConfig, TaskStatus,
    WorkerConfig,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = RuntimeConfig::new(3, [WorkerConfig::new(1); 3])?;
    let mut runtime = Runtime::builder(config)
        .with_owned_limits(OwnedLimits::new(32, 32, [16; 3], [2; 3])?)
        .build()?;
    let lane = runtime.lane(ExecutionClass::High);
    let mut batch = lane.batch();
    let group = batch
        .begin()
        .map_err(|error| format!("begin wave: {error:?}"))?;
    let operations = (0_u64..8).map(|value| move || value * value).collect();
    let (mut accepted, rejection) = match lane.try_spawn_batch(
        BatchSpawnOptions {
            group: Some(group),
            ..Default::default()
        },
        operations,
    ) {
        Ok(accepted) => (accepted, None),
        Err(rejected) => {
            // This application declines to retry. The untouched suffix can be
            // dropped, but accepted work must still settle before phase exit.
            let remaining = rejected.remaining.len();
            (rejected.accepted, Some((rejected.reason, remaining)))
        }
    };
    group.seal();
    group
        .wait_helping()
        .map_err(|error| format!("settle wave: {error:?}"))?;
    assert_eq!(group.completion().status(), Some(TaskStatus::Succeeded));
    for (index, (task, _control)) in accepted.iter_mut().enumerate() {
        match task.try_take() {
            Some(Outcome::Success(value)) => assert_eq!(value, (index as u64).pow(2)),
            _ => return Err("accepted operation did not succeed".into()),
        }
    }
    runtime.shutdown()?;
    if let Some((reason, remaining)) = rejection {
        return Err(
            format!("phase incomplete: {reason:?}; {remaining} operations unaccepted").into(),
        );
    }
    Ok(())
}
