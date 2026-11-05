use super::*;
use crate::identity::{WindowGeneration, WindowSlots};

fn target() -> WindowTarget {
    let mut slots = WindowSlots::<()>::new(HostId::allocate().unwrap());
    WindowTarget {
        id: slots.reserve().unwrap(),
        generation: WindowGeneration::INITIAL,
    }
}

fn placement() -> WindowedPlacement {
    WindowedPlacement {
        position: ScreenPosition { x: -700, y: 32 },
        size: PhysicalSize {
            width: 640,
            height: 480,
        },
        maximized: false,
    }
}

fn observed(mode: DisplayMode) -> DisplayObserved {
    DisplayObserved {
        backend_mode: mode,
        geometry: DisplayGeometry {
            position: Some(placement().position),
            size: placement().size,
            scale: ScaleFactor::new(1.25).unwrap(),
            minimized: Some(false),
            maximized: false,
        },
    }
}

fn monitor(host: HostId) -> MonitorSnapshot {
    let id = MonitorId {
        host,
        selection: MonitorSelection::Simulated(1),
    };
    MonitorSnapshot {
        id: id.clone(),
        name: Some(String::from("simulated monitor")),
        position: ScreenPosition { x: 0, y: 0 },
        size: PhysicalSize {
            width: 1920,
            height: 1080,
        },
        scale: ScaleFactor::new(1.0).unwrap(),
        video_modes: vec![VideoMode {
            monitor: id,
            native: None,
            size: PhysicalSize {
                width: 1920,
                height: 1080,
            },
            bit_depth: 32,
            refresh_millihertz: 59940,
        }],
    }
}

fn request(mode: DisplayMode, host: HostId) -> DisplayRequest {
    let monitor = monitor(host);
    match mode {
        DisplayMode::Windowed => DisplayRequest::Windowed { placement: None },
        DisplayMode::Borderless => DisplayRequest::Borderless {
            monitor: monitor.id,
        },
        DisplayMode::Exclusive => DisplayRequest::Exclusive {
            mode: monitor.video_modes[0].clone(),
        },
    }
}

#[test]
fn all_six_directed_transitions_require_exact_presentation_report() {
    let modes = [
        DisplayMode::Windowed,
        DisplayMode::Borderless,
        DisplayMode::Exclusive,
    ];
    for from in modes {
        for to in modes {
            if from == to {
                continue;
            }
            let target = target();
            let mut state = DisplayState::new(target, observed(from));
            let receipt = state
                .request(request(to, target.id.host()))
                .unwrap()
                .receipt;
            assert_eq!(state.observed().backend_mode, from);
            assert_eq!(
                state.pending().unwrap().state,
                DisplayTransitionState::Requested
            );
            let apply = state.start_next().unwrap();
            assert_eq!(apply.transition.receipt, receipt);
            assert_eq!(receipt.transition.target(), target);
            assert_eq!(
                state.active().unwrap().state,
                DisplayTransitionState::Applying
            );
            assert_eq!(
                state.report(receipt, PresentationStatus::Ready),
                Err(Error::StaleIdentity)
            );
            state.applied(receipt, observed(to)).unwrap();
            assert_eq!(
                state.active().unwrap().state,
                DisplayTransitionState::AwaitingPresentation
            );
            assert!(state.settled().is_none());
            assert_eq!(
                state
                    .report(receipt, PresentationStatus::Ready)
                    .unwrap()
                    .state,
                DisplayTransitionState::Ready
            );
            assert_eq!(state.observed().backend_mode, to);
            assert!(state.active().is_none());
        }
    }
}

