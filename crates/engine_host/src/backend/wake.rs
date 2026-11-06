//! Serialize native wake posting with final event-loop-window access.

use std::sync::{Arc, Mutex, MutexGuard};

use crate::{Error, NativeOperation};

struct State {
    closed: bool,
    proxy: Option<winit::event_loop::EventLoopProxy<()>>,
    #[cfg(test)]
    send: Option<Arc<dyn Fn() -> Result<(), Error> + Send + Sync>>,
}

/// Every `engine_host` native wake destination shares this gate. Holding its private lock
/// through send covers winit's channel-enqueue THEN PostMessage interval. It
/// invokes no application/provider callback. Close waits for admitted sends.
#[derive(Clone)]
pub(crate) struct NativeWake {
    state: Arc<Mutex<State>>,
}

impl NativeWake {
    /// Construct before native acquisition. The no-proxy state supports the
    /// deterministic backend; no production application starts before install.
    pub(crate) fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                closed: false,
                proxy: None,
                #[cfg(test)]
                send: None,
            })),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Install exactly once before publishing native-capable destinations.
    pub(crate) fn install(
        &self,
        proxy: winit::event_loop::EventLoopProxy<()>,
    ) -> Result<(), Error> {
        let mut state = self.lock();
        if state.closed {
            return Err(Error::AdmissionClosed);
        }
        if state.proxy.is_some() {
            return Err(Error::AlreadyRun);
        }
        state.proxy = Some(proxy);
        Ok(())
    }

    /// A durable source owns progress; this is only its native scheduling hint.
    pub(crate) fn post(&self) -> Result<(), Error> {
        let state = self.lock();
        if state.closed {
            return Err(Error::AdmissionClosed);
        }
        #[cfg(test)]
        if let Some(send) = &state.send {
            return send();
        }
        match &state.proxy {
            Some(proxy) => proxy.send_event(()).map_err(|error| Error::Native {
                operation: NativeOperation::Wake,
                message: error.to_string(),
            }),
            None => Ok(()),
        }
    }

    /// After return, no admitted send can still touch the backend wake HWND,
    /// and surviving clones cannot begin another native send.
    pub(crate) fn close(&self) {
        let mut state = self.lock();
        state.closed = true;
        state.proxy = None;
    }

    /// Declare after acquiring the actual event-loop owner, and run the loop
    /// through a borrowed API. Reverse local destruction then closes this gate
    /// before that owner drops, including native errors and Rust unwinding.
    pub(crate) fn close_guard(&self) -> NativeWakeCloseGuard {
        NativeWakeCloseGuard(self.clone())
    }
}

pub(crate) struct NativeWakeCloseGuard(NativeWake);

impl Drop for NativeWakeCloseGuard {
    fn drop(&mut self) {
        self.0.close();
    }
}

#[cfg(test)]
#[path = "../../tests/unit/wake.rs"]
mod tests;
