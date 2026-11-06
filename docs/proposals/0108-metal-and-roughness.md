# Proposal 0108: metal and roughness

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

The human asked whether metal, roughness, specular and gloss could all be there, even if metal and roughness aren't our main way of working, since glTF is. [The note](../research/texture-standard.md) thought they could, as they aren't four separate things.

- Gloss and roughness say the same thing the other way round. In Blinn-Phong terms, `gloss = 2 / roughness^4 - 2` is the usual match.
- glTF packs metal in blue and roughness in green, and most tools put occlusion in red of the same picture, so one map carries all three.
- In metal-roughness, what isn't metal reflects about 4%, and metal reflects its own colour and has no diffuse colour. So both ways end up as a colour to tint highlights and a gloss. That's what the existing specular path takes.
- Metal with only highlights goes black, because most of what it shows is the room. We have no reflection probes. The light baked here stands in for the room, and the final pass's mirror adds what's on screen.

## Design

- `metal_rough`, `metallic` and `roughness` in a material give `Material::metal_rough`. With a map the numbers scale it, as glTF's factors do. Without one, they're the whole answer.
- The shader turns them into a tint and a gloss, then shares `shine_at` with the specular path, so a surface can have both, and they add.
- Metal's colour comes from the light here, tinted, and less of it from what's rough.
- Sharper highlights are brighter, as the same light is in a smaller spot. This is only for metal and roughness: specular maps stay as they were, so nothing already made changes.
- Polished metal mirrors in the final pass without being told to.
- Occlusion from red darkens what's lit.
- A surface using these isn't `PLAIN`, so it costs nothing on surfaces that don't.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** this isn't a full physically based model, and doesn't try to be. Light baked here standing in for the room is an approximation that reads as metal, and the engine's look is what matters. Reflection probes would be better, and the mirror covers what's on screen.
- **Acceptance criteria:** all four work on one surface, a material with none of them looks as it did, and metal doesn't go black.
- **Future extension:** glTF import fills these in (item 8).

## Review

No existing material changed: Vesper Deep has none of these fields, and its captures match. A wall made polished metal in Vesper Deep was looked at, and read as metal, with the room's colour in it.
