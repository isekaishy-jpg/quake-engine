# Rendering and editor plan

The work list for taking [maps, materials and lighting](maps-materials-lighting.md) through the workflow, set by the human on 2026-11-05. Each stage gets its proposal, implementation, review and commit on `increment-11`. Known limitations become items here, not caveats. When every item is done, circular quality and bug passes run until nothing is left.

## Chosen for the human's review

The note left these open. Each was chosen by recommendation so work could go on, and each can be changed.

| Question | Chosen | Why |
|---|---|---|
| `Map` shape | It grows new sections and keeps its tree as a compile output | The note's first stage fills sections from a compiled tree, and every runtime system keeps working |
| Lighting space | Gamma stays the default, with linear as a setting | The human isn't after PBR, and existing levels keep their look |
| Text syntax | Our own block syntax, as sketched in the note | Small, readable, and nothing outside has to read it |
| Binary | The same document tree, written as sections | One data model, two encodings |
| Tile grid | Flat, with one file per tile under a manifest | Simplest, and stacking can come with a version bump |
| Placed thing IDs | A hidden 64-bit ID on each record | Stable through saves, and authors never see it |
| Editor UI | `egui`, painted through `vulkane` | One window with the viewport, and the human's Scopework uses it |

## Stages

Status is one of todo, doing or done.

| # | Stage | Contents | Status |
|---|---|---|---|
| 1 | Format | Text and binary documents, versioning, `engine_format` | done |
| 2 | Compile | Brushes to faces, tree and clip hulls, and the working map loading and playing | done |
| 3 | Containers | Reusable items by name, layered, as folders and packs | done |
| 4 | Materials | One image type, materials, shading models, 8 slots, the Quake importer's mapping | done |
| 5 | Sampling | Filter modes, anisotropy, generated mips | done |
| 6 | Surface maps | Specular, normal, emissive, height, parallax, tint, detail | todo |
| 7 | Layers | Blended layers with painted masks, height blending, flattening on compile | todo |
| 8 | Alpha | Modes, sorting, alpha to coverage | todo |
| 9 | Anti-aliasing | MSAA and SMAA as settings | todo |
| 10 | Sky | Sphere, gradient horizon, cloud layer, premade or procedural | done |
| 11 | Light compiler | Fixed lights, direct and bounced apart, direction, probes, crisp shadows, glass tint, keyframes, cookies, IES, area lights | todo |
| 12 | Live lights | Clustered lists, shadow maps with caching, budgets | todo |
| 13 | Object shadows | Capsules, occlusion sidecars, per-part detail | todo |
| 14 | Atmosphere | Height fog, fog grid, stencil rays with boxes and collision occlusion, water light | todo |
| 15 | Small lights | Torch shader, flipbooks, particles | todo |
| 16 | Stylised | Cel shading, matcap, outlines by hull and post-process, semi-outlines | todo |
| 17 | Surface extras | Decals, hotspots, hex tiling, triplanar, flow maps, reflections, rounded edges | todo |
| 18 | Stamping | Relief, carving, inpainting new surfaces, seam filling, mega texture | todo |
| 19 | Visibility | Our own compiler, and entity culling from the plan | todo |
| 20 | Tiles | Grid, manifest, streaming, coordinates, the unloaded tile rule, sleeping entities | todo |
| 21 | Pathing | Waypoint splines, navmesh, doorway baseline | todo |
| 22 | Platform | Shell, operations layer, command line, MCP, viewport, editor tools, prefabs | todo |
| 23 | Test maps | Maps exercising every feature, with prefabs and prefab entities | todo |
| 24 | Reviews | Circular quality and bug passes until none are left | todo |

## Log

Newest last. A line is added when a stage's state changes.
