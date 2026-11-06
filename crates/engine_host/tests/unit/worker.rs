use super::*;
use crate::{CoreAffinity, CpuCore};
use engine_worker::ExecutionClass;

#[expect(clippy::cast_possible_truncation, reason = "a test has few cores")]
fn topology(physical: usize, logical: usize) -> CpuTopology {
    CpuTopology {
        cores: (0..physical)
            .map(|index| CpuCore {
                affinities: vec![CoreAffinity {
                    group: index as u16,
                    mask: 1,
                }],
                efficiency_class: 0,
            })
            .collect(),
        logical_processors: logical,
    }
}
fn request(limits: [usize; 3], reserved: usize) -> WorkerRequest {
    WorkerRequest {
        reserved_physical_cores: reserved,
        class_limits: limits,
        priorities: [
            Some(ThreadPriority::BelowNormal),
            Some(ThreadPriority::Normal),
            Some(ThreadPriority::AboveNormal),
        ],
    }
}
fn counts(plan: WorkerPlan) -> [usize; 3] {
    ExecutionClass::ALL.map(|class| plan.effective().workers_for(class).worker_count())
}

#[test]
fn physical_input_and_independent_caps_do_not_become_an_aggregate_cpu_limit() {
    let plan = WorkerPlan::independent_physical_caps(&topology(6, 12), request([6; 3], 1));
    assert_eq!(counts(plan), [5; 3]);
    assert_eq!(plan.effective().worker_budget(), 15);
    let plan = WorkerPlan::independent_physical_caps(&topology(10, 12), request([6; 3], 1));
    assert_eq!(counts(plan), [6; 3]);
    assert_eq!(plan.effective().worker_budget(), 18);
    assert_eq!(plan.requested(), request([6; 3], 1));
    for (class, priority) in ExecutionClass::ALL
        .into_iter()
        .zip(plan.requested().priorities)
    {
        assert_eq!(
            plan.effective().workers_for(class).requested_priority(),
            priority
        );
    }
}

#[test]
fn zero_is_disabled_and_floor_ceiling_arithmetic_is_bounded() {
    let plan = WorkerPlan::independent_physical_caps(
        &topology(1, 2),
        request([0, 1, usize::MAX], usize::MAX),
    );
    assert_eq!(counts(plan), [0, 2, 2]);
    let plan =
        WorkerPlan::independent_physical_caps(&topology(128, 256), request([usize::MAX; 3], 0));
    assert_eq!(counts(plan), [64; 3]);
    assert_eq!(plan.effective().worker_budget(), 192);
    let plan = WorkerPlan::independent_physical_caps(&topology(8, 16), request([0; 3], 0));
    assert_eq!(counts(plan), [0; 3]);
    assert_eq!(plan.effective().worker_budget(), 0);
}
