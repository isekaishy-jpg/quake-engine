# Proposal 0109: meshes

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

[The note](../research/mesh-support.md) has the human's choice: import glTF 2.0, and generate light map UVs and collision. Light and collision are items 9 and 10. This is the format, normals, drawing and import.

- `.emesh` had no normals, so everything drew flat, and nothing came from a modelling tool.
- Meshes reach the renderer as triangles of the world, which already has batching by material, visibility and a material path. A second path would break "one way of drawing", so normals go through that one.
- [`gltf`](https://github.com/gltf-rs/gltf) reads glTF 2.0 and `.glb`, and is widely used. Its `import` feature decodes pictures too.
- glTF's normal maps have green going up the picture, and ours down.

## Design

- **`.emesh` gains `normals`,** one to a corner. With none, they're worked out: triangles meeting at a place are smoothed together, weighted by area, unless they turn by more than 40 degrees. It goes by place, as a mesh often has two corners at one place for its UVs. Existing meshes get this for free.
- **`Map::normals`** runs beside `Map::points`, as three bytes each, and stops at the last that has one. All zeros means a flat face, so brushes cost nothing and old files read the same. Kept geometry holds them.
- **A vertex has a normal,** which brings it to 40 bytes. The material shader uses it for `surface_frame` where there is one. Tangents still come from screen derivatives, so they need no vertex data and no convention to get wrong.
- **Stretching:** a placed normal goes by the inverse of the stretch, or it leans the wrong way.
- **`import_gltf`** in `engine-tool` and `engine-mcp` flattens a scene's nodes into parts, brings Y up to Z up, and scales it. Materials convert, as item 7 made that possible: base colour, normal, metal and roughness, glow, alpha.
- Written as a binary document, which `document_of` already reads.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a mesh in the world is unmoving triangles batched by material, so instancing wouldn't save anything at this scale. A model with several placements costs its triangles each time.
- **Acceptance criteria:** a glTF model comes in, the right way up and the right size, with its material, and draws with its own normals. Old meshes and brushes draw as they did.
- **Future extension:** skeletal animation and morph targets, which the note puts after this, and level of detail, once a level has enough triangles to measure the cost. The fixture's alias models keep their own shader, as they're animated by whole frames.

## Review

A well of 288 triangles was generated as a `.glb`, brought in and placed in a level. It came in the right way up and size, with its colour, and cast its shape. Its wall showed each triangle's own light, as each is baked flat. That's item 9.
