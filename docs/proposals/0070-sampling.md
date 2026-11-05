# Proposal 0070: sampling

Status: accepted, 2026-11-05, increment 11.

## Research

- Texture sampling was fixed in code: nearest filtering, with blended mips, and no anisotropy.
- The human wants filtering as a feature, and plain Quake textures should keep their look unless asked.

## Design

- `r_filter` chooses for plain textures: 0 hard texels with blended mips, as now, 1 smooth, 2 smooth with blended mips.
- `r_anisotropy` sharpens surfaces at an angle, from 1 to 16, for materials always and for textures when smooth.
- Materials are always smooth, since they have full mip chains and aren't pixel art.
- `Renderer::set_filter` makes new samplers and rewrites each descriptor set, waiting for the device first.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** changing the setting stalls for a moment, which is fine for a setting.
- **Acceptance criteria:** the default leaves Quake's levels unchanged, each setting changes a capture, and a material stays smooth under `r_filter 0`.
- **Future extension:** none planned.

## Review

See the increment 11 review.
