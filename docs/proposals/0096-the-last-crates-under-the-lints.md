# Proposal 0096: the last crates under the lints

Status: accepted and implemented, 2026-11-05, increment 12.

## Research

- `engine_host`, `engine_worker`, `engine_cache` and its worker example came from outside with their own standard, and proposal 0009 left them out of the workspace lints.
- Turned on, the lints gave 1,684 warnings and 47 errors: about 470 doc comments, 220 `# Errors` and `# Panics` sections, 490 `unwrap`s, 125 needless qualifications, 50 `unsafe` blocks with no safety comment, 47 uses of `unsafe` not allowed by name, 44 needlessly public items, and 16 modules in `x.rs` beside a folder `x/`.
- Most `unwrap`s were outside `src`, in tests, examples and benches. `clippy.toml` allows them in `#[test]` functions and test modules, but not in the helpers of an integration test or anything with its own `main`.
- 41 files under those crates' `src` hold `#[cfg(test)]` code, and much of it reaches private seams. Moving that is a different job from linting it, and would mean opening those seams.

## Design

- All four inherit the workspace lints, and `LINT_EXEMPT` is gone.
- `unsafe` is allowed by name, on the module or function that needs it, with a reason. Every block has a `// SAFETY:` comment saying what makes it sound. The ones in tests and examples are all Win32 calls on the test's own windows, or the external resource contract.
- In `src`, `unwrap` became `expect` with what was being relied on, which panics in the same cases. Tests, examples and benches expect `clippy::unwrap_used` and the print lints at their root, with the reason `clippy.toml` already gives: a failed test is the intended report.
- Docs are written by hand. A function's `# Panics` says what's relied on, and nearly all are bugs in the crate rather than something a caller can do.
- Modules move to `x/mod.rs`, and `#[path]`s follow.
- The test layout rule keeps its own short list, `INLINE_TESTS`, as that's the part still left.

## Proposal

- **Selected approach:** the above. Nothing changes what the code does, apart from what a panic says.
- **Tradeoffs:** the doc comments are short, and say what a thing is rather than the contract round it, where the existing longer comments already do that.
- **Acceptance criteria:** `cargo xtask check` passes with all four under the lints, and `LINT_EXEMPT` no longer exists.
- **Future extension:** moving those crates' tests out of `src`, which needs a decision on their private seams.

## Review

The `cargo clippy --fix` pass and every hand edit were checked by the full suite.
