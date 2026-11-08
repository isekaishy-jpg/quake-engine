# Proposal 0151: anti-aliasing

Status: accepted for items 172 to 176, 2026-11-07. Item 177 of the [work list](../research/work-list.md) is added to this when it's reached.

## Why

The human, looking at pictures of shadows: the geometry has a lot of aliasing, and the tools to deal with it should all be there. Every picture stepped because what there was, was off unless a player turned it on, and none of it was in the options.

## What there was

| Setting | What it does |
|---|---|
| `r_msaa` | Several samples a pixel, 2, 4 or 8, of what's solid. [Proposal 0082](0082-antialiasing-and-alpha.md) |
| `r_aa 1` | A quick blend across what looks like an edge |
| `r_aa 2` | SMAA 1x. [Proposal 0093](0093-what-increment-11-left.md) |

"Smooth Edges" in the options was neither: it's `r_edges`, which makes hard corners look a little rounded.

## How each looks and what it costs

Measured at the start of Vesper Deep, in a window, from `gpu_times`:

| | What it adds | How it looks |
|---|---|---|
| None | | Stairs along every edge against the sky |
| Quick blend | 0.01 ms | A light halo round what's dark against the sky, and the stairs stay. It reads half a pixel across an edge, which only blurs it |
| SMAA | 0.25 ms (see 173: first measured as 0.01 to 0.02, when no span of `gpu_times` held it) | Smooth, and what's inside textures stays sharp |
| 2 samples | 0.15 ms | Smoother than none, with some stair left |
| 4 samples | 0.3 ms | Close to SMAA |
| 8 samples | 0.6 ms | The smoothest edges, and the most to pay, more on a bigger screen |

## What changes

- **SMAA is on unless a player turns it off.** It's as cheap as the quick blend and better than two samples. It's `r_antialias` now, a new name for `r_aa`, so a config of this engine that had it off, as every one did, takes the new usual. A config's old `r_aa` line is passed over quietly.
- **The options have both, by names a player knows:** "Anti-aliasing", Off or SMAA, and "Multisampling", Off, 2x, 4x or 8x. The two go together.
- **"Smooth Edges" is "Rounded Corners",** which is what it does.
- **The quick blend is still there,** as `r_antialias 1` at the console, and not in the options, as SMAA is better for the same.

## What came of it

### Anti-aliasing that's on (172)

- **Tried in the game:** each kind side by side at the start of Vesper Deep, and a config from before, with `r_aa 0`, starts with SMAA on and says nothing of the old setting.
- **A row in the options can set a setting to one of its own values** now, which choices couldn't, as they kept theirs in the menu. Turning one goes round, and a value it doesn't list counts as its first.
- **On a short window the menus go to two columns,** where a row says what it's set to beside its name. The video screen never does, so nothing tests a picking row that way.

## Temporal anti-aliasing (173)

### What it does

`r_antialias 3`, "TAA" in the options. Each frame the view moves by less than a pixel, round eight places from Halton's sequences in 2 and 3, which cover a pixel evenly. Each pixel then takes a tenth of what's new and keeps nine tenths of what it had, found where it was last frame. Over a few frames an edge gets every shade between its sides, and what shimmers, such as thin lines and far detail, settles.

What was kept can be wrong: something came out from behind something else, or the light changed. So what's kept is held to what the pixels round it show now: their mean, give or take 1.25 times how much they vary, in YCoCg, where brightness and colour are apart. Bright light is squeezed before it's compared and blended, so that a speck far brighter than a screen shows doesn't flicker as it moves.

### Where a pixel was

- **What doesn't move** went where depth and the view say. Both "now" and "last frame" are worked out with the view as it is, without that frame's move. A first try took "now" from the view that was moved, and every pixel found itself a fraction of a pixel off, so a view that stood still blurred more each frame. On a black and white floor, one frame after the view moved was 145,000 off the frames after it, and 51,000 after this.
- **What moves** is drawn again, into an image of its own: models, moving meshes, and brush models that moved since the last frame. Each is drawn where it is and where it was, and the image says how far it went across the screen and how far it is from the eye. A pixel goes by that only if the distance is what the scene has there, within 2% or a unit, so something moving behind a wall doesn't move the wall.
- **Who's who:** a model or moving mesh says who it is with an id, the entity's, and where each was last frame is kept by that. The player's weapon has an id of its own. An id of 0 is no one in particular, and goes where depth says.
- **The weapon** is kept in front by squeezing its depth, so the distance that depth gives isn't the one it is. The motion image works its distance out the same way, from where it's drawn, so the two still agree.

