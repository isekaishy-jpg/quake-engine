# Review 0002: increment 0

Status: review record, 2026-11-03. Reviewed: proposals 0001 to 0009, the workspace, `xtask` and the guides.

| # | Finding | Handling |
|---|---|---|
| 1 | Vendored `winit` and `micropool` counted as workspace members and failed `-D warnings` | Moved to `vendor/`, recorded in proposals 0003 and 0009 |
| 2 | Rules can't see a third-party type in a public API | Recorded as a limit in proposal 0008, and added to the style guide's review checklist |
| 3 | `engine_client` depended on `engine_sim` for movement in the first draft | Split out `engine_movement` |
| 4 | The Sol crates don't meet the workspace lints or test layout | Named exemptions, with the follow-up in the research plan |

## Verified

- `cargo --version` reports the pinned toolchain, and `cargo run` builds and exits cleanly.
- `cargo xtask check` passes: format, clippy with `-D warnings`, and all workspace tests.
- Adding `engine_render` to `engine_sim` made the permanent crate-graph test fail, and the change was reverted.
- Adding an undocumented public function with a narrowing cast to `engine_core` failed clippy, and the change was reverted.

## Remaining

Increment 1 is next: window, main loop, console and packs. Each item goes through the workflow on its own. Open questions are in [docs/research/open-questions.md](../research/open-questions.md).
