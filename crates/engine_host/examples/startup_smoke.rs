//! Main-process native startup/cleanup witness, with no interactive window.
#![expect(clippy::unwrap_used, reason = "an example keeps to what it shows")]
#![expect(clippy::print_stdout, reason = "an example reports to its console")]

use engine_host::{
    Application, Context, Error, Host, HostConfig, StopProgress, WindowSpec, WindowState,
    WindowTarget,
};

struct Smoke {
    window: Option<WindowTarget>,
    polls: usize,
    fail: bool,
}

impl Application for Smoke {
    type Message = String;
    type LocalEvent = u8;
    fn started(&mut self, cx: &mut Context<'_, Self>) -> Result<(), Error> {
        self.window = Some(
            cx.create_window(WindowSpec {
                visible: false,
                ..WindowSpec::default()
            })
            .map_err(|rejected| rejected.into_parts().1)?,
        );
        println!("native window acquired");
        if self.fail {
            return Err(Error::application("injected startup failure"));
        }
        cx.request_stop();
        Ok(())
    }

    fn stopping(&mut self, cx: &mut Context<'_, Self>) -> StopProgress {
        let Some(window) = self.window else {
            return StopProgress::Settled;
        };
        assert_eq!(cx.window_state(window), Ok(WindowState::Retiring));
        self.polls += 1;
        if self.polls < 3 {
            StopProgress::Pending
        } else {
            println!("application cleanup settled");
            StopProgress::Settled
        }
    }
}

fn main() {
    let fail = std::env::args().any(|argument| argument == "--fail-startup");
    let mut app = Smoke {
        window: None,
        polls: 0,
        fail,
    };
    let mut host = Host::<Smoke>::new(HostConfig::default()).expect("native event loop");
    let result = host.run(&mut app);
    assert_eq!(app.polls, 3);
    if fail {
        assert_eq!(result, Err(Error::application("injected startup failure")));
    } else {
        println!("{result:?}");
        assert_eq!(result.unwrap().windows_retired, 1);
    }
    assert_eq!(host.state(), engine_host::HostState::Closed);
    assert_eq!(host.run(&mut app), Err(Error::AlreadyRun));
    println!("native smoke completed");
}