### What it does to the rest

The noise rays, fog and corners are blurred with moves on each frame, so that TAA averages it away.

Off unless a player picks it. SMAA stays the usual one, as TAA softens a little, and it ghosts on a model's own animation (below).

### What came of it

- **Tried in the game:** E1M1 with TAA on looks smooth and a little soft. The weapon stays sharp while turning, and nothing trails behind.
- **What it costs:** `gpu_times` has an `edges` span now, round SMAA, TAA and the motion pass. Before it neither was timed, so the 0.01 to 0.02 ms that 172 put on SMAA was only what it took from the final pass. At the start of E1M1, in a window, TAA takes 0.38 ms and the final pass 0.04 ms less, as SMAA isn't run; SMAA takes 0.24 ms and the final pass 0.05 ms more. Over none, that's about 0.35 ms for TAA and 0.25 ms for SMAA. SMAA stays the usual all the same: it's cheaper than four samples, which it looks like, though not the next to nothing 172 took it for.
- **A new size starts afresh.** The frame values were written before the scene's target was made, so a frame at a new size said there was something kept when its kept image was new and empty. The target is made first now.
- **What isn't followed:** how a model moves inside itself, as it animates from pose to pose or a mesh bends at its joints, isn't in the motion image. Only how it moves as a whole is. There the clamp is what holds ghosting down. Particles, sprites and what's see-through don't write depth, so they go where what's behind them went.
- **Tests:** the view moves by less than a pixel, edges settle and keep their colours, and turning it off leaves nothing behind. As the view moves, as a brush model, a model or a moving mesh moves, and with the weapon held in front, one frame after is near what many frames after are. Without an id, the weapon is fifteen times as far off. Something moving under the floor leaves the floor alone. A moving mesh and a model with the same number and id are told apart, what's left where something was is let go of at once, and a new size starts afresh. Each bug put back, 24 of them, fails a test.
- **What no test reaches:** the game passing each entity's id to what it draws, and `r_antialias` going up to 3, as no test runs the game with a renderer; in the game TAA is on at 3, and the weapon, by its id, stays sharp as the view turns. Leaving a brush model out of view out of the motion image only saves work, as the distance check would pass it over anyway.

## Shading that shimmers (174)

### Why it shimmers

A highlight is worked out once a pixel, at its middle. Where the surface's normal turns a long way across a pixel, from far off or on bumps, a sharp highlight falls between pixels' middles or on one of them, and it flashes as the view moves. Anti-aliasing doesn't help: SMAA looks for edges, and TAA averages what flashes into a dimmer crawl.

A normal map's mips make it worse. Averaged, bumps leaning every way face out, so far off the surface is flat to a highlight, and as sharp as a mirror, where the bumps it can't show would have spread it.

### What's done elsewhere

- **Geometric specular anti-aliasing** (Kaplanyan and others, 2016; Tokuyoshi and Kaplanyan, 2019): how far the normal turns across a pixel, from its screen derivatives, is added to the roughness, squared, up to a limit, so a highlight is never smaller than a pixel.
- **Toksvig's** (2004), and LEAN and CLEAN mapping after it: the average of unit normals is shorter the more they spread, and how much shorter says how much rougher a highlight on them is. It needs the average kept as it is, where mips normally make it a unit again.
- **Rougher mips of a roughness map**, from the normal map's spread, which needs the two kept together, the same size, and not shared.

### What's done here

