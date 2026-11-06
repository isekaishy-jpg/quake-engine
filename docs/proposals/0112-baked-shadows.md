# Proposal 0112: baked shadows

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

[The note](../research/shadow-rendering.md) sets out what the human saw, and what's open.

- A sample was lit or it wasn't, from one ray. So a lantern post's fan, thinner than a sample, baked as a blunt block.
- Only the first sharp light on a face kept an edge.
- A face has four layers. A light past that was dropped, and which one went by the order of lights in the map.
- Sample sharing only covers faces on one plane, so a shadow crossing a fold could jump by a whole sample.
- Most of a level isn't near a shadow's edge, so more rays everywhere would pay for nothing.

## Design

- **Several rays where they count.** Each light is first worked out with a ray a sample, along with how much light there'd be with nothing in the way. Where the share that gets through changes between neighbours, a shadow's edge is near, and those samples look across their own patch of surface, at eight places, held to the face. A sample then keeps how much of its patch is lit, so a thin fan tapers.
- **Round a corner.** A patch stops at the face's edge, so samples on either side of a fold both measure what's on their own side, and the shadow's edge lands in the same place. No sample sharing is needed for that.
- **Three sharp lights.** `CRISP_STYLES` holds three styles, which a face's sharp lights take in turn. Each layer already had its own edges, so nothing is added to a bake's format. Baked light tracks lose two styles for it.
- **Layers by what they're worth.** A face's lights are ranked by how much light they give it, so it no longer matters what order they're in a map. Lights of a style share a layer, as before. One layer is kept for bounced light.
- **Nothing dropped.** Light past the limit joins the plainest layer, as light that doesn't change. A sharp light that doesn't fit is worked out with its shadow first. If only sharp lights got a layer, the weakest gives way.
- **No empty layers.** A light that doesn't reach a face no longer takes a layer on it. That more than paid for everything above.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a flickering light past a face's layers shows steady there. A face needs four lights of different styles for that, and it's the weakest that goes.
- **Acceptance criteria:** a thin shadow comes out as coverage, a shadow's edge agrees across a fold, each sharp light has an edge, no light is lost past the limit, and the result doesn't depend on order.
- **Future extension:** none planned.

## Review

| Vesper Deep | Before | After |
|---|---|---|
| Bake | 25.4 s | 21.8 s |
| Light samples | 2,976,051 | 1,293,325 |

- At a join with samples 32 units apart, a shadow's edge fell at -15.5 on both faces. Without the patches it fell at -15.5 and -47.5, a whole sample apart.
- The first tests of this passed with the patches off, so proved nothing. They were rewritten to compare with the same level with no post, and now fail without it: `a_thin_shadow_comes_out_as_how_much_of_a_sample_it_covers` finds none part shaded.
