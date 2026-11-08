# The work list after the sprint

Status: a list, not a plan. Nothing here has been through the workflow, and nothing is approved to build. The human set it on 2026-11-06, after an audit found that [the sprint](sprint.md) left agreed work out.

It replaces the Next table in the sprint. Those items were called "never on the list", and that was wrong: several are in the notes the sprint's items linked to.

## Order, and where it stands

The human's word, 2026-11-06: take every item through the whole workflow, with nothing left out. Afterwards the human makes more textures to the new standard. Earlier items unblock later ones. Status is in the log at the end.

| Stage | Items |
|---|---|
| 1. Going back to what was said | 149, 150 |
| 2. The base | 77, 3, 5, 4, 29, 30 |
| 3. Specs | 26, 133 |
| 4. MCP | 31 to 38, then 124 begun |
| 5. Cameras | 56 to 60 |
| 6. UI | 1, 2, 74, 6 |
| 7. Meshes | 7 to 12 |
| 8. Terrain | 43, 125 to 132 |
| 9. Shaders | 13, 14 |
| 10. Effects | 78 to 97, 16, 9 |
| 11. Fog and water | 70 to 76, 17, 98 to 102 |
| 12. Sky and light | 62, 63, 64, 69, 164, 65, 163, 66, 67, 68, 165 to 168, then 172 to 177 |
| 12b. Land that's whole | 171, then 169 and 170 in the tools, and the editor's part of them in stage 15 |
| 13. Sound | 103 to 123, 23 |
| 14. Smaller limits | 24, 25 |
| 15. The editor | 155 to 162, 39 to 42, 44 to 55, 61, 15 |
| 16. Doors | 151 |
| 17. Artifacts and the test map | 133 to 148, 124 finished, 18, 19, 20, 21 |
| 18. Review | 152, then circular quality and bug rounds, and a performance sweep |

## How we work through this

Too much space went and too many maps were recompiled in the sprint. `target` is 50 GB, and all 20 maps were rebaked again and again.

| # | Item | Waits on |
|---|---|---|
| 124 | One small test map that covers all the things, for everyday testing. It's a real level, not a room of patterns, and it grows as features land. Small is relative: a lift still needs another level to go down to. But it's there to test features, with no gameplay areas that test nothing. Each area earns its place, such as mesh terrain coming over flat slabs of old stone, or meeting the flat brush floor of a building. Other levels are baked at milestones, or when a change really touches them The human, 2026-11-07: it has no land yet, and a mixed area goes in later, with land running over old stone, meeting a building's brush floor, a dip with water in it, and a cave or an overhang. | Comes first |

## Artifacts for the test map

Real content of good quality, for item 124, and reusable after it. What's on the map shows what's working. All of them are made new from scratch, not reworked from the prefabs we have. Blender is available, with an MCP. A written spec comes first, as assets have to be on a standard.

| # | Item | Waits on |
|---|---|---|
| 133 | A spec for meshes and prefabs: sizes, detail, how they texture, how they snap, what comes with them | 26 |
| 134 | A proper well, as a mesh | 133 |
| 135 | A couple of good real mesh bridges | 133 |
| 136 | A lift platform, with a script: it moves when a player stands on it, or runs its own cycle up and down | 133 |
| 137 | Teleports of various styles: a doorway, a window with blinds, a ring gate style, and a portal you can see through to the other side | 133, 138 |
| 138 | Seeing through a portal, which the engine can't do. The `portal` prefab is a teleporter pad and nothing is drawn | |
| 139 | Better stairs and staircases, slopes and ramps | 133 |
| 140 | Rooms and hallways, as primitives and meshes | 133 |
| 141 | Dressings and pillars, as primitives and meshes | 133 |
| 142 | Lamps, torches and candles on holders, braziers and a pillar brazier, with their fire and light | 133, 78 |
| 143 | Sky presets, of higher quality and more variety. Item 42 is this | 63 |
| 144 | Switches of various types, as meshes with scripts | 133 |
| 145 | Objects that move | 133 |
| 146 | Grass, trees and rocks | 133, 147 |
| 147 | What foliage needs from the engine: drawing great numbers of copies, wind, and scattering them on land | 7 |
| 148 | Other mesh prefabs, for reuse | 133 |

## UI and packs

