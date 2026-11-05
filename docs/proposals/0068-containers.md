# Proposal 0068: containers

Status: accepted, 2026-11-05, increment 11.

## Research

- [The research note](../research/maps-materials-lighting.md) decides on common containers holding reusable items, referred to by name, so file sizes don't balloon.
- `engine_assets::FileSystem` already searches archives in mount order, so later ones override earlier ones by path, which is the layering wanted.

## Design

- A container is a folder while working, and a `.epak` file once compiled. Both are `Archive`s, so nothing else can tell.
- A pack is a small index of paths, offsets and sizes, then the files. Every offset is checked against the file's size on opening.
- `-content` takes a folder or a pack.
- Items live at conventional paths, such as `materials/stone.emat` and `textures/stone.png`.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** no compression yet, since most bulk data will be image files that are already compressed.
- **Acceptance criteria:** a pack reads back what its folder held, a later container overrides an earlier one by name while the rest still reads through, and damaged packs are refused.
- **Future extension:** none planned.

## Review

See the increment 11 review.
