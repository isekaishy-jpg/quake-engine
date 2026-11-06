# Proposal 0111: mesh collision

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

[The note](../research/mesh-support.md) has the human's choice: a mesh gets convex pieces that fit it, generated, so a well can be stepped into with nobody placing boxes. It asks how close a fit is enough, how many pieces, what a hollow shape turns into, and how pieces fit the map's trees.

- Collision goes through the map's trees as brushes, and a brush is any set of planes round something convex. So pieces become brushes, and neither the trees nor movement change.
- V-HACD and CoACD do this with voxels and a measure of how far a piece is from convex. They need a convex hull, and CoACD a tree search. What we need is much less exact: a player is 32 units wide.
- A hull needs a real algorithm and gives planes in any direction, each of which costs the trees. Planes in a fixed set of directions, pushed out until they touch what's there, need none of that. That's a k-DOP, as broad phase collision has long used.
- A hollow shape needs a rule. A well is open at the top, so its middle can be reached from outside, and it stays open. A closed barrel's inside can't, so it fills, which is right, as nobody gets in.

## Design

- **Voxels.** A mesh's longest side gets 48 voxels, down to a smallest of 1 unit. Triangles mark what they touch.
- **Filling.** What can't be reached from outside the grid is solid.
- **Cutting.** Each joined-up piece gets planes in 34 directions: the axes, the diagonals, and eight more round the upright axis, for what's round. If more than 12% of what they hold is empty, it's cut where the boxes round its halves hold least, and the halves go round again, up to seven times.
- **Planes that do nothing go,** as most only touch a corner, and each costs the trees. Those along the axes stay, to keep a piece closed. A level with three statues and a well took 6.3 s to compile with all of them.
- **`Mesh::convex`** holds pieces in the mesh's own space. `import_gltf` writes them into the `.emesh`, and a mesh with none has them worked out when it's read. Placing turns each plane, so stretching and turning work.
- **Boxes still win.** Boxes in a file are an author's choice. The mesh's bounds as one box are only for a mesh with no size to fit.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a fit is as good as a voxel, which is a 48th of a mesh, or 4 units on a 192 unit well. Fine detail isn't collided with, which is what a player wants anyway. Thin walls are a voxel thick, however thin they're drawn.
- **Acceptance criteria:** a well can be stepped into, its walls stop you all the way round, a box is one piece, stretching and turning work, and an author's boxes still win.
- **Future extension:** none planned.

## Review

A player dropped into a generated well and stood on the floor inside it. `a_placed_mesh_is_collided_with_by_its_pieces` caught planes being placed by their turned direction, not their own.
