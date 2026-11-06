//! Private, statically dispatched native operations and window retention.

use crate::error::{Error, NativeOperation};
use crate::window::WindowSpec;

pub(crate) mod deadline;
pub(crate) mod wake;
pub(crate) mod windows;
pub(crate) mod winit;
pub(crate) mod winit_input;

#[cfg(test)]
#[path = "../../tests/common/backend.rs"]
pub(crate) mod test;

pub(crate) enum NativeWindow {
    Winit(std::sync::Arc<::winit::window::Window>),
    #[cfg(test)]
    Test(test::TestWindow),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeWindowKey {
    Winit(::winit::window::WindowId),
    #[cfg(test)]
    Test(u64),
}

impl NativeWindow {
    pub(crate) fn display_observed(&self) -> Result<crate::DisplayObserved, Error> {
        match self {
            Self::Winit(window) => crate::DisplayObserved::query(window),
            #[cfg(test)]
            Self::Test(window) => {
                if window.fail_observation.replace(false) {
                    crate::ScaleFactor::new(f64::NAN)?;
                }
                Ok(window.display.get())
            }
        }
    }

    pub(crate) fn monitors(
        &self,
        host: crate::HostId,
    ) -> Result<Vec<crate::MonitorSnapshot>, Error> {
        match self {
            Self::Winit(window) => crate::display::monitor_snapshots(host, window),
            #[cfg(test)]
            Self::Test(_) => Ok(crate::display::simulated_monitors(host)),
        }
    }

    pub(crate) fn apply_display(
        &self,
        host: crate::HostId,
        request: &crate::DisplayRequest,
        placement: Option<crate::WindowedPlacement>,
    ) -> Result<(), Error> {
        let monitors = self.monitors(host)?;
        request.validate_choices(host, &monitors)?;
        let placement = placement
            .map(|value| value.adjusted_for_monitors(&monitors))
            .transpose()?;
        match self {
            Self::Winit(window) => {
                let fullscreen = request.native_fullscreen(window)?;
                window.set_fullscreen(fullscreen);
                if let Some(placement) = placement {
                    window.set_maximized(false);
                    window.set_outer_position(::winit::dpi::PhysicalPosition::new(
                        placement.position.x,
                        placement.position.y,
                    ));
                    let _ = window.request_inner_size(::winit::dpi::PhysicalSize::new(
                        placement.size.width,
                        placement.size.height,
                    ));
                    window.set_maximized(placement.maximized);
                }
                Ok(())
            }
            #[cfg(test)]
            Self::Test(window) => {
                let mut observed = window.display.get();
                observed.backend_mode = request.mode();
                if let Some(placement) = placement {
                    observed.geometry.position = Some(placement.position);
                    observed.geometry.size = placement.size;
                    observed.geometry.maximized = placement.maximized;
                }
                window.display.set(observed);
                if window.fail_display.get() {
                    Err(Error::Native {
                        operation: NativeOperation::DisplayTransition,
                        message: String::from("simulated failure after mode mutation"),
                    })
                } else {
                    Ok(())
                }
            }
        }
    }

    pub(crate) fn request_redraw(&self) {
        match self {
            Self::Winit(window) => window.request_redraw(),
            #[cfg(test)]
            Self::Test(_) => (),
        }
    }

    pub(crate) fn geometry(&self) -> (crate::PhysicalSize, f64) {
        match self {
            Self::Winit(window) => {
                let size = window.inner_size();
                (
                    crate::PhysicalSize {
                        width: size.width,
                        height: size.height,
                    },
                    window.scale_factor(),
                )
            }
            #[cfg(test)]
            Self::Test(window) => {
                let observed = window.display.get();
                (observed.geometry.size, observed.geometry.scale.get())
            }
        }
    }

    pub(crate) fn confine(&self, confine: bool) -> Result<(), Error> {
        match self {
            Self::Winit(window) => window
                .set_cursor_grab(if confine {
                    ::winit::window::CursorGrabMode::Confined
                } else {
                    ::winit::window::CursorGrabMode::None
                })
                .map_err(|error| Error::Native {
                    operation: NativeOperation::ConfinePointer,
                    message: error.to_string(),
                }),
            #[cfg(test)]
            Self::Test(window) => window.confine(confine),
        }
    }

    pub(crate) fn cursor(&self, selection: &crate::CursorSelection) {
        match self {
            Self::Winit(window) => match selection {
                crate::CursorSelection::Default => {
                    window.set_cursor(::winit::window::CursorIcon::Default)
                }
                crate::CursorSelection::System(icon) => window.set_cursor(icon.native()),
                crate::CursorSelection::Custom(cursor) => window.set_cursor(cursor.inner.clone()),
            },
            #[cfg(test)]
            Self::Test(_) => (),
        }
    }