| # | Item | From | Waits on |
|---|---|---|---|
| 1 | Finish moving the UI to scripts: status bar, menus, words in the middle of the screen, scores and the intermission | The human | |
| 2 | Proper mouse support in menus | The human | |
| 3 | `.epak` becomes `.pkt`, our own name for the container. It's a change of name | The human | |
| 4 | Pack everything into `common.pkt`, or `ui.pkt` as well if the UI must be alone, with scripts in the pack | The human | 3 |
| 5 | Fids, which are file ids, and a FlatBuffers manifest for packs. The schemas are Claude's to research and design, and none exist yet | The human | 3 |
| 6 | Remove Quake's UI: `hud.rs`, `menu.rs` and the fallback to them. This reverses [proposal 0118](../proposals/0118-ui-scripts.md), which kept them | The human | 1, 2, 4, 74, 77 |

Older work to look at for 5, as examples and not to copy: `populous-reborn/crates/reborn_assets/schema/pack.fbs` has ids from a hash of the path, entries sorted by id, and a pack saying which packs it needs. `test/assets/manifest.json` has a size and hash for each file.

To settle in research for 4: whether the UI must be alone, how a game finds its packs with no `-content`, and where packs live while `content/ui` and `content/fonts` stay out of git.

## Organising the engine

At first Quake was the game. We're moving off that slowly, and the fixture keeps working, on the new systems. So Quake stays playable through all of this, as the test that nothing was lost.

| # | Item | Waits on |
|---|---|---|
| 77 | An organisation pass: what belongs in the engine, in game code, in scripts and in the fixture, and things moved to match. Only `engine_app` depends on `fixture_quake`, but 12 of its files do: rules, weapons, sounds, saves, options, the HUD and menus. Screen tints and flashes are one example: the renderer draws them, and game code says when | Before 6 |

## Mesh support

Some editor items wait for this.

| # | Item | From | Waits on |
|---|---|---|---|
| 7 | Instancing | [Mesh note](mesh-support.md) | |
| 8 | Level of detail | [Mesh note](mesh-support.md) | |
| 9 | Attach points, for maps, prefabs and models | [Mesh note](mesh-support.md), [effects note](effects-water-shaders.md) | |
| 10 | Skeletal animation, with bones to attach to | [Mesh note](mesh-support.md) | 9 |
| 11 | Morph targets | [Mesh note](mesh-support.md) | 10 |
| 12 | Whether the fixture's models move onto the mesh path, as there's meant to be one way of drawing | [Effects note](effects-water-shaders.md) | 10 |

## Mesh terrain

Likely a heightfield and an inverse heightfield, as many games have. Real maps are expected to mix it with meshes and primitives. Item 43 is this section, and the editor's terrain tab and circle are items 44 and 45.

| # | Item | Waits on |
|---|---|---|
| 125 | Heightfield terrain, in chunks on the tile grid, with level of detail | 8 |
| 126 | Inverse heightfield, and holes. It's needed: without it, anything underground, overhangs and bridges can only be prefabs, with no sculpting. Research how far it goes, such as more than one layer of land stacked for a bridge over a road | 125 |
| 127 | Shaping tools: raise, lower, smooth, stamp, splat, copy and the like | 125, 45 |
| 128 | Painting the land properly, with its own way of blending materials. A material has 8 image slots, and at most 3 blended layers over its base, each with no normal or specular of its own, so land needs more than that | 125, 45 |
| 129 | Terrain in everything else: collision straight from the heights and not as brushes, baked light and shadows, visibility and paths | 125 |
| 130 | Water on terrain: a surface shaped to the land, with an area set underneath it, so all of water's effects, fog, swimming and the rest work there. Pools are boxes today | 125, 17 |
| 131 | More blended textures on a surface, each with its own normal and shine, for materials as well as land. Research how many, and what each costs a pixel. Baking can take much of that cost: the mega texture already mixes layers and paint into one read, in one 2048 image a map. Research how that grows to big land, such as an image for each tile, or baking far land and blending near it, against what it costs in memory and disk | |
| 132 | Better tiling over big areas. Today a texture is just spread over them. Research the ways: variation at large scale, breaking up the repeat, and mixing sizes | |

