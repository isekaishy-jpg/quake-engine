# Proposal 0002: the Rust toolchain pin

Status: accepted and implemented, 2026-11-03, increment 0.

## Research

- The required checks need clippy and rustfmt, and the Sol crates need Rust 1.98 (`solapp` declares `rust-version = "1.98"`).
- `solworker` and `solcache` pinned 1.98.1, and `solapp` pinned 1.98.0. populous-reborn pins 1.98.0, and that record notes a toolchain change can change lint results.
- 1.98.0 and 1.98.1 are both installed here.

| Option | For | Against |
|---|---|---|
| `1.98.1` exact | Newest of the crates' pins, reproducible, and every crate here passes on it | Differs from populous-reborn, which doesn't share this workspace |
| `1.98.0` exact | Matches populous-reborn | Older than two of the three Sol crates' pins |
| `stable` channel | Always current | Not reproducible |

## Design

`rust-toolchain.toml` names channel `1.98.1`, profile `minimal`, components `clippy` and `rustfmt`. The workspace declares `rust-version = "1.98"`.

## Proposal

- **Selected approach:** pin `1.98.1` exactly.
- **Tradeoffs:** a point release needs an explicit bump, which is the intent.
- **Acceptance criteria:** `cargo --version` reports 1.98.1 in the repository, and the required checks need nothing beyond rustup. Every package declares `rust-version` 1.98, enforced by proposal 0008's test.
- **Future extension:** a bump is its own item.

## Review

`cargo xtask check` passes on 1.98.1. The nested per-crate pins were removed, because a nested `rust-toolchain.toml` would override this one when working inside that crate.
