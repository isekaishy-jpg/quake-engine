# Proposal 0088: visibility and entity culling

Status: accepted, 2026-11-05, increment 11.

## Research

- [The plan](../research/plan.md) sets out entity culling: every client is sent every entity, so a cheating client can see through walls, and traffic is higher than it needs to be. It needs visible sets, and working maps have none: all their faces sit in one leaf.
- Portals and `vis`-style flood fills are exact but need leaves that follow the geometry, and ours have one leaf for each kind of contents. Sampling lines between points in space is simple, works on any tree, and errs towards a small risk of missing a thin gap, which widening by a cell covers.
- The decided rule for what's loaded and drawn is that a player never sees something missing. So a set should be generous.

## Design

### Cells

- Compiling lays a grid over the world, of `cell_size` units, 256 unless the world says. Each cell keeps a copy of the part of the tree touching it, with its own leaf for each kind of contents it holds. Collision is unchanged: its hulls don't use these leaves.
- A face is listed in every cell its bounds touch. With no visible sets every leaf shows, so an unbaked map draws as it did.

### Visible sets

- Baking samples nine points in each cell that aren't solid, and calls two cells visible if a line between any pair is clear of solid brushes. Glass and moving things don't block.
- Each set is widened by one cell, then run-length encoded as `Tree::visibility` expects. It goes in the bake.

### Entity culling

- A seat's view has a visible set. Each entity's box is tested against the leaves it touches, found from the tree each tick.
- Each seat's snapshot leaves out what it can't see, by the plan's rules: the player always sees themselves, things with no place in the world always show, and a map with no visible sets shows everything. What's near the player always shows, so nothing they could touch is missing.
- `solid_boxes` takes the same filter. Moving brush models are always sent, since prediction collides with them.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** sets are generous, by a cell, which costs some drawing and traffic in exchange for never popping in.
- **Acceptance criteria:** two sealed rooms can't see each other, a doorway lets them, a map with no bake draws everything, an entity behind a wall isn't in a snapshot and one in view is, one nearby always is, and nothing about collision or prediction changes.
- **Future extension:** none planned.

## Review

See the increment 11 review.
