use engine_host::{Application, Context, Error, Host, HostConfig, StopProgress};
use std::time::Duration;

struct NeverStarted;
impl Application for NeverStarted {
    type Message = String;
    type LocalEvent = u8;
    fn started(&mut self, _: &mut Context<'_, Self>) -> Result<(), Error> {
        panic!("invalid config must not acquire a loop")
    }
    fn stopping(&mut self, _: &mut Context<'_, Self>) -> StopProgress {
        unreachable!()
    }
}

#[test]
fn rejected_host_configuration_preserves_owned_input_before_native_loop_acquisition() {
    for interval in [Duration::ZERO, Duration::from_secs(2)] {
        let config = HostConfig {
            stop_poll_interval: interval,
            ..HostConfig::default()
        };
        let rejection = match Host::<NeverStarted>::new(config.clone()) {
            Ok(_) => panic!("invalid configuration accepted"),
            Err(rejection) => rejection,
        };
        let (returned, reason) = rejection.into_parts();
        assert_eq!(returned, config);
        assert!(matches!(reason, Error::InvalidInput(_)));
    }
}
