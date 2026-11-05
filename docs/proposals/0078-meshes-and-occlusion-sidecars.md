# Proposal 0078: meshes and occlusion sidecars

Status: accepted, 2026-11-05, increment 11.

## Research

- [The research note](../research/maps-materials-lighting.md) makes brushes and meshes both map geometry. An entity can be a mesh, a placement is a reference with its own position, rotation and scale, static placements are baked at compile, and bulk data stays in files text refers to.
- It also decides a tool makes a separate occlusion file for each mesh, with one averaged volume and minor per-part detail, so moving things can shade what's around them cheaply.
- The stages left meshes out, so this adds them.

## Design

- A `.emesh` file under `meshes/` is a document, in text or binary: `part` blocks with a material, points, UVs and triangles, and `hull` blocks with boxes for collision. With no hull, a mesh's bounds are one.
- A map's `mesh` block places one in the world. An entity's `mesh` field makes it that entity's model, as brushes do, and its `origin`, `angles` and `scale` place it.
- Each triangle becomes a face. Its UVs fit a face's texture axes exactly, so the renderer, the light compiler and materials need no new path.
- Hulls only enter the trees. They don't cut the faces of brushes, and the light compiler leaves them out, so a mesh's surface isn't baked as inside a solid. It traces the mesh's triangles instead, so shadows have the mesh's shape.
- `engine-compile --occlusion` writes `meshes/<name>.eocc`: a sphere for the whole mesh, then one for each part. Compiling moves an entity mesh's spheres into `Map::occluders`, and they join the capsules wherever that model goes, with parts only when near.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** faces are flat, so meshes look faceted until rounded edges. Spheres from corners follow their density, which suits an average.
- **Acceptance criteria:** a placed mesh draws with its material, collides, casts its shape in a bake and is lit where it stands. A moved mesh entity shades the floor from its sidecar. A bad mesh says what's wrong.
- **Future extension:** none planned.

## Review

See the increment 11 review.