- **Both, on every highlight:** a material's specular map, its metal and roughness, and painted land. `steadied` takes a gloss and how much rougher it should be, and gives the gloss and what the same light is multiplied by, as a wider highlight is dimmer. Where nothing turns it gives the gloss as it was.
- **How far the normal turns** is from its derivatives, as the 2019 paper has it, limited to 0.18 of roughness squared.
- **How much bumps spread** is kept in a normal map's mips, in blue. The shader works a normal's third part out from red and green, so blue was free. Each level down averages every texel of the first level under it, keeps which way they face in red and green, and how long the average is in blue. The first level keeps its red and green as they are. Toksvig's length then adds 2(1 - length) / length to roughness squared. That works whatever a material's roughness comes from, a map or a number, which a rougher roughness map wouldn't.
- **Compiled normal maps are BC7 now,** as BC5 has only two channels. They're the same size. A normal map compiled as BC5 is old whatever its source, and is compiled again; a game reads its source meanwhile.
- **`r_steady_shine`,** on unless a player turns it off, as it only takes away what flashes.
- **A picture that's a normal map in a material and something else too,** such as its height, takes two slots now, as its mips aren't the same. No material here does that.

### What came of it

- **Tried in the game:** a floor of a small shiny metal tiled with brick bumps, under a low sun, with anti-aliasing off. Without it the brick edges were full of white specks, 128 pixels at the top in a strip of the floor. With it there were none, and the highlight was a sheen. It costs nothing measurable: the scene took 1.806 ms and 1.807 ms.
- **Tests:** bumps too small to see, two texels to a pixel, take a sharp highlight from 233 to 64, and metal said to be smooth shines as the rougher thing it looks. Far off, where a floor turns across a pixel, its brightest swings by 192 as the view moves across a pixel, where steadied it swings by 14 and never drops below 55. Rippling land's light flickers by 18% as the view moves, and by 1.3% steadied. Near, a highlight is much as it was, and a flat surface's exactly. A normal map's mips keep how much its bumps agree, from the first level and wrapping at an edge; a compiled one has the same levels; a mega texture's does; and one compiled as BC5 is compiled again and not read.
- **Found on the way:** the test maps have no tangents, so a normal map's direction does nothing in the GPU tests, only how much its bumps agree. Turning there is made with corners' normals, and with land's heights. Land far off is drawn with fewer points, so a ripple a point across is gone before it can sparkle; one a few points across isn't.
- **Layers' bumps** go into a surface picture whose blue is height, so how much they agree isn't kept. Where a layer covers a surface, how much the base's bumps agree counts for as much as the base shows, and the rest goes by how far the normal turns.

## Cut-out edges (175)

### What there was

- **A masked material** already covered some of a pixel's samples by how far alpha is past its cutoff, sharpened to a pixel's width, so with several samples a pixel its edges had shades between. With one, as is usual, it's all or nothing.
- **One of Quake's pictures with holes** had none of that: its shader threw away what's under half, so samples didn't smooth it. Nor did sprites.
- **Mips** average alpha, so a bar thinner than a texel of a mip falls under the cutoff and goes: far off, fences and leaves thin out and vanish.
- **Shadows** were solid. A shadow map draws depth alone, and the bake's light tree had what's masked as solid as a wall, so a grate cast a black square. The human saw this on trying it.

### What's done

- **Mips as solid as the first level** (Castaño, 2010): each level's alpha is scaled until as many of its texels pass the cutoff as the first level's do, or as near as whole texels let it, the solider of two as near. That's for a masked material's picture as a level loads, and as it's compiled, which now says it's a cut-out, with its cutoff, so a picture compiled as colour before is compiled again.
- **Quake's pictures with holes and sprites** are drawn as masked materials are, at a cutoff of half, so several samples a pixel smooth their edges too.
- **One sample a pixel stays all or nothing.** TAA moving the view each frame moves the edge with it and smooths it by itself, and SMAA smooths the picture it's given; a first try that dithered the cutoff with TAA was only noise over what TAA does anyway, and was taken out.
- **Light goes through holes.** Shadow maps draw what has holes a picture at a time, reading it and leaving out what's under its cutoff: the world's faces, brush models, moving meshes and copies of meshes. The bake takes a masked brush out of its tree and has light cross it as it crosses glass, through where its picture is a hole and stopped where it isn't, and a mesh's triangle with holes is looked up where a ray meets it.
- **A screenshot with TAA** was the first frame at its own size, which TAA has nothing to go on for. It's drawn for a few rounds first now, as on screen.

### What came of it