    pub(crate) fn cursor_visible(&self, visible: bool) {
        match self {
            Self::Winit(window) => window.set_cursor_visible(visible),
            #[cfg(test)]
            Self::Test(window) => window.cursor_visible(visible),
        }
    }

    pub(crate) fn text(&self, caret: Option<crate::TextCaret>) {
        match self {
            Self::Winit(window) => {
                if let Some(caret) = caret {
                    window.set_ime_cursor_area(
                        ::winit::dpi::PhysicalPosition::new(caret.position.x, caret.position.y),
                        ::winit::dpi::PhysicalSize::new(caret.size.width, caret.size.height),
                    );
                }
                window.set_ime_allowed(caret.is_some());
            }
            #[cfg(test)]
            Self::Test(_) => (),
        }
    }
    pub(crate) fn already_destroyed(self) {
        match self {
            Self::Winit(window) => {
                // Upstream Drop blindly posts destruction to its HWND. After
                // unexpected destruction, that value could alias a reused HWND.
                // The native window is gone; retain this fault-only wrapper to
                // avoid any second native access. Normal retirement never leaks.
                std::mem::forget(window);
            }
            #[cfg(test)]
            Self::Test(mut window) => window.already_destroyed = true,
        }
    }

    pub(crate) fn key(&self) -> NativeWindowKey {
        match self {
            Self::Winit(window) => NativeWindowKey::Winit(window.id()),
            #[cfg(test)]
            Self::Test(window) => NativeWindowKey::Test(window.id),
        }
    }
}

pub(crate) enum BackendOps<'a> {
    Winit(&'a ::winit::event_loop::ActiveEventLoop),
    Unavailable,
    #[cfg(test)]
    Test(&'a mut test::TestBackend),
}

impl BackendOps<'_> {
    pub(crate) fn raw_input(&mut self, policy: crate::RawInputPolicy) -> Result<(), Error> {
        use ::winit::platform::windows::ActiveEventLoopExtWindows;
        match self {
            Self::Winit(event_loop) => event_loop
                .try_listen_device_events(match policy {
                    crate::RawInputPolicy::Never => ::winit::event_loop::DeviceEvents::Never,
                    crate::RawInputPolicy::WhenFocused => {
                        ::winit::event_loop::DeviceEvents::WhenFocused
                    }
                })
                .map_err(|error| Error::Native {
                    operation: NativeOperation::RegisterRawInput,
                    message: error.to_string(),
                }),
            #[cfg(test)]
            Self::Test(backend) => backend.raw_input(policy),
            Self::Unavailable => Err(Error::Native {
                operation: NativeOperation::RegisterRawInput,
                message: String::from("native event loop unavailable"),
            }),
        }
    }

    pub(crate) fn create_cursor(
        &mut self,
        host: crate::HostId,
        input: crate::PreparedCursor,
    ) -> Result<crate::Cursor, crate::Rejected<crate::PreparedCursor>> {
        match self {
            Self::Winit(event_loop) => crate::cursor::create_native_cursor(event_loop, host, input),
            _ => {
                let reason = input.validate().err().unwrap_or_else(|| Error::Native {
                    operation: NativeOperation::CreateCursor,
                    message: String::from("native cursor creation unavailable"),
                });
                Err(crate::Rejected::new(input, reason))
            }
        }
    }
    pub(crate) fn create_window(&mut self, spec: &WindowSpec) -> Result<NativeWindow, Error> {
        match self {
            Self::Winit(event_loop) => {
                let attributes = ::winit::window::Window::default_attributes()
                    .with_title(&spec.title)
                    .with_inner_size(::winit::dpi::PhysicalSize::new(
                        spec.size.width,
                        spec.size.height,
                    ))
                    .with_visible(spec.visible);
                event_loop
                    .create_window(attributes)
                    .map(|window| NativeWindow::Winit(std::sync::Arc::new(window)))
                    .map_err(|error| Error::Native {
                        operation: NativeOperation::CreateWindow,
                        message: error.to_string(),
                    })
            }
            Self::Unavailable => Err(Error::Native {
                operation: NativeOperation::CreateWindow,
                message: String::from("native event loop unavailable"),
            }),
            #[cfg(test)]
            Self::Test(backend) => backend.create_window(spec).map(NativeWindow::Test),
        }
    }
}
