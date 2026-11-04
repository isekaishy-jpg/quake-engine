# Proposal 0009: the Sol crates as repository crates

Status: accepted and implemented, 2026-11-03, increment 0.

## Research

- DESIGN.md makes `solapp`, `solworker` and `solcache` part of this engine, editable here.
- They were copied from their own repositories at `solapp` `8bd81a0`, `solworker` `273b45b` and `solcache` `63105fe`.
- `solapp` needs `solworker` and `solcache` present, because its tests use them.
- Their own tests pass here: 524 passed, 0 failed, 7 ignored when first brought in.
- They pass default clippy with `-D warnings`. The workspace lints add about 800 warnings, mostly missing documentation, plus `unwrap` use and trivial casts.
- `solapp` keeps tests under `src/` through `#[path]` modules, which the style guide doesn't allow for engine code.

## Design

- The crates live under `crates/` as ordinary members, with `publish = false` and `rust-version = "1.98"`.
- Their per-crate toolchain pins, lockfiles and `.gitignore` files are removed. `[profile.bench]` moves to the root.
- Patched `winit` and `micropool` move to `vendor/`, keeping their licences and notes.
- They're named in `LINT_EXEMPT` for now, and the test-layout test skips them.
- Only `engine_platform` may use `solapp`.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** these crates are held to a lower lint bar for now, by a named exception. Fixing about 800 warnings isn't needed to start, and it's better done when we're working in each crate.
- **Acceptance criteria:** the crates pass default clippy and their own tests through `cargo xtask check`.
- **Future extension:** bringing them under the workspace lints is a recorded item in the research plan. Changes here don't flow back to their source repositories automatically.

## Review

`cargo xtask check` passes with the crates in the workspace.
