# Proposal 0092: naming the Sol crates

Status: accepted and implemented, 2026-11-05, increment 12.

## Research

- `solapp`, `solworker` and `solcache` kept the names they arrived with, so they didn't line up with the `engine_*` crates. [The research plan](../research/plan.md) set out the steps, and the human asked for it to be done.
- Their public types carried prefixes from when they were separate libraries: `SA`, `SW` and `SC`, about 300 names in all. Nothing else in the engine prefixes its types, and a crate path already says where a name comes from.
- With the prefixes gone, a few private names would collide, and `engine_platform`'s own types share names with the host's.

## Design

| Was | Now |
|---|---|
| `solapp` | `engine_host` |
| `solworker` | `engine_worker` |
| `solcache` | `engine_cache` |
| `solcache-solworker-example` | `engine_cache_worker_example` |
| `SAError`, `SWRuntime`, `SCCache` and the like | `Error`, `Runtime`, `Cache`: the prefix goes and the rest stays |
| `SOLAPP_*`, `SOLWORKER_*` test variables | `ENGINE_HOST_*`, `ENGINE_WORKER_*` |
| `SW_BENCH_*` and the like | `WORKER_BENCH_*` and the like |
| `SA`, `SW`, `SC` in prose | The crate's name |

- Private names that would clash were renamed: `Phase` to `RuntimePhase`, the owner's `Reservation` to `SlotReservation`, and `Backing` to `BackingStore`. `winit`'s names are imported as `WinitKeyLocation` and the like.
- `engine_platform` calls the crate `backend`, as `backend::Error`, so its own types keep their names and it's plain which is which.
- The test layout check skips what `LINT_EXEMPT` names, rather than anything starting with `sol`.

## Proposal

- **Selected approach:** the above. The names are the plan's suggestions.
- **Tradeoffs:** `engine_host` sits next to `engine_platform`'s own `host.rs`. They're the backend and what wraps it, as they were.
- **Acceptance criteria:** no behaviour changes, `cargo xtask check` and the real map and netplay tests pass, and a search finds the old names only in the records.
- **Future extension:** none.

## Review

Accepted proposals and reviews keep the old names, as records. This table is the map between them.
