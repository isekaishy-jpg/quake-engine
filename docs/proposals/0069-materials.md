# Proposal 0069: materials

Status: accepted, 2026-11-05, increment 11.

## Research

- [The research note](../research/maps-materials-lighting.md) wants about 8 slots a material, shared by blended layers and extra maps, with the shading model chosen per material and no assumption of PBR.
- Today alpha means different things on different images, and each texture is one image with one fixed sampler.
- A surface's normal and tangents can be worked out from screen derivatives of position and texture coordinates, so brush faces need no extra vertex data.
- `textureSampleGrad` with derivatives taken up front lets a shader read images inside branches.

## Design

- `engine_map::Material` holds shading, alpha, slots and settings. A `Texture` gets an optional material, and without one it draws exactly as before, so Quake's levels are untouched.
- A material is an `.emat` document in a container, at `materials/<name>.emat`, naming PNG images. A missing material shows a placeholder.
- Slot 0 is the base. Named maps and layers take the other seven, and a material asking for more is refused. An image used twice takes one slot.
- Roles: normal, specular with gloss in alpha, emissive, height, tint, matcap, ramp, detail, and up to 3 layers of base and mask.
- `material.wgsl` follows `world.wgsl` in one module. One fragment entry handles scrolling, flipbooks, parallax, triplanar projection, hexagon tiles, height-aware layers, detail, cel bands, matcap, specular from dynamic lights and emissive.
- Masked materials use the same pipeline and discard. Blended materials draw after everything opaque, with depth writes off.
- Parameters sit in a uniform buffer, 256 bytes apart, one per material. Unused slots bind a 1 pixel image.
- Materials get full mip chains and a linear, anisotropic sampler.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** one large shader costs some registers on simple materials, but it keeps one path to test. Specular only answers dynamic lights until the light compiler stores direction.
- **Acceptance criteria:** an `.emat` loads with its slots counted and shared, too many images are refused, a missing material gives a placeholder, unchanged Quake levels draw as before, and GPU tests see each feature change pixels against a control.
- **Future extension:** painted per-face masks arrive with the editor.

## Review

See the increment 11 review.
