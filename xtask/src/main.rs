//! `cargo xtask`: repository automation.
//!
//! Commands:
//! - `check`: the required checks (Luau format, format, clippy, test);
//! - `fmt-luau`: formats Luau;
//! - `build-number`: shows the tracked build number and build identity;
//! - `reserve-build-number`: reserves the next package build number.

use std::env;
use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;

use xtask::build_number::BuildSequence;
use xtask::checks::run_required_checks;
use xtask::luau::{check_luau, format_luau};
use xtask::product::{ProductVersion, build_identity};
use xtask::workspace_root;

/// Usage text, printed for `help` and for unknown commands.
const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  check                  check Luau's format, then run cargo fmt --check, clippy
                         -D warnings and the tests
  fmt-luau               format Luau
  build-number           show the tracked build number and build identity
  reserve-build-number   reserve the next package build number
";

/// Runs the requested command and reports failures on standard error.
fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let result = match arguments.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["check"] => check(),
        ["fmt-luau"] => fmt_luau(),
        ["build-number"] => show_build_number(),
        ["reserve-build-number"] => reserve_build_number(),
        ["help" | "--help" | "-h"] => report(&mut io::stdout(), USAGE),
        _ => {
            let _ignored = report(&mut io::stderr(), USAGE);
            return ExitCode::FAILURE;
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ignored = report(&mut io::stderr(), &format!("xtask: {error}\n"));
            ExitCode::FAILURE
        }
    }
}

/// The error type of every command: each one is reported and ends the run.
type CommandError = Box<dyn std::error::Error>;

/// Writes `text` to `stream`. The library crates never print; this binary is
/// the reporting layer.
fn report(stream: &mut dyn Write, text: &str) -> Result<(), CommandError> {
    stream.write_all(text.as_bytes())?;
    stream.flush()?;
    Ok(())
}

/// The cargo that runs this command. `cargo xtask` passes it in `CARGO`, so
/// nested checks use the pinned toolchain; outside cargo, `cargo` on the path
/// is used, which also honours `rust-toolchain.toml`.
fn cargo() -> OsString {
    env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"))
}

/// Formats Luau.
fn fmt_luau() -> Result<(), CommandError> {
    let changed = format_luau(&workspace_root())?;
    report(&mut io::stdout(), &format!("formatted {changed} files\n"))
}

/// Runs the required checks.
fn check() -> Result<(), CommandError> {
    check_luau(&workspace_root())?;
    run_required_checks(&cargo(), &workspace_root())?;
    report(&mut io::stdout(), "xtask: all required checks passed\n")
}

/// The product version of this source tree.
fn product_version() -> Result<ProductVersion, CommandError> {
    Ok(ProductVersion::from_cargo(env!("CARGO_PKG_VERSION"))?)
}

/// Shows the tracked build number without reserving one.
fn show_build_number() -> Result<(), CommandError> {
    let sequence = BuildSequence::for_repository(&workspace_root())?;
    let identity = build_identity(product_version()?, sequence.current()?);
    report(&mut io::stdout(), &format!("{identity}\n"))
}

/// Reserves the next build number. The lock is released on return; a future
/// packaging command will hold it through compilation instead.
fn reserve_build_number() -> Result<(), CommandError> {
    let version = product_version()?;
    let sequence = BuildSequence::for_repository(&workspace_root())?;
    let lock = sequence.lock()?;
    let number = sequence.reserve(&lock)?;
    drop(lock);
    let identity = build_identity(version, number);
    report(
        &mut io::stdout(),
        &format!("reserved {identity}\nrecord BUILD_NUMBER in the package commit\n"),
    )
}
