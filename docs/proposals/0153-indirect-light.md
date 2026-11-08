# Proposal 0153: indirect light from every light

Status: accepted for item 179, 2026-11-08.

## Why

The human, after sky light: shadows should take light from any source, or an interior lit by its lights has dark shadows unless that's what's wanted; and the ambient that has no source still matters. Engines call light off what's lit global illumination, and its parts by other names (below).

## What there was

| Light | What its shadows got |
|---|---|
| A baked lamp | What it bounces, baked, by the level's `bounce` |
| The sun | Its bounce, baked, and since 0152 the sky's light |
| A live light | Nothing: its shadow was where its light wasn't |
| `ambient`, the level's and a day's | Itself, the same everywhere, shadows and rooms and all, and corners darken it |

## What engines call it

| | Unreal | Unity | Godot | Quake's light tools |
|---|---|---|---|---|
| All of it | Lumen live, Lightmass baked | Global Illumination: Enlighten live, the Progressive Lightmapper baked | SDFGI and VoxelGI live, LightmapGI baked | Bounce, or radiosity |
| Each light's share of it | Indirect Lighting Intensity | Indirect Multiplier, once Bounce Intensity | Indirect Energy | — |
| Light from the sky | Sky Light | Environment Lighting | the environment's sky | the sky's dome light |
| What keeps a room from black | Skylight Leaking | Environment Lighting's ambient | Ambient Light | minlight |
| Light with no source | (none: a Sky Light does it) | Ambient, a colour or a gradient | Ambient Light | minlight, darkened in corners by dirtmapping |

Unreal's and Unity's are from their manuals; Godot's and the Quake tools' are from memory, and weren't checked.

## What's done live elsewhere

- **Probes worked out as lights move** (DDGI and the like): a grid of probes, each gathering light by rays and keeping how far it can see so it doesn't light through walls. It needs rays against the scene on the GPU.
- **Voxels** (VoxelGI, SVOGI): the scene in a grid of cells, light spread through them. Much to keep, and it smears.
- **The screen** (SSGI): light off what's on screen. Cheap, and it misses what isn't.
- **Light off shadow maps** (reflective shadow maps, light propagation volumes): what a light lights, from its shadow map, sent on. It goes through walls unless something else stops it.

## What's done here

- **Each light says how much of what it lights bounces on**, `bounce`, 1 unless it says, as Unreal's and Unity's do. A baked lamp's goes into its face's bounce by how much of the face's light it gives. A live light's is worked out live, below. A live light from an effect, a flash or a spark, bounces none: it's there for a moment.
- **A live light's bounce goes to the map's probes round it**, worked out on the CPU as it changes, a couple of lights a frame, and the rest keep what they had: light off what it lights falls on a probe as its own light falls off, by a quarter as a baked bounce does, and by how much of its room the probe can see. Its room is places round it, out along each axis and to each corner, as far as half its reach and short of what stops it, and a probe that can see none of them gets none, so it doesn't go through a wall. A probe behind a wall it can see over gets some, as what's lit beyond reaches it. Surfaces read it between the probes, as light from all round, which corners darken.
- **What sees no sky can get some of it,** `sky_leak` in the world, from 0 to 1, as Unreal's skylight leaking is, for art's sake where a room shouldn't go black. Where the sky is half hidden, half of what's hidden leaks.
- **`ambient` stays** as it was: light with no source, everywhere alike.

## What came of it

- **Tried in the game:** a shut room lit only by a live light with shadows, and a pillar. With its bounce the pillar's face in its own shadow goes from black to its bricks showing, from 0.1 on the screen to 5.2, and the floor in its shadow from 0 to 0.6. The level's look has light go dim fast, as the old look did, which takes what's bounced down with it, as it does a baked lamp's.
- **Tests:** a line is clear unless it crosses what's solid, either way, and over a wall but not a high one. A live light bounces in its room and not through a wall, into its own shadow over a wall it can see over but less, as it falls off, by its colour and how much it bounces, and none with none or out of reach. On the GPU, what a live light pointing away from a floor bounces lights it, none with no bounce or no probes, and it follows the light's colour as it changes. A lamp's `bounce` gives none at 0 and twice at 2, a live light's is read and is 1 unless it says, and a shut room gets half the sky at `sky_leak 0.5`.
- **What it doesn't do:** models aren't lit by live lights yet, so not by what they bounce either. A live sun's own light bounces as baked from where the level puts it, which a day tints. Bounce is only as fine as the probes, so it's soft, and it's a frame or two behind a light that moves. A probe in a corner the room's places can't see gets none.

## Sources

- [Proposal 0152: sky light](0152-sky-light.md)
- [Proposal 0084: live lights](0084-live-lights.md)
- [Unreal: Lumen global illumination](https://dev.epicgames.com/documentation/unreal-engine/lumen-global-illumination-and-reflections-in-unreal-engine)
- [Unreal: Indirect Lighting Intensity](https://docs.unrealengine.com/4.27/en-US/API/Runtime/Engine/Components/ULightComponentBase/IndirectLightingIntensity/index.html)
- [Unity: Light, Indirect Multiplier](https://docs.unity3d.com/ru/2019.3/Manual/class-Light.html)
- Carsten Dachsbacher and Marc Stamminger, "Reflective Shadow Maps", I3D 2005
- Zander Majercik and others, "Dynamic Diffuse Global Illumination with Ray-Traced Irradiance Fields", JCGT 2019
