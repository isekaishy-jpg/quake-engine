# Proposal 0125: holding the host, worker and cache crates to the same bar

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

`engine_host`, `engine_worker` and `engine_cache` came in as a block. [Proposal 0096](0096-the-last-crates-under-the-lints.md) put them under the workspace lints, and left two things: a bug and quality review, and moving their tests out of `src`, with `INLINE_TESTS` in the test layout rule as the list of what was still let off.

They're 32,159 lines of code and 29,422 of tests, which is a lot to take on trust.

## Design

**Tests.** `engine_worker` and `engine_cache` had none left in `src`. `engine_host` had 23 in six modules. The earlier worry was private seams, and a `#[path]` module answers it, as the rest of `engine_host` already does: the test lives in `tests/unit` and is still compiled as a child of what it tests, so it reaches what it reached. `INLINE_TESTS` is gone. No `#[test]` is under any crate's `src` now, and a test holds that. All three keep `#[cfg(test)]` hooks, 126 in `engine_host`, 61 in `engine_worker` and 2 in `engine_cache`: `#[path]` modules, and seams that drive a fake window system or inject faults. So `TEST_SEAMS` lets those three off the stricter rule that no test code is in `src` at all, and every other crate is held to it.

**Review.** It went by what can go wrong rather than by reading in order:

| Looked at | How many | Found |
|---|---|---|
| `unsafe` | 22, all | Each has a reason. The chunked slice in `execution/scope.rs`, the topology buffer and the Win32 calls all hold up: lengths are checked, alignment is by construction, nothing outlives what it points at |
| `panic!`, `unreachable!` | 19, all | Three are injected faults in tests. The rest are real invariants, each right after the check that makes it so |
| `.unwrap()` in `src` | 82 | All were in the test modules now moved. 5 are left in `src` |
| `.expect` | 218 | Named invariants, the most common being "a lock nothing panics under". Those 27 are all in `owner/delivery.rs` and `owner/sender.rs`, whose locks hold only their own state and call nothing supplied by a caller |
| User code under a lock | Tasks and cleanups | Run through `catch_unwind` in `cleanup.rs` and `execution/scope.rs`, outside the state they report to |
| `TODO`, `FIXME` | 0 | |

No bug was found. These crates are the most carefully written in the workspace, with a test suite as big as the code.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** this is a review by kind of fault, and not of every line. The final rounds of the sprint go over them again with the rest.
- **Acceptance criteria:** no `#[test]` under any crate's `src`, `INLINE_TESTS` gone, and the test count unchanged.
- **Future extension:** none planned.

## Review

All 171 of `engine_host`'s tests still run, the same as before the move.
