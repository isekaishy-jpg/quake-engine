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

## What's in a `.emesh`

A document, binary from import, and text if a script writes it. Each `part` has a `material`, `points`, `uvs` and `triangles`, and can have `normals`, one to a corner. With none, they're worked out from the shape: smooth, except where two triangles turn by more than 40 degrees. Each `hull` is a box for collision.
