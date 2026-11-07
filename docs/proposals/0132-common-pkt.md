# Proposal 0132: common.pkt

Status: accepted and implemented, 2026-11-06, the [work list](../research/work-list.md), item 4.

## Research

The human asked for everything to go in one `common.pkt`, or a `ui.pkt` as well if the UI must be alone, with scripts in the container.

- Nothing needs the UI to be alone. A container is opened once and read by path, and a file in it costs nothing until it's read. So one container does.
- The packing from [proposal 0106](0106-packing.md) already takes UI art, fonts, `scripts/ui.luau`, shaders and effects, as they aren't made for content alone.
- A game could only reach a container through `-content`, so nothing came with the program.
- `content/ui` and `content/fonts` stay out of git. A container is built from a folder, so it holds them without them going in the repository.

## Design

- **`common.pkt` beside the program** is mounted if it's there, and first, so that `-content` overrides it.
- It's made with `engine-compile content --pack common.pkt`, and it's a build's output, not something in the repository.
- `fids.txt` itself doesn't go in the container.
- The fixture's game scripts stay in the fixture's crate, as they're the fixture's. `scripts/` in content is what goes in a container.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** one container means a small change rebuilds all of it, which takes 13 seconds and only happens when packaging. While working, the folder is used as ever.
- **Acceptance criteria:** a copy of the program with only `common.pkt` beside it plays a level with the engine's own UI.
- **Future extension:** none planned.

## Review

A copy of the game in an empty folder with `common.pkt` loaded Vesper Deep with no `-content`, and the container held `scripts/ui.luau`, the splash and the fonts. The first run also put `fids.txt` in itself, which is why it's left out now.
