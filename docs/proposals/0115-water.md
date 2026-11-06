# Proposal 0115: water

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

[The note](../research/effects-water-shaders.md) asks for a standard general system for all types of water, with reflections and the usual features, depth fog, underwater effects and particles, and particle waterfalls. Higher quality water and lava must still be able to take a murky look like Quake's. The sprint list adds reflections without stipple.

- The stipple was a choice: each pixel's walk across the screen started at a different offset, from a noise pattern, to hide the walk's steps. On a calm surface the pattern itself was what you saw.
- Screen-space reflections usually close in on a hit with a short binary search. Once the place is found exactly, there are no steps to hide, and no noise is needed.
- What a surface mirrored was then tinted by the water's depth, as if it were under the water.
- [Screen-space reflections only see what's on screen](https://docs.unity3d.com/Packages/com.unity.postprocessing@2.0/manual/Screen-space-Reflections.html), so they need something to fall back on. The sky and what's under the surface already do that.
- Shafts through a pool had noise in them as well.

## Design

- **Reflections close in on what they hit,** in five steps, with no noise. Mirrors on materials get this too.
- **The order is right.** Bending, caustics, depth and shafts happen to what's under a surface, and then it mirrors what's over it, undimmed.
- **Fresnel by the usual curve:** little straight down, nearly all at a glance.
- **Refraction.** What's seen through a surface is bent by its waves, unless that would pull in something above the surface.
- **Waves** are three sines at odd angles, as a normal, so they're smooth and show no pattern.
- **Each level says how.** `reflect`, `waves` and `refraction` join `color`, `absorption` and `caustics`, and each liquid has its usual values. That's what makes the murky look a matter of settings.
- **Effects** for splashes, bubbles and falls are files, on the system of proposal 0114, and a surface can have a shader.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a pool is still a box with a flat top, with effects from the eight nearest, as a level's liquid is made of brushes. A surface can be any material, so a sloped river is a mesh with a shader, without depth.
- **Acceptance criteria:** a calm surface has no stipple, what it mirrors isn't tinted, it bends what's under it, and a level can make it murky.
- **Future extension:** none planned.

## Review

Vesper Deep's canal was looked at from two places. It mirrors its bank cleanly, and frame rate didn't move, at about 205 to 213 FPS at 1280 by 720.
