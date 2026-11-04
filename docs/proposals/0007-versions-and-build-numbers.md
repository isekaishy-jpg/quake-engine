# Proposal 0007: versions and build numbers

Status: accepted and implemented, 2026-11-03, increment 0.

## Research

- DESIGN.md sets product versions as `major.minor.patch` plus `a`, `b`, `rc` or `s`, and a separate six-digit build number that's reserved before compiling and never reused.
- Cargo needs SemVer, and populous-reborn spells stages as `-alpha`, `-beta`, `-rc` and none for stable.
- Its reservation scheme uses a tracked `BUILD_NUMBER`, a high-water mark in Git's common directory that survives branch switches, and an exclusive lock file. Its proposal 0007 records the file-system reasoning, and the code has tests.

## Design

Adopted as it stands, with the files named `quake-engine-build-number` and `quake-engine-build.lock`. `BUILD_NUMBER` starts at 0, the state before the first package. `cargo xtask build-number` shows the identity, and `cargo xtask reserve-build-number` reserves the next number. The starting product version is `0.0.0a`.

## Proposal

- **Selected approach:** the populous-reborn scheme and code.
- **Tradeoffs:** a crashed reservation leaves a lock that must be deleted by hand, because taking it over automatically could reuse a number.
- **Acceptance criteria:** the carried-over tests pass.
- **Future extension:** the working name appears in the build identity and file names, and changes with the product name.

## Review

`cargo xtask build-number` is covered by tests. No packaged build exists yet.