- **Tried in the game:** grates near and far against the sky. With TAA settled, SMAA or several samples their edges are smooth, and the far ones are still grates. Their shadow in a live sun is a lattice, where it was a black square.
- **Tests:** scattered spots, as leaves are, stay within 7% of as solid down three levels, where averaged they thin to under half; thin bars stay; the cutoff is the material's; compressed levels are left alone. A masked material's mips keep as solid, as compiled and as a level loads, and a picture that becomes a cut-out is compiled again, and once more for another cutoff. With several samples a pixel, one of Quake's pictures with holes and a sprite get shades at their edges. Light goes through a cut-out's holes and is stopped where it's solid, for a brush model, one of Quake's pictures, the world, a moving mesh and a copy of a mesh, and in the bake for a wall and a mesh.
- **Found on the way:** every screenshot taken with TAA, including those that 173 was judged by, was a frame with nothing kept. E1M1 taken again, settled, is as 173 says: smooth, and a little soft.
- **Quake's own pictures with holes** have their own mips, made by its tools, and alpha all or nothing, so scaling them changes nothing. They keep theirs.
- **What no test reaches:** a screenshot settling with TAA, as no test runs the game with a renderer; it was seen in the game, where the grates and E1M1 above were taken that way.

## Drawing at another size (176)

### What's done elsewhere

- **Drawing above the screen's size** and taking each screen pixel from those under it, supersampling: the surest anti-aliasing, at twice the cost of pixels for a half again each way.
- **Bringing a smaller picture up** by a filter alone: bilinear, which is soft; a cubic such as Catmull and Rom's, sharper; AMD's FSR 1, an edge-following one with sharpening after, which is MIT-licensed and a few hundred lines of shader.
- **Bringing it up over frames**, with what TAA keeps at the screen's size: Unreal's TSR and TAAU, AMD's FSR 2 and 3, Intel's XeSS and NVIDIA's DLSS. XeSS and DLSS need their makers' libraries, and DLSS a card of its maker's. FSR 2 is open, and many passes, of which this engine's TAA already does the heart: moves of the view, what's kept found where it went, held to what's round it. What it adds is each of the frame's pixels taken where it truly is, by how near that is to each of the screen's, textures read as finely as the screen's size would, and sharpening after.

### What's done here

- **`r_scale`,** "Render Scale" in the options, from 50% to 200%, 100% unless a player says. The scene and all that's drawn at its size are that much of the screen each way: light in the air, how open corners are, what glows, SMAA and what moves. The final pass and TAA are the screen's size, and everything over the view after.
- **Above 100%** the final pass takes each pixel from four blends across the scene's pixels under it.
- **Below 100%, without TAA,** it's brought up by Catmull and Rom's curve, from five blends, kept between the four nearest pixels so that an edge doesn't ring. With SMAA, SMAA's picture is blended between the four nearest.
- **Below 100%, with TAA,** TAA keeps the screen's size. Each of the frame's pixels round a screen pixel is where what it saw truly is, with the view's move taken out, and counts by how near that is in the screen's pixels, by a window near enough Blackman and Harris's; one that's near counts for more of what's kept. The moves go round more frames the smaller it is, eight over the scale squared, so that more places in each pixel are seen. Textures are read as finely as the screen's size would have them, and what TAA makes is sharpened after by how much contrast there is round it, as AMD's RCAS does, held between the pixels round it. Without TAA finer textures would only shimmer, so they're as the scene's size says.
- **Capturing and screenshots** draw at the scale, at the size asked for.

### What came of it

- **Tried in the game,** at the start of E1M1 in a window, with what `gpu_times` says. With SMAA: at 50% the scene takes 0.12 ms, against 0.31 at 100% and 0.6 to 1.0 at 200%, which also puts SMAA up to 0.8 ms. With TAA at 50% the scene, final pass and edges take about 0.6 ms together, against about 1 ms at 100%. Against TAA at 100%, a picture at 50% with SMAA is off by 1.9 a pixel and TAA at 50% by 1.2, as sharp as SMAA's at a third less off; it went from 1.6 to 1.4 when how near was measured in the screen's pixels and not the scene's, and to 1.2 with textures read finer, and sharpening brought its sharpness from 1.47 to 1.67 of TAA at 100%'s 1.85.
- **Found on the way:** the scene's passes were told the screen's size for what to draw into, and drew a corner of it; TAA's move was read from a part of the view's matrix that's nothing when the view's level, and came out black.
- **Tests:** a scene at half and at twice the size fills the screen and is as it was where it's flat, twice smooths edges, and 0 is the screen's size. At a half again, a fine pattern is averaged under each pixel, to under a third of it, where one blend leaves more. What's brought up doesn't ring past the pixels round it. TAA at half comes under half as far from TAA at the screen's size as bringing up alone, and sharpening makes its edges a fifth as steep as the screen's size's, all told, where without it they're under that (177 changed how this is measured); any of its parts undone, or the shorter round, comes further off, the round by only 5%, so that test's margin is narrow. Textures are read finer drawn smaller only with TAA. SMAA comes along when it's brought up. The options show it and turn it.
- **What no test reaches:** `r_scale` held between a half and twice in the game, as with every setting beside it.