| 169 | Caves, overhangs and bridges in one step, in the tools and the editor. Inverse land and holes are in, and each can be made and walked today, but by hand: sheets are added one at a time, their holes cut to match, and nothing lines their edges up. Say where and how big, and the sheets, thickness, matching holes and edges come out right | 126, 127 |
| 171 | Land that folds back on itself, so that a cliff can lean out and a cave can be sculpted, as one surface. Sheets can't, and today what folds has to be a mesh, which the human says is a real issue. Research the ways: heights that can also push sideways, a volume made into a surface when a level's built, or sculpting meshes. Each has to work with collision, the bake, paths, painting, water and the tools. Decide this first, as it changes what items 169 and 170 are | 126 |
| 170 | Better edges on holes. A hole is a whole cell, so its edge steps, and today a mesh has to hide that. Research the ways, for the common cases at least | 126 |

`test/docs/terrain.md` and `test/src/render.rs` are an older experiment in heightfields, painted layers and water on mesh terrain. They give an idea, and aren't to copy.

## Shaders, particles and water

| # | Item | From | Waits on |
|---|---|---|---|
| 13 | Shader node graphs, over surface code | [Effects note](effects-water-shaders.md) | |
| 14 | Compute shaders | [Effects note](effects-water-shaders.md) | |
| 15 | One visual graph editor, shared by shaders and particles | [Effects note](effects-water-shaders.md) | 13, the editor talk |
| 16 | Mesh particles | [Effects note](effects-water-shaders.md) | 7 |
| 17 | Water of all types: more than a box with a flat top, on meshes with depth | [Effects note](effects-water-shaders.md) | |
| 29 | Precompile shaders. Compiling at runtime can stay for testing, and has no place in a finished product. This reverses [proposal 0107](../proposals/0107-shaders.md), which measured the cost and left it | The human | |
| 30 | The engine's own shaders in the container, as the note says. Proposal 0107 kept the core in the program, so a broken one could still show green, and that needs an answer | [Effects note](effects-water-shaders.md) | 29 |

## The particle and effects pipeline

What's there is a working first version: files of settings, CPU and GPU, sprites, ribbons, beams, lights and marks, and scripts can start them. It has to reach spell effects, rocket splashes, fog and dust with turbulence, sparks and player effects. Item 13 of the sprint was not that. Items 9, 15, 16, 18 and 37 are part of this too.

**Look**

| # | Item | Waits on |
|---|---|---|
| 78 | Textured particles: an image, material or shader for each emitter. This comes first, as nothing like a spell can be made without it | |
| 79 | Flipbooks: pictures that animate through a particle's life | 78 |
| 80 | Shaders that reach particles | 78 |
| 81 | Rotation and spin | |
| 82 | Stretching along how they move | |
| 83 | Fading where they meet a surface, so they don't cut hard into walls | |
| 84 | Lit particles, which take light from the level | |
| 85 | Proper ordering of what's drawn over what | |

**Behaviour**

| # | Item | Waits on |
|---|---|---|
| 86 | Collision: particles that hit surfaces, bounce, die or stick | |
| 87 | Events beyond `on_death`, such as on hitting something, and a timeline | 86 |
| 88 | More forces: attractors, vortices and wind | |
| 89 | More shapes: rings, cones, and emitting from a mesh's surface | Mesh support, for meshes |
| 90 | What the GPU can do, with compute, so it isn't limited to what follows from a number and the time | 14 |

**Control**

| # | Item | Waits on |
|---|---|---|
| 91 | Stopping an effect from a script, and fading it out | |
| 92 | Settings for one instance from a script: colour, size, strength | |
| 93 | Limits that scale: distance, how much is on screen, and a budget | |

**Pipeline**

| # | Item | Waits on |
|---|---|---|
| 94 | Reloading effects while the game runs | |
| 95 | A preview and live tweaking in the editor, in an effects tab | The editor's design |
| 96 | Quake's particles on the new system, so there's one way of doing it. The fixture's look stays | 78 |
| 97 | A real library of effects to start from | Authoring, later |

## Fog and being underwater

Fog needs a lot. Today it's one even height layer for a whole map, with no shape or movement. Lit fog, `r_fog_grid`, is off by default and only reads the bake.

