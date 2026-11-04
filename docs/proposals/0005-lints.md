# Proposal 0005: the lint set

Status: accepted and implemented, 2026-11-03, increment 0.

## Research

- DESIGN.md requires zero warnings. populous-reborn enables lints one by one at workspace level, never whole groups, and records why for each.
- Cargo's `[workspace.lints]` applies only to packages that write `[lints] workspace = true`, so inheritance needs a test.
- `#[expect(lint, reason = "...")]` warns when a suppression goes stale.
- Those lints fit here for the same reasons, with one difference: casts matter because Quake's data formats and network protocol are full of narrowing conversions.
- Applying them to the Sol crates produces about 800 warnings, mostly missing documentation. That's real work, covered in [proposal 0009](0009-sol-crates.md).

## Design

Rust lints, all `warn` except `unsafe_code`: `missing_docs`, `unsafe_code` (deny), `unreachable_pub`, `elided_lifetimes_in_paths`, `unused_lifetimes`, `trivial_casts`, `trivial_numeric_casts`, `unused_qualifications`, `unused_import_braces`.

Clippy lints, all `warn`: `cast_possible_truncation`, `cast_possible_wrap`, `cast_sign_loss`, `cast_lossless`, `allow_attributes`, `allow_attributes_without_reason`, `unwrap_used`, `todo`, `unimplemented`, `dbg_macro`, `print_stdout`, `print_stderr`, `missing_errors_doc`, `missing_panics_doc`, `undocumented_unsafe_blocks`, `self_named_module_files`, `wildcard_imports`.

`clippy.toml` allows `unwrap` and `expect` in tests. The required clippy run uses `-D warnings`.

## Proposal

- **Selected approach:** the lints above, inherited by every engine package.
- **Tradeoffs:** the cast lints make arithmetic longer, in exchange for visible conversions.
- **Acceptance criteria:** checks pass with zero warnings, and a permanent test confirms inheritance.
- **Future extension:** adding or removing a lint is its own item. Generated code gets a narrow exception in its own module.

## Review

The lints fire: an undocumented function with a narrowing cast added to `engine_core` failed clippy. The Sol crates are exempt through a named list, not silently.
