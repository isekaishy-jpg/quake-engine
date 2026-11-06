# Meshes

A mesh is what a brush isn't: anything round, or made in a modelling tool. See [proposal 0109](proposals/0109-meshes.md).

## Bringing one in

Models come in as glTF 2.0, in a `.gltf` or `.glb` file.

```bash
target/release/engine-tool content - import_gltf "{\"path\":\"well.glb\",\"name\":\"well\",\"scale\":32}"
```

That writes `meshes/well.emesh`, a material for each of the model's, and its pictures. `scale` is how many world units a metre is. glTF's Y up becomes our Z up. Metal, roughness, normal, glow and alpha all come across, and a colour with no picture becomes a small plain picture.

## Placing one

```text
mesh { source = well origin = 200 -180 0 angles = 0 45 0 scale = 1.4 }
```

Or give an entity `mesh = well`, and it's that entity's model.

Light maps are laid out for it when the level is baked, so there's no second set of UVs to make. For something small, `light = probe` lights the whole of it from one place, which costs almost nothing:

```text
mesh { source = barrel origin = 40 12 0 light = probe }
```

## What's in a `.emesh`

A document, binary from import, and text if a script writes it. Each `part` has a `material`, `points`, `uvs` and `triangles`, and can have `normals`, one to a corner. With none, they're worked out from the shape: smooth, except where two triangles turn by more than 40 degrees. Each `hull` is a box for collision.

## Collision

Convex pieces are fitted to a mesh for you, so a well can be stepped into and its walls stop you. A `hull` box in the file replaces that, if you'd rather choose. A placement with `only_seen = 1` has none.