| # | Item | Waits on |
|---|---|---|
| 70 | Fog volumes: fog in a place, with clear air beside it. Item 25's fog by zone goes with this | |
| 71 | Turbulence and movement: drift, wisps and banks | |
| 72 | Lit fog as the usual kind, if it holds up on cost, and lit by moving lights and rays as well as the bake | |
| 73 | Underwater fog, in areas, thickening with depth | |
| 74 | Underwater screen tint, a cheap effect. One exists, as a fixed colour. It needs to be set with the water and properly used. It's in `hud.rs`, so it has to keep working when item 6 removes that | Before 6 |
| 75 | Underwater particles, such as motes and bubbles around the view | |
| 76 | Each body of water with its own look. Today there's one `water` block for a map | |

Items 73 to 75 work together: being underwater is shown by all of them, without any one being overdone.

## Water's surface

A surface can have a material, with scrolling and a shader such as `ripple`. Nothing on it reacts to anything, and it has no overlays.

| # | Item | Waits on |
|---|---|---|
| 98 | Trails through water: a mask an author defines, such as a V with a rounded point around a player, whose arms spread as ripples and fade behind them | |
| 99 | Ripples: rings from what enters, what's shot, and rain | 98 |
| 100 | Overlays on a surface: foam, scum and the like, in layers over its material | |
| 101 | Foam and a soft edge where water meets the shore, by depth | 100 |
| 102 | Effects that come from play: splashes on going in, and spray when moving fast. Water's effects exist, and nothing starts them | |

For 98 and 99, `solarityclient` has surface ripple passes, with circular and directional kinds, as a reference and not to copy: `crates/rendering/src/device/vulkan_ripple`.

## Sound

What's there is a good mix: 128 voices, fades, following, muffling, rooms, buses, ducking and a limiter. Around it, sound is still shaped by the port. How a sound behaves is the author's to control, with spatial sound as the default and not the only way.

**The base**

| # | Item | Waits on |
|---|---|---|
| 103 | Ogg and the like for effects, not only WAV. `parse_wav` moves out of the fixture | 77 |
| 104 | The engine's own sounds, so a level isn't silent without Quake | Authoring, later |
| 105 | Sound events: a named sound with variants, random pitch and volume, its fade and its bus. A sound's kind comes from data, not from its name | 103 |
| 106 | Author control of each sound: spatial, or global with no place at all, and how it fades, muffles, follows and takes reverb. A script or a level can set these | 105 |
| 107 | A guide to sound, as water, zones and effects have | 105 |

**Headphones**

| # | Item | Waits on |
|---|---|---|
| 108 | Binaural sound for headphones, as the default: simulated surround, so you can hear which way a sound comes from, in front, behind and above. Plain stereo stays as a choice for speakers. This reverses [proposal 0129](../proposals/0129-real-sound.md), which left it out. Research what it costs, as `kira` mixes to stereo | |

**Authoring**

| # | Item | Waits on |
|---|---|---|
| 109 | A sound tab in the editor, with preview and live tweaking | The editor's design, 105 |
| 110 | Sound in effects files, so a fire or a spell brings its own | 105 |

**The world**

| # | Item | Waits on |
|---|---|---|
| 111 | Footsteps by surface, from the material | 105 |
| 112 | Underwater muffling, with items 73 to 75 | |
| 113 | Sound from an area or along a line, such as a river's length | |
| 114 | Sets of random background sounds for a zone: birds, drips, distant creaks | 105 |
| 115 | Those sets, and music, keyed by time of day | 66 |

**Music**

| # | Item | Waits on |
|---|---|---|
| 116 | Crossfades between zones | |
| 117 | Layers that rise in a fight, and stingers on events | 116 |
| 118 | Streaming long tracks | |

**Spatial**

| # | Item | Waits on |
|---|---|---|
| 119 | Doppler, for fast things like rockets | |
| 120 | Sound that goes round corners and through doorways, not only by line of sight. Item 23 goes with this | |
| 121 | Reverb set by a zone, over what the rays measure | |

**Access**

| # | Item | Waits on |
|---|---|---|
| 122 | Subtitles and sound cues on screen | 1 |
| 123 | More sliders, such as voices and interface sounds | |

Voice chat and surround sound systems aren't wanted.

## Sky