#[test]
fn rapid_reversal_supersedes_only_pending_and_preserves_applying_obligations() {
    let target = target();
    let host = target.id.host();
    let mut state = DisplayState::new(target, observed(DisplayMode::Windowed));
    let first = state
        .request(request(DisplayMode::Borderless, host))
        .unwrap()
        .receipt;
    assert!(state.start_next().is_some());
    let second = state
        .request(request(DisplayMode::Exclusive, host))
        .unwrap()
        .receipt;
    let reversal = state.request(request(DisplayMode::Windowed, host)).unwrap();
    let superseded = reversal.superseded.unwrap();
    assert_eq!(superseded.receipt, second);
    assert_eq!(superseded.state, DisplayTransitionState::Superseded);
    assert!(state.start_next().is_none());
    assert_eq!(state.active().unwrap().receipt, first);
    state
        .applied(first, observed(DisplayMode::Borderless))
        .unwrap();
    state.report(first, PresentationStatus::Ready).unwrap();
    let apply = state.start_next().unwrap();
    assert_eq!(apply.transition.receipt, reversal.receipt);
    assert_eq!(apply.windowed_placement, Some(placement()));
    state
        .applied(reversal.receipt, observed(DisplayMode::Windowed))
        .unwrap();
    assert!(state.saved_windowed.is_none());
    assert_eq!(
        state.report(first, PresentationStatus::Ready),
        Err(Error::StaleIdentity)
    );
    let mut stale_revision = reversal.receipt;
    stale_revision.presentation_revision = first.presentation_revision;
    assert_eq!(
        state.report(stale_revision, PresentationStatus::Ready),
        Err(Error::StaleIdentity)
    );
    state
        .report(reversal.receipt, PresentationStatus::Ready)
        .unwrap();
}

#[test]
fn stale_target_failure_and_retirement_do_not_complete_newer_transition() {
    let target = target();
    let mut state = DisplayState::new(target, observed(DisplayMode::Windowed));
    let first = state
        .request(request(DisplayMode::Borderless, target.id.host()))
        .unwrap()
        .receipt;
    state.start_next().unwrap();
    let mut wrong_target = first;
    wrong_target.transition.target = self::target();
    assert_eq!(
        state.applied(wrong_target, observed(DisplayMode::Borderless)),
        Err(Error::StaleIdentity)
    );
    let reason = Error::application("simulated native acquisition failure");
    assert_eq!(
        state.fail(first, reason.clone()).unwrap().state,
        DisplayTransitionState::Failed(reason)
    );
    assert_eq!(state.observed().backend_mode, DisplayMode::Windowed);
    let next = state
        .request(request(DisplayMode::Exclusive, target.id.host()))
        .unwrap()
        .receipt;
    state.start_next().unwrap();
    state
        .applied(next, observed(DisplayMode::Exclusive))
        .unwrap();
    let pending = state
        .request(request(DisplayMode::Windowed, target.id.host()))
        .unwrap()
        .receipt;
    assert_eq!(state.begin_retirement().unwrap().receipt, pending);
    assert!(matches!(
        state.request(request(DisplayMode::Windowed, target.id.host())),
        Err(Error::AdmissionClosed)
    ));
    assert_eq!(state.active().unwrap().receipt, next);
    assert_eq!(
        state.report(first, PresentationStatus::Ready),
        Err(Error::StaleIdentity)
    );
    assert!(
        state
            .report(
                next,
                PresentationStatus::Failed(Error::application("simulated SR failure"))
            )
            .is_ok()
    );
}

