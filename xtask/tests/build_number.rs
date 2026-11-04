//! Build number reservation: monotonic, recorded before compiling, and never
//! reused across branch switches, fresh clones or concurrent attempts.

use std::fs;
use std::path::{Path, PathBuf};

use xtask::build_number::{BuildNumber, BuildNumberError, BuildSequence};

/// A fresh, empty directory for one test under Cargo's test scratch area.
fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("build_number")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clear scratch directory");
    }
    fs::create_dir_all(dir.join("shared")).expect("create scratch directory");
    dir
}

/// A sequence in `dir` whose tracked file starts as `tracked`.
fn sequence(dir: &Path, tracked: &str) -> BuildSequence {
    let tracked_path = dir.join("BUILD_NUMBER");
    fs::write(&tracked_path, tracked).expect("write tracked number");
    BuildSequence::new(&tracked_path, &dir.join("shared"))
}

/// Reserves one number, taking and releasing the lock.
fn reserve(sequence: &BuildSequence) -> Result<BuildNumber, BuildNumberError> {
    let lock = sequence.lock()?;
    sequence.reserve(&lock)
}

#[test]
fn reservations_increase_by_one_and_update_both_records() {
    let dir = scratch("increase");
    let sequence = sequence(&dir, "0\n");
    assert_eq!(
        sequence.current().expect("current"),
        BuildNumber::UNNUMBERED
    );
    assert_eq!(reserve(&sequence).expect("first"), BuildNumber::new(1));
    assert_eq!(reserve(&sequence).expect("second"), BuildNumber::new(2));
    assert_eq!(sequence.current().expect("current"), BuildNumber::new(2));
    let high_water =
        fs::read_to_string(dir.join("shared/quake-engine-build-number")).expect("high-water mark");
    assert_eq!(high_water, "2\n");
    // Records are replaced through a temporary file that must not remain.
    assert!(!dir.join("BUILD_NUMBER.tmp").exists());
    assert!(!dir.join("shared/quake-engine-build-number.tmp").exists());
}

#[test]
fn older_branch_does_not_reuse_a_number() {
    let dir = scratch("older_branch");
    let sequence = sequence(&dir, "7\n");
    assert_eq!(reserve(&sequence).expect("reserve"), BuildNumber::new(8));
    // Switching to an older branch restores an older tracked number.
    fs::write(dir.join("BUILD_NUMBER"), "3\n").expect("older tracked number");
    assert_eq!(reserve(&sequence).expect("reserve"), BuildNumber::new(9));
}

#[test]
fn fresh_clone_continues_from_the_tracked_number() {
    let dir = scratch("fresh_clone");
    let sequence = sequence(&dir, "41\r\n");
    assert_eq!(reserve(&sequence).expect("reserve"), BuildNumber::new(42));
}

#[test]
fn held_lock_blocks_a_second_reservation() {
    let dir = scratch("locked");
    let sequence = sequence(&dir, "0\n");
    let held = sequence.lock().expect("first lock");
    assert!(matches!(
        sequence.lock(),
        Err(BuildNumberError::Locked { .. })
    ));
    assert_eq!(
        sequence.reserve(&held).expect("reserve"),
        BuildNumber::new(1)
    );
    drop(held);
    assert_eq!(
        reserve(&sequence).expect("after release"),
        BuildNumber::new(2)
    );
}

#[test]
fn lock_of_another_sequence_is_refused() {
    let first = sequence(&scratch("foreign_lock_a"), "0\n");
    let second = sequence(&scratch("foreign_lock_b"), "0\n");
    let lock = first.lock().expect("lock");
    assert!(matches!(
        second.reserve(&lock),
        Err(BuildNumberError::ForeignLock { .. })
    ));
}

#[test]
fn malformed_records_are_reported_not_guessed() {
    for (index, text) in ["", "\n", "x\n", "-1\n", "1 2\n", "4294967296\n"]
        .into_iter()
        .enumerate()
    {
        let dir = scratch(&format!("malformed_{index}"));
        let sequence = sequence(&dir, text);
        assert!(
            matches!(reserve(&sequence), Err(BuildNumberError::Malformed { .. })),
            "{text:?}"
        );
    }
}

#[test]
fn exhausted_sequence_is_reported() {
    let dir = scratch("exhausted");
    let sequence = sequence(&dir, "4294967295\n");
    assert!(matches!(
        reserve(&sequence),
        Err(BuildNumberError::Exhausted)
    ));
}

#[test]
fn repository_build_number_is_readable() {
    let sequence = BuildSequence::for_repository(&xtask::workspace_root()).expect("git repository");
    sequence.current().expect("tracked BUILD_NUMBER");
}