Today the sky has a gradient, a sun and one flat cloud layer, from a picture or made in the shader. A window's stencil can take the sun as its light, but nothing comes from the sky itself.

| # | Item | Waits on |
|---|---|---|
| 63 | Planes in the sky for clouds, curved or flat, and more than one | |
| 64 | Sky ray stencils, so rays come down through the sky and clouds. Research the ways and what they cost: skies made with cheap god rays, or stencils generated to match generated clouds for a few. An author's choice, for each sky. By default they stay in the sky and don't reach the level, unless an author wants them to | 63, 62 for what reaches the level |
| 65 | A sun that moves, and a moon: light from one direction that reaches the ground and its specular, with shadows, by day and night. Research how, and what it costs. Today the sun is baked, and it's set once in a map | |
| 66 | Day and night: the sky, sun, moon and light going through a day, set by an author or a script. Nothing does this today | 65 |
| 67 | Open world light in the style of older online worlds: sets of sky, cloud, fog, water, ambient, sun and specular colours, keyed through a day. Each map has one, zones can have their own, and they blend by where you stand and how far in | |
| 68 | The human's shadow heat map: the sun moves through a set arc of some degrees, and the bake keeps, for each sample, at what angles it's in shadow. Shadows then deepen and retract slowly, with no new bake and no live sun. Research how many angles it can hold, what it costs to bake, and thin things. It's for the final bake only, not every build, as it's likely expensive at production quality | 67, 69 |
| 69 | A final bake, kept apart from everyday builds, for what's too slow to run each time. Everyday builds use a quick bake or none, with item 53's run mode. What only a final bake shows has to be checked before a level's done, so the editor should be able to run one on part of a level | |

For 65 to 67, `solarityclient` shows the kind of thing, as a reference and not to copy: `crates/asset/src/database/light`. A map has a global set and local volumes with a falloff, each with 16 keys through a day, and the light's direction is worked out from the time and barely moves. So most of a day is colours changing, which is cheap. That may suit us: a baked sun on its own layer could change colour through a day with no new bake. Whether it holds up is for research.

## Light on screen

Added by the human, 2026-11-07. Scenes are drawn in 16 bit floats with an exposure for each map and zone, but what's over 1 only has its colour scaled down to fit, and what reaches a screen is 8 bits. A sun and a day need more than that.

| # | Item | Waits on |
|---|---|---|
| 163 | SSAO. Research which kind, what it costs, how it sits with baked light, which already has soft corners, and with a live sun, which doesn't. Whether it's a player's choice, an author's or both | 65 |
| 164 | A tone curve, so that bright light rolls off softly and doesn't hit a ceiling. Comes before the moving sun, so that a day is tuned once, on the curve it'll have. Research which, and whether a level can choose its own | |
| 165 | Exposure that adjusts by itself, as eyes do, going from a dark room out into the sun, with limits an author can set. Zones already set their own, and the two have to work together | 164 |
| 166 | Bloom, so that what's bright glows, the sun among them. There's none | 164 |
| 167 | HDR all the way through. Find what cuts brightness off before the final pass, and take away what needn't: light on a surface stops at 2, and research how baked light is stored. Then put back bloom's weighting of bright looks (item 166) and see a test fail, which none can while light stops at 2 | 164 |
| 168 | HDR out to a screen that has it. What's sent today is 8 bits a colour. Research what it takes, and what happens on a screen that hasn't | 164, 167 |
| 172 | Anti-aliasing that's on. Edges step in every picture, as what's there is all off unless a player turns it on: several samples a pixel (`r_msaa`), a quick blend (`r_aa 1`) and SMAA (`r_aa 2`). Research what each costs here and how each looks, choose what's on by default, and put them in the options with names a player knows. The human, 2026-11-07: the geometry has a lot of aliasing, and the tools to deal with it should all be there | |
| 173 | Temporal anti-aliasing: the view shifted a little each frame and blended over frames, by how things moved, so that thin things and shimmer settle. Research ghosting on what moves, what moves needs to say how it moved, and how it sits with the noise the rays, fog and SSAO passes lean on to blur | 172 |
| 174 | Shading that shimmers: highlights and bumps that sparkle as they shrink with distance. Research what's done, such as taking a bump's spread into its roughness, and what textures' mips and filtering need | 172 |
| 175 | Cut-out edges: fences, grates and leaves step and crawl, as several samples a pixel don't smooth what's cut out. Research alpha to coverage, and keeping how much is covered through mips | 172 |
| 176 | Drawing at another size: above the screen's, for pictures and strong cards, and below with upscaling for weak ones. Research what upscaling there is to use, and how it goes with item 173 | 173 |
| 177 | Thin things that fall between pixels, such as wires, poles and far edges, which the rest may not help. Research what does, once 172 to 176 are in | 173, 174, 175 |

