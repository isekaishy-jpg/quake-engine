# Proposal 0072: the light compiler

Status: accepted, 2026-11-05, increment 11.

## Research

- [The research note](../research/maps-materials-lighting.md) sets the kinds of light. Fixed lights are baked and featureful, bounced light is baked, and direct and bounced stay apart so moving things combine properly. Baked light stores direction for specular and for reflections that shift as you pass.
- Our own format means `ericw-tools` can't bake our maps.
- Unreal's Lightmass bakes coloured shadows through stained glass, and Assassin's Creed baked lighting at keyframes, blended at runtime.
- `Map` already holds a lightmap per face with up to four styles. `Tree` isn't needed: the brushes give an exact occluder tree.

## Design

- `engine_compile::bake_light` runs after geometry, on every core, with no external tools.
- An occluder tree holds every opaque solid brush. See-through brushes are left out, and their faces become glass that tints what crosses them, by their `tint` image, so lead stops light and panes colour it.
- Lights are entities whose type starts with `light`: `origin`, `color`, `intensity`, `style`, `size` for soft shadows, `spot_direction` and `spot_angle`, `cookie`, `profile` for an IES-style curve by angle, and `keyframes` for limited motion. A `sun_intensity` on `worldspawn` makes the sky's sun a light.
- A light with `keyframes` bakes once at each, each on its own style from 48 up. Blending those styles at runtime moves the light.
- Each face gets a layer per style, and bounced light always goes on style 63, which never changes.
- Bounced light gathers from up to 1,024 lit surface samples, weighted by each face's average colour.
- `Map::light_directions` runs alongside the samples, and the renderer packs a second atlas from it.
- A probe grid, 128 units apart, holds six sided light and a direction, for moving things.
- Compiled light goes in a `.ebake` file beside the map, holding a hash of the map's text. A bake that doesn't match is ignored, so editing never shows stale light.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** one bounce only, and a face holds three lit styles plus bounce. Crisp edges rely on `light_spacing` for now, with distance field shadows as a later stage.
- **Acceptance criteria:** a wall behind a pillar is darker than one in the open, a light's colour reaches the surface, a spot only lights its cone, glass tints what's behind it, bounced light reaches a corner no light sees, a keyframed light gives one style per key, probes are brighter near a light, and a stale bake is ignored.
- **Future extension:** none planned.

## Review

See the increment 11 review.
