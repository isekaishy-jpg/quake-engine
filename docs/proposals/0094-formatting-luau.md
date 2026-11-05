# Proposal 0094: formatting Luau

Status: accepted and implemented, 2026-11-05, increment 12.

## Research

- [The research plan](../research/plan.md) left open which tool enforces `docs/style.md` for Luau, and suggested `StyLua` in `xtask`.
- `StyLua` is the usual formatter for Luau, and understands its syntax with a feature of the same name. It's a library as well as a program, under the MPL 2.0.
- As a program it would be one more thing to install before `cargo xtask check` passes. As a library it's pinned like every other dependency, and `cargo` fetches it.

## Design

- `xtask` depends on `stylua` with default features off and `luau` on, so nothing of its language server or editor config comes along.
- `xtask::luau` formats with tabs, 100 columns and Unix line endings, which is what the style guide says.
- `cargo xtask fmt-luau` rewrites what's out of step, and `cargo xtask check` fails on it, before anything slower. A test holds the repository to it as well.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** the first formatting touches most Luau files, and ported scripts move a little further from how their sources are laid out. Their names stay, which is what comparing against a source needs.
- **Acceptance criteria:** tabs indent, Luau's own syntax parses, broken text is reported, and every Luau file in the repository is formatted.
- **Future extension:** none planned.

## Review

See the increment 12 review.