## Unfinished work in the real levels

Some of this, and of what's elsewhere on the list, will come down purely to authoring: scripts, textures, maps and shaders, not engine work. Each item says which when it reaches the workflow.

| # | Item | From | Waits on |
|---|---|---|---|
| 18 | Effects in real levels. No map places one and no script starts one, and that includes the waterfall and splashes | The audit | |
| 19 | Water at its best in real levels | The human | 17 |
| 20 | Meshes in real levels, starting with the well, which is why mesh work began | The audit | |
| 21 | Music in zones. No map sets any | The audit | |

## Smaller limits left in

| # | Item | From |
|---|---|---|
| 23 | A closed door stopping sound, and loops moving between muffled and clear | [Proposal 0129](../proposals/0129-real-sound.md) |
| 24 | Trains as a way for a path to go | [Proposal 0122](../proposals/0122-paths-that-leap.md) |
| 25 | Fog by zone | [Proposal 0116](../proposals/0116-zones.md) |
| 62 | Rays from windows and stencils go through floors, where they should stop. It can be seen in Vesper Deep. The likely cause: a ray stops at what's in front of the eye, and each box is only traced once, so nothing checks what's between a point in the air and the stencil | The human |

## Texture spec

| # | Item | From |
|---|---|---|
| 26 | Rewrite [the standard](../texture-standard.md), which is poor and mostly covers tiled sets | The human |

What it has to cover:

- Textures that aren't tiled: UI art, frames, icons, cursors, splash and title art, and painted scenes.
- How they're packed, and the system that saves space.
- Kitbashing: pieces made to be combined, and sheets shared between them.
- What a model can make. Its line saying to ask for colour only is out of date, as models can make specular and other maps too.

The test repo's art is an older example, to give an idea of what's possible, and not something to copy. The spec is designed for this engine. `test/assets/source/ui` shows the kind of thing:

- Sheets with a JSON file naming each region, and a border for nine-slice.
- Each UI piece in normal, hover, pressed, focused, selected and disabled.
- Scenes with depth, occlusion and region maps beside the picture.
- The prompt kept beside what it made.

## MCP

So Claude can use it well. A test on 2026-11-06 found the loop works: a level built only through MCP compiled, ran and looked right. These are the gaps it found.

| # | Item | Why |
|---|---|---|
| 31 | Real schemas for all 42 tools | All are empty, so an agent guesses at arguments |
| 32 | Validation that catches bad content, when it's added and in `validate` and `compile` | An unknown type, missing effect or music, a zone with no shape, an inverted brush, an unknown block and a misspelt field all pass, and a missing material still counts as success |
| 33 | `shot` and `playtest` report what really happened, and a shot can say where it looks from | With a map outside the content, `shot` said ok and showed the Quake menu. `playtest` only says the game stayed up |
| 34 | One rule for paths: from the content, and staying inside it | They go from where the server started, and `save` can write outside |
| 35 | Find and filter, in place of everything at once | `describe` is 2 MB on Vesper Deep |
| 36 | Undo that keeps changes, not whole maps | 22 MB became 419 MB after 40 edits on Vesper Deep |
| 37 | Tools to write shaders, effects and prefabs, compile textures and build packs | None exist |
| 38 | An answer to a malformed request, and tests through MCP itself | It gets no reply, and the 51 tests only reach the operations |

## The editor

The human's direction, 2026-11-06: item 18's floor plan went the wrong way. This is a starter on what it should be, not a design, and it goes through the workflow later. Anyone being able to make maps quickly is still the goal. It has to be organised and easy to use.