#[test]
fn selection_loss_foreign_host_and_invalid_geometry_fail_before_admission() {
    let target = target();
    let snapshot = monitor(target.id.host());
    let borderless = DisplayRequest::Borderless {
        monitor: snapshot.id.clone(),
    };
    let exclusive = DisplayRequest::Exclusive {
        mode: snapshot.video_modes[0].clone(),
    };
    let current = vec![snapshot.clone()];
    assert!(
        borderless
            .validate_choices(target.id.host(), &current)
            .is_ok()
    );
    assert!(
        exclusive
            .validate_choices(target.id.host(), &current)
            .is_ok()
    );
    assert_eq!(
        borderless.validate_choices(target.id.host(), &[]),
        Err(Error::StaleIdentity)
    );
    let mut missing_mode = snapshot;
    missing_mode.video_modes.clear();
    assert_eq!(
        exclusive.validate_choices(target.id.host(), &[missing_mode]),
        Err(Error::StaleIdentity)
    );
    let mut state = DisplayState::new(target, observed(DisplayMode::Windowed));
    assert!(matches!(
        state.request(request(
            DisplayMode::Borderless,
            HostId::allocate().unwrap()
        )),
        Err(Error::ForeignHost)
    ));
    let invalid = WindowedPlacement {
        size: PhysicalSize {
            width: 0,
            height: 1,
        },
        ..placement()
    };
    assert!(
        state
            .request(DisplayRequest::Windowed {
                placement: Some(invalid)
            })
            .is_err()
    );
    assert!(state.pending().is_none());
    assert_eq!(state.next_serial, 1);
    assert_eq!(
        ScaleFactor::new(f64::NAN),
        Err(Error::InvalidInput("scale must be positive and finite"))
    );
}

#[test]
fn monitor_loss_adjusts_saved_position_without_inventing_monitor_or_mode() {
    let snapshot = monitor(HostId::allocate().unwrap());
    let adjusted = placement()
        .adjusted_for_monitors(std::slice::from_ref(&snapshot))
        .unwrap();
    assert_eq!(adjusted.position, ScreenPosition { x: 0, y: 32 });
    assert_eq!(adjusted.size, placement().size);
    assert_eq!(
        adjusted.adjusted_for_monitors(&[snapshot]).unwrap(),
        adjusted
    );
    assert_eq!(
        placement().adjusted_for_monitors(&[]),
        Err(Error::StaleIdentity)
    );
}

#[test]
fn checked_ids_and_backend_mismatch_preserve_inflight_transition() {
    let target = target();
    let mut state = DisplayState::new(target, observed(DisplayMode::Windowed));
    let receipt = state
        .request(request(DisplayMode::Borderless, target.id.host()))
        .unwrap()
        .receipt;
    state.start_next().unwrap();
    state
        .applied(receipt, observed(DisplayMode::Windowed))
        .unwrap();
    assert!(matches!(
        state.report(receipt, PresentationStatus::Ready),
        Err(Error::InvalidInput(_))
    ));
    assert_eq!(state.active().unwrap().receipt, receipt);
    state.next_serial = u64::MAX;
    assert!(matches!(
        state.request(request(DisplayMode::Windowed, target.id.host())),
        Err(Error::IdentityExhausted(IdentityKind::DisplayTransition))
    ));
    state.next_serial = 2;
    state.next_revision = u64::MAX;
    assert!(matches!(
        state.request(request(DisplayMode::Windowed, target.id.host())),
        Err(Error::IdentityExhausted(IdentityKind::Presentation))
    ));
    assert!(state.pending().is_none());
    state.observe(observed(DisplayMode::Borderless));
    state.report(receipt, PresentationStatus::Ready).unwrap();
}

#[test]
fn implicit_maximized_return_preserves_backend_normal_restore_rectangle() {
    let target = target();
    let mut maximized = observed(DisplayMode::Windowed);
    maximized.geometry.maximized = true;
    let mut state = DisplayState::new(target, maximized);
    let fullscreen = state
        .request(request(DisplayMode::Borderless, target.id.host()))
        .unwrap()
        .receipt;
    state.start_next().unwrap();
    assert!(state.saved_windowed.unwrap().maximized);
    state
        .applied(fullscreen, observed(DisplayMode::Borderless))
        .unwrap();
    state.report(fullscreen, PresentationStatus::Ready).unwrap();
    let restore = state
        .request(request(DisplayMode::Windowed, target.id.host()))
        .unwrap()
        .receipt;
    assert!(state.start_next().unwrap().windowed_placement.is_none());
    state.applied(restore, maximized).unwrap();
    state.report(restore, PresentationStatus::Ready).unwrap();
    let explicit = WindowedPlacement {
        maximized: true,
        ..placement()
    };
    state
        .request(DisplayRequest::Windowed {
            placement: Some(explicit),
        })
        .unwrap();
    assert_eq!(
        state.start_next().unwrap().windowed_placement,
        Some(explicit)
    );
}