## Thin things (177)

### What there was

Something thinner than a pixel, a wire, a pole or a grate's bar far off, is in a pixel only where a sample falls on it. With one sample it comes and goes as the view moves, and SMAA can't help, as in a frame it isn't in there's no edge to find. A test of a black strip a third of a pixel wide over the square, the view creeping a twentieth of a pixel a frame, measured how much of it shows as it's shown and how much that swings:

| | Shows | Swings |
|---|---|---|
| Nothing | 883 | 1310 |
| SMAA | 883 | 1310 |
| 4 samples, before | 4 | |
| TAA, before | 11 | |

So samples and TAA both lost it, where they should be the answer. Each had its own reason, found by taking frames apart:

- **Samples were averaged as light, not as they're shown.** The square's light is 2.7 times what the screen shows at its brightest, as a bright wall in a level made for this engine is, and the final pass brings a colour that's past that down to fit, keeping its hue. A pixel with three of its four samples on the square and one on the strip averages to twice that brightest, which is shown as the square. Only a pixel nearly all strip looked any darker, so the strip was eroded to almost nothing, and a white strip on a dark floor was widened.
- **TAA blended as light squeezed by Karis's 1 over 1 and how bright,** which is milder than the final pass, so the same happened more slowly.
- **TAA held what it kept to the mean and spread of the pixels round it,** and a line a pixel wide is so unlike what's round it that the spread left it out: a still black line came to rest at 31, not 0.
- **TAA read what it kept blended between the four nearest pixels.** As the view moves a fraction of a pixel a frame, that softens it frame on frame, and something thin spreads into the pixels beside it, where the box cuts it, and fades.
- **TAA alone can't keep what most of its frames don't see.** The strip is in three of eight moves of the view. In the other five there's nothing dark round it now, so the box closes on the square, and what was kept of it goes.

### What's done elsewhere

- **Resolving samples as they'll be shown:** Pettineo's experiments with MSAA resolve filters, and Karis, who weighs each sample by 1 over 1 and how bright before averaging, then takes it back. Unreal and others resolve their own way for the same reason.
- **Reading what TAA kept by a sharper curve** than a blend: Catmull and Rom's, in five reads, from Jimenez's Filmic SMAA, which Unreal's TAA also uses.
- **Keeping thin things with TAA alone:** Unreal's TSR spots thin geometry and how much it flickers, and trusts what it kept more there; other TAAs trust what's kept where nothing moved. Either keeps a trail of what passes in front of something still unless it also knows what was where last frame, which this engine doesn't keep.

### What's done here

- **Samples are brought to one a pixel by a pass of their own,** after everything's drawn into them, in place of the card's resolve. Each is tamed as the screen will show it, near enough, averaged, and taken back to light, so that what's brighter than the screen shows stays as bright for what glows, exposure and TAA.
- **Taming** is shared by that pass and TAA. It goes by the brightest of a colour's channels at the light that reaches the eye, against the brightest the screen shows, as the final pass does. Below 0.9 of that, light is as it is, so what's within the screen is averaged as plainly as before; above, it rolls off smoothly towards the brightest, never reaching it, so it can be taken back. Karis's 1 over 1 and how bright is the same curve started at 0; started at 0.9 it's near the final pass's, and a pixel a quarter dark comes out 0.74 of the way, against 0.75 shown sample by sample.
- **TAA's box always holds what this pixel is now.**
- **What TAA kept is read by Catmull and Rom's curve,** in five reads, and the box takes out what it overshoots.
- **TAA alone is left losing what most of its frames miss.** Trusting what it kept where nothing moved would leave a monster's trail on a still wall, and this engine keeps nothing of where things were last frame to tell. The answer here is samples, with TAA over them or not, and the options say so: "Multisampling" keeps what's thinner than a pixel, far off.