| # | Item | Waits on |
|---|---|---|
| 39 | Start on a plane, with the infinite horizon and sky | |
| 40 | A player in third person when needed, for scale, who can move around for live checks. Quake's model for now, until we have our own | |
| 41 | Sky tab: choose or make sky scripts, sky spheres and horizons | |
| 42 | Good default skies with variance | Authoring, later |
| 43 | Mesh terrain, which the engine doesn't have yet. See items 125 to 130 | Mesh support |
| 44 | Terrain tab, with proper support for all of what land can do. Today the editor can shape, cut holes, fill them, paint and flood, and works on a sheet facing down, but it can't make a sheet or add one, or say which way it faces or how thick it is. Those are only in the tools. It has to do those, and items 169 to 171 | 43, 169 to 171 |
| 45 | A real circle on terrain and geometry, as the brush for sculpting and for painting textures, with our layers and alpha blending. A must have | |
| 46 | Prefabs tab, with two sets: primitives and meshes | Mesh support, for the meshes |
| 47 | Primitives that snap together: a ramp with walls, dressing and moulding, rooms, blockouts and basic items | |
| 48 | Generated thumbnails of every item, to see what's being chosen | |
| 49 | Placing as a see-through ghost, with optional snapping | |
| 50 | Proper widgets to move, rotate and resize | |
| 51 | Railings and the like that take a texture, showing its detail and see-through between | |
| 52 | Working together in real time: the human sees what Claude does in the editor as it's done, so MCP joins the open editor | 31 to 38 |
| 53 | A run mode for testing, with no compile step | |
| 54 | Toolbars of small icon squares in the viewport, for path drawing, swapping to the camera and the like | |
| 55 | Remove the floor plan: you can't really tell what you're making on it, so it isn't useful. That's `easy.rs`, `plan.rs`, the `plan_` tools and its part of `docs/editor.md`. Bigger, Plain colours and working by keyboard or mouse alone are kept for the new editor | 39 |
| 61 | Camera setup tab | 57, 155 to 162 |

Real maps are expected to mix meshes, mesh terrain and primitives.

For 52 and 53, what exists: the game already runs a text map with no bake, bright and flat, and the editor's view uses the same renderer. It doesn't run the game, it rebuilds the whole map on a change, and nothing reloads a level in a running game. So a big map needs rebuilding only what changed, and a quick look at light.

To settle in design:

- Stairs, zones, effects, a screen reader, and what else needs a home in the tabs.
- What waits for mesh support.

## Cameras

Added by the human, 2026-11-06. How the usual third person camera works is research. The view is Rust in `view.rs` today, and scripts can only turn it with `engine.set_view`.

| # | Item | Waits on |
|---|---|---|
| 56 | Cameras as scripts. First person and the intermission become scripts, even if they carry over as they are | |
| 57 | A way to set up cameras: make one, place it and choose which is in use | 56 |
| 58 | A core third person camera, with a spring boom, pushing in at walls, snapping in, and the rest of what makes one usable | 56 |
| 59 | Scene cameras: a fixed one like the intermission's, or a set play that moves along a path | 57 |
| 60 | The intermission camera: static, by the human's choice, after checking videos | 56 |
| 155 | Cameras that follow something other than the player, such as a monster, a moving door or another player | 56 to 60 |
| 156 | A camera's own field of view, which can change as it goes | 56 to 60 |
| 157 | Shots in a row for a cutscene: cameras in order, with cuts and blends between them, and a way to skip | 59 |
| 158 | Camera shake, from what happens near it, on whichever camera's in use | 56 to 60 |
| 159 | A free camera for watching, which isn't tied to a player | 155 |
| 160 | A script driving a camera frame by frame, past the kinds there are | 56 |
| 161 | A view shown on a screen in the world, from a camera in the level | 155 |
| 162 | Whether the player's body shows goes by where the camera is, not its kind, and a camera doesn't need the player moved to it. It comes from the end of level camera showing part of the player | 56 to 60 |

Third person is likely to be the more usable camera in the editor, so items 40 and 54 lean on 58.

## Going back to what was said

The audit found these. The human said to take every item with nothing left out, so each goes back to what was first asked for, and the proposal says so.

