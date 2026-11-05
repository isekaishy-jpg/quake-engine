# Proposal 0066: the map format

Status: accepted, 2026-11-05, increment 11.

## Research

- [The research note](../research/maps-materials-lighting.md) decides on our own format: text while working, binary from a final compile, one data model, plain versioning.
- Godot's `.tscn` shows that a text format with one version number can carry an engine for years. Compiled data there is a binary form of the same thing.
- `serde_json` isn't a fit for hand-read map text, and Rust's own float formatting already prints the shortest text that reads back exactly.

## Design

- `engine_format` owns a generic `Document`: a version, a kind, and a tree of `Block`s, each with a kind, an optional name, typed fields and child blocks.
- A value is a number, text, a flag or a list of values. The format attaches no meaning to them.
- Text looks like `entity elevator_1 { speed = 150 }`, with `#` comments. A document starts `format 1 map`.
- Binary is the same tree, length-prefixed, under a magic number. Every count is checked against what's left, so a bad file is refused, never trusted.
- Reading a version newer than the reader's is refused with a clear error.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a second parser to own, in exchange for files we can read and diff. Comments are dropped when a tool rewrites a file.
- **Acceptance criteria:** text and binary both round-trip exactly, including awkward floats and escapes. Text and binary of one document compare equal. Malformed text gives a line number. Truncated or oversized binary is refused without panicking.
- **Future extension:** typed map contents arrive in the next proposals.

## Review

See the increment 11 review.
