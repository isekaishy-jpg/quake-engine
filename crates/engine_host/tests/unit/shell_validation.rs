use super::super::{PREPARATIONS, VALIDATIONS};
use super::*;
use crate::backend::BackendOps;
use crate::{ContextPhase, ShellDestination, ShellOutcome, StopProgress};
use std::time::{Duration, Instant};
struct App;
impl Application for App {
    type Message = ();
    type LocalEvent = ();
    fn started(&mut self, _: &mut Context<'_, Self>) -> Result<(), Error> {
        Ok(())
    }
    fn stopping(&mut self, _: &mut Context<'_, Self>) -> StopProgress {
        StopProgress::Settled
    }
}
#[test]
fn context_validates_once_before_helper_admission_and_closed_precedes_invalid() {
    let mut core = Core::<App>::new().unwrap();
    core.shell = Some(
        ShellHelper::spawn(core.id, 1, || |_: &super::super::PreparedRequest| Ok(())).unwrap(),
    );
    let request = ShellRequest {
        destination: ShellDestination::Url("custom:unicode".into()),
    };
    VALIDATIONS.with(|count| count.set(0));
    PREPARATIONS.with(|count| count.set(0));
    let (receipt, counts) = crate::allocation_probe::measure(|| {
        Context::new(&mut core, BackendOps::Unavailable, ContextPhase::Startup)
            .request_shell(request)
            .unwrap()
    });
    assert_eq!(VALIDATIONS.with(|count| count.get()), 1);
    assert_eq!(PREPARATIONS.with(|count| count.get()), 1);
    assert_eq!(counts.allocations, 2);
    assert_eq!(counts.reallocations, 0);
    println!(
        "context shell validations=1 preparations=1 allocations={} requested_bytes={}",
        counts.allocations, counts.requested_bytes
    );
    core.request_stop(StopReason::Application);
    let invalid = ShellRequest {
        destination: ShellDestination::Url("invalid".into()),
    };
    VALIDATIONS.with(|count| count.set(0));
    let rejected = Context::new(&mut core, BackendOps::Unavailable, ContextPhase::Retirement)
        .request_shell(invalid.clone())
        .unwrap_err();
    assert_eq!(rejected.request, invalid);
    assert_eq!(rejected.reason, ShellFailure::Closed);
    assert_eq!(VALIDATIONS.with(|count| count.get()), 0);
    let helper = core.shell.as_mut().unwrap();
    helper.close();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !helper.poll_closed().unwrap() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(receipt.outcome(), ShellOutcome::Complete(Ok(())));
    core.shell = None;
}
#[test]
fn lazy_helper_startup_failure_returns_validated_original_without_preparation() {
    let mut core = Core::<App>::new().unwrap();
    core.shell_capacity = 0;
    let request = ShellRequest {
        destination: ShellDestination::Url("custom:destination".into()),
    };
    VALIDATIONS.with(|count| count.set(0));
    PREPARATIONS.with(|count| count.set(0));
    let rejected = Context::new(&mut core, BackendOps::Unavailable, ContextPhase::Startup)
        .request_shell(request.clone())
        .unwrap_err();
    assert_eq!(rejected.request, request);
    assert_eq!(
        rejected.reason,
        ShellFailure::InvalidInput("shell capacity must be positive")
    );
    assert_eq!(VALIDATIONS.with(|count| count.get()), 1);
    assert_eq!(PREPARATIONS.with(|count| count.get()), 0);
    assert!(core.shell.is_none());
    let invalid = ShellRequest {
        destination: ShellDestination::Url("invalid".into()),
    };
    let rejected = Context::new(&mut core, BackendOps::Unavailable, ContextPhase::Startup)
        .request_shell(invalid.clone())
        .unwrap_err();
    assert_eq!(rejected.request, invalid);
    assert_eq!(
        rejected.reason,
        ShellFailure::InvalidInput("URL needs a scheme")
    );
    assert!(core.shell.is_none());
}