| # | Item | From |
|---|---|---|
| 149 | Your own shots aim for half of the gap, as the human said, in place of [proposal 0126](../proposals/0126-own-shots.md)'s all of it | The human, 2026-11-04 |
| 150 | Tighten the rewind allowance. [Proposal 0128](../proposals/0128-network-numbers.md) took it from about 42 ms to 114 ms over a compensated ping, which a cheat may be able to use | The fair play rules |
| 151 | Doors: the revamp to better logic the human asked for, with the uneven closing delay and the pause then snap dealt with. Brush models don't turn when drawn or collided with, which a door that swings needs, so research that here, and an effect on a model turns with it then: places already do (proposal 0143) | The human, 2026-11-04 |
| 152 | Finish the review: items 1 to 17 of the sprint were held to their tests, not reread | The sprint's final rounds |
| 153 | Enemy models are choppy, and always have been. They move ten times a second, and were only blended over a tick | The human's play session, 2026-11-07 |
| 154 | Level pictures clutter the repository: ignore them or put them in a container | The human, 2026-11-07 |

## Not on the list

Moving lights' shadows are soft, with no volumes. The human had a look and they're good enough for now. They may be revisited in a second pass at some later time, and aren't on this list until then.

Player setup, in the editor, is likely to be wanted. That's future work, after rendering is done.

Colour isn't engine work for now: sRGB made it in. Nothing we've made is worthy of showing it, so it can't really be inspected yet, and that's still to be judged once something is. It comes back if something turns out missing or low quality. Number 22 was this, and it's left unused so the others keep theirs.

## Log

Newest last. One line for each thing finished, with its commit.

- Items 149 and 150: 2273c58
- Items 3 and 5: 9ec5198
- Item 4: 0605bb7
- Items 29 and 30: 4bb7867
- Item 74, and the first step of 77: c9d0c1c
- Items 26 and 133, and item 77 mapped: 70646ee
- Items 31 to 38: f65b7c8
- Item 124 begun: 4881c8d
- Items 56 to 60: 71b1a6a
- Item 60 made static, by the human: 292cee9
- Items 1, 2 and 6: 5fc5127
- Item 7: c193fd5
- Item 8: c29a57e
- Item 9: e384d75
- Item 10: cc6a580, fda7c89
- Item 11: 7ef5130
- Item 12, alias models stay: c90dd80
- Item 12 reversed, on the human's decision: what moving meshes cost, 2e2190d, and the fixture's models moved over, 4cccc4e
- What meshes left undone, proposal 0143: depth pass 37577d5, rest weights and the like a659311, a copy's triangles 27edc1e, levels for what moves 8d99720, hit volumes d9dd133, a normal map's lean 9620995, vertex colours 09855dd, see-through meshes 5c8448f, clips online 54eb373
- Stage 8, terrain, proposal 0144. Items 43, 125, 126 and 129: 534b356, a68c660. Items 127 and 45: 7aba6c2. Items 128 and 132, and item 131 for land: c9ca88e. Item 131 for materials: f2fbd89. Tiles and scale: e98e57c. Item 130 waits on item 17, at stage 11, and item 44 is at stage 15, as the Order has them
- Stage 9, shaders, proposal 0145. Item 13: cf65589. Item 14: 32e73e3
- Stage 10, effects, proposal 0146. Items 78 to 85: 4b6c67f. Items 86 to 89 and 91 to 93: 8c1d46d. Item 90: 0ecd8ed. Item 94: ad358e2. Item 95: 32551c8. Item 96: faa09d5. Item 97: 9af1bae. Item 16 and the rest of 89: 8ae2ac7. Item 9 was done at stage 7
- Stage 11, fog and water, proposal 0147. Items 70 to 72: e5bf844. Items 73, 75 and 76: b85f395, and item 74 needed no change. Items 17 and 130: 10a9853. Items 100 and 101: 3a62e97. Items 98, 99 and 102: b4c914f
- Item 62: f36817b. Item 63: 17e7eef. Item 64: 95cb9fb
- Item 153, enemies going evenly: 369f8e0, and the human found them smooth. Item 154, level pictures out of the repository: 332dc60
- What the play session found: level text in words, and the body at the end of a level: f41e2a8
- Item 69: 21406ed
- Item 164: ac79e4b
- Item 65: 50eb81b
- Items 66 and 67: 3551b08
- Item 163: 09aef88
- Item 68: 075183f
- Item 165: 106e0d5
- Item 166: 5a5c461
- Item 167: 5ea7c7c
- Item 168: 0bf669b