#[test]
fn failed_fullscreen_entries_refresh_restore_from_each_observed_windowed_start() {
    for mode in [DisplayMode::Borderless, DisplayMode::Exclusive] {
        for maximized in [false, true] {
            let target = target();
            let host = target.id.host();
            let mut state = DisplayState::new(target, observed(DisplayMode::Windowed));
            for attempt in 0..3 {
                let failed = state.request(request(mode, host)).unwrap().receipt;
                state.start_next().unwrap();
                state
                    .fail(failed, Error::application("failed native entry"))
                    .unwrap();
                let mut moved = observed(DisplayMode::Windowed);
                moved.geometry.position = Some(ScreenPosition {
                    x: 150 + attempt * 20,
                    y: 100,
                });
                moved.geometry.size = PhysicalSize {
                    width: 900,
                    height: 700,
                };
                moved.geometry.maximized = maximized;
                state.observe(moved);
            }
            let newer = state.observed().windowed_placement().unwrap();
            // Superseding an unstarted request must not snapshot that request.
            let superseded = state.request(request(mode, host)).unwrap().receipt;
            let entry = state.request(request(mode, host)).unwrap();
            assert_eq!(entry.superseded.unwrap().receipt, superseded);
            state.start_next().unwrap();
            assert_eq!(state.saved_windowed, Some(newer));
            state.applied(entry.receipt, observed(mode)).unwrap();
            state
                .report(entry.receipt, PresentationStatus::Ready)
                .unwrap();
            let other = if mode == DisplayMode::Borderless {
                DisplayMode::Exclusive
            } else {
                DisplayMode::Borderless
            };
            let switching = state.request(request(other, host)).unwrap().receipt;
            state.start_next().unwrap();
            assert_eq!(state.saved_windowed, Some(newer));
            state.applied(switching, observed(other)).unwrap();
            state.report(switching, PresentationStatus::Ready).unwrap();
            state.request(request(DisplayMode::Windowed, host)).unwrap();
            assert_eq!(
                state.start_next().unwrap().windowed_placement,
                (!maximized).then_some(newer)
            );
        }
    }
}

#[test]
fn failure_after_native_fullscreen_entry_preserves_restore_and_explicit_precedence() {
    for mode in [DisplayMode::Borderless, DisplayMode::Exclusive] {
        let target = target();
        let host = target.id.host();
        let mut state = DisplayState::new(target, observed(DisplayMode::Windowed));
        let entry = state.request(request(mode, host)).unwrap().receipt;
        state.start_next().unwrap();
        state.applied(entry, observed(mode)).unwrap();
        state
            .report(
                entry,
                PresentationStatus::Failed(Error::application("SR failed after native entry")),
            )
            .unwrap();
        assert_eq!(state.observed().backend_mode, mode);
        assert_eq!(state.saved_windowed, Some(placement()));
        state.request(request(mode, host)).unwrap();
        let retry = state.start_next().unwrap().transition.receipt;
        assert_eq!(state.saved_windowed, Some(placement()));
        // A native failure observed while already fullscreen also retains restore state.
        state
            .fail(retry, Error::application("native retry failed"))
            .unwrap();
        state.request(request(DisplayMode::Windowed, host)).unwrap();
        let restore = state.start_next().unwrap();
        assert_eq!(restore.windowed_placement, Some(placement()));
        state
            .fail(
                restore.transition.receipt,
                Error::application("restore failed"),
            )
            .unwrap();
        let explicit = WindowedPlacement {
            position: ScreenPosition { x: 42, y: 70 },
            ..placement()
        };
        state
            .request(DisplayRequest::Windowed {
                placement: Some(explicit),
            })
            .unwrap();
        assert_eq!(
            state.start_next().unwrap().windowed_placement,
            Some(explicit)
        );
    }
}