### What came of it

| | Shows | Swings |
|---|---|---|
| 4 samples | 510 | 198 |
| TAA | 85 | 63 |
| TAA over 4 samples | 336 | 60 |

- **Each change carries part:** with Karis's taming, samples showed 211; taming from 0.5, TAA over samples showed 68, and from 0.9 but with what's kept blended, 140.
- **While the view turns,** TAA is nearer what's drawn without it than before: 2.0 million off in all, against 2.8 with what's kept blended and 2.5 with Karis's taming, and over samples 1.2, against 2.4 and 1.7.
- **Tried in the game,** the grate yard from 175, from one place, with nothing, 4 samples, TAA over them, and TAA. With nothing the far grate breaks into pieces; with samples it's an even mesh, and the sky and snow are as bright. It was watched while the view was turned and while the game shared the machine with tests, which made shadows seem to creep and flicker; they didn't once it had the machine to itself, and the same frames taken on the commit before showed the same.
- **Found on the way:** the research before this measured how dark the whole picture was, and with the strip on a square that filled a third of it, what it measured was mostly the square's edges. A thin strip over a floor much wider than the light map drew dark beyond it, as the test map's light covers only the square, so the test keeps to the square.
- **Seen and left for later:** a dark line along the top of a far grate, which comes and goes as the view tips. It's on the commit before too.
- **Tests:** the strip with 4 samples shows, steadily, where with one it comes and goes and SMAA doesn't help, and TAA over samples keeps half of it, steadier. With TAA a still line a pixel wide stays black. Light past what the screen shows glows as much with samples as without. Half the light at twice the exposure is resolved the same at a strip's edges. Each change undone fails one of them.
- **What no test reaches:** HDR screens, where taming goes by their brightest, which needs a person with one.

## Sources

- [Proposal 0082: anti-aliasing and alpha](0082-antialiasing-and-alpha.md)
- [Proposal 0093: what increment 11 left](0093-what-increment-11-left.md)
- Brian Karis, "High Quality Temporal Supersampling", SIGGRAPH 2014: moving the view round a pattern, and squeezing bright light before blending
- Marco Salvi, "An Excursion in Temporal Supersampling", GDC 2016: holding what's kept to the mean and spread of the pixels round it
- Anton Kaplanyan, Stephen Hill, Anjul Patney, Aaron Lefohn, "Filtering Distributions of Normals for Shading Antialiasing", HPG 2016
- Yusuke Tokuyoshi and Anton Kaplanyan, "Improved Geometric Specular Antialiasing", I3D 2019: the derivatives of the normal, and the limit of 0.18
- Michael Toksvig, "Mipmapping Normal Maps", 2004: how much shorter an average of normals is
- Marc Olano and Dan Baker, "LEAN Mapping", I3D 2010
- Ignacio Castaño, "Computing Alpha Mipmaps", 2010: scaling each mip's alpha so as much of it passes the cutoff
- Ben Golus, "Anti-aliased Alpha Test: The Esoteric Alpha To Coverage", 2017: alpha sharpened to a pixel's width
- AMD, FidelityFX Super Resolution 1 and 2, and Contrast Adaptive Sharpening: MIT-licensed upscaling and sharpening
- Brian Karis, "High Quality Temporal Supersampling", SIGGRAPH 2014: taking each pixel where it truly is, by a window
- Matt Pettineo, "Experimenting with Reconstruction Filters for MSAA Resolve", 2012: resolving samples as they'll be shown
- Jorge Jimenez and others, "Filmic SMAA: Sharp Morphological and Temporal Antialiasing", SIGGRAPH 2016: Catmull and Rom's curve in five reads, for what TAA kept
- Lei Yang, Shiqiu Liu and Marco Salvi, "A Survey of Temporal Antialiasing Techniques", Eurographics 2020: thin things, and what trusting what's kept costs
