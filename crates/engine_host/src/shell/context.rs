//! Host admission and final access for association opening.

use super::{
    ShellFailure, ShellHelper, ShellReceipt, ShellRejected, ShellRequest, ValidatedRequest,
};
use crate::host::{Application, Core, StopReason};
use crate::{Context, Error};

impl<A: Application> Context<'_, A> {
    /// Opens an application-selected association synchronously on the owner.
    ///
    /// This may block in Windows or an installed association handler and may
    /// enter native modal processing. It does not pump nested application work.
    /// Use `request_shell` when the owner must remain responsive. Success means
    /// native association acceptance, not external process start or exit.
    ///
    /// # Errors
    ///
    /// Returns an error if the request isn't valid, or the system won't open it.
    pub fn open_shell(&mut self, request: &ShellRequest) -> Result<(), ShellFailure> {
        self.core
            .check_owner()
            .map_err(|_| ShellFailure::WrongThread)?;
        if !self.core.ordinary_open() {
            return Err(ShellFailure::Closed);
        }
        let destination = request.prepare()?;
        crate::backend::windows::shell::Apartment::new()?.launch(&destination)
    }

    /// Accepts an owned association request on the bounded dedicated helper.
    /// Rejection returns the original request; an accepted receipt survives
    /// host closure without retaining the application or a native window.
    ///
    /// # Errors
    ///
    /// Hands the request back, with why, if the helper won't take it.
    ///
    /// # Panics
    ///
    /// Panics if the helper just made has gone, which is a bug in this crate.
    pub fn request_shell(&mut self, request: ShellRequest) -> Result<ShellReceipt, ShellRejected> {
        let admission = self
            .core
            .check_owner()
            .map_err(|_| ShellFailure::WrongThread)
            .and_then(|()| {
                if self.core.ordinary_open() {
                    Ok(())
                } else {
                    Err(ShellFailure::Closed)
                }
            });
        if let Err(reason) = admission {
            return Err(ShellRejected { request, reason });
        }
        let request = ValidatedRequest::new(request)?;
        if self.core.shell.is_none() {
            match ShellHelper::new(self.core.id, self.core.shell_capacity) {
                Ok(helper) => self.core.shell = Some(helper),
                Err(reason) => return Err(request.reject(reason)),
            }
        }
        self.core
            .shell
            .as_ref()
            .expect("the helper just made")
            .try_launch_validated(request)
    }

    /// Number of accepted requests whose native call or payload reclamation
    /// has not finished. Zero alone does not prove helper apartment retirement.
    pub fn shell_pending(&self) -> usize {
        self.core.shell.as_ref().map_or(0, ShellHelper::pending)
    }
}

impl<A: Application> Core<A> {
    /// Retirement is polled independently of closed ordinary post admission.
    pub(crate) fn poll_shell_retirement(&mut self) -> bool {
        let Some(helper) = &mut self.shell else {
            return true;
        };
        helper.close();
        match helper.poll_closed() {
            Ok(true) => {
                self.shell = None;
                true
            }
            Ok(false) => false,
            Err(error) => {
                // A crashed helper must not fabricate terminal receipts or
                // allow pending native final access to be treated as settled.
                self.fail(Error::Shell(error), StopReason::BackendFailed);
                crate::host::fail_stop("shell helper failed before final access was established")
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/shell_context.rs"]
mod tests;

#[cfg(test)]
#[expect(clippy::print_stdout, reason = "a test reports to its console")]
#[path = "../../tests/unit/shell_validation.rs"]
mod validation_tests;
