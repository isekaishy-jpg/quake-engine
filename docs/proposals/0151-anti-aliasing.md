# Proposal 0151: anti-aliasing

Status: accepted for item 172, 2026-11-07. Items 173 to 177 of the [work list](../research/work-list.md) are added to this as each is reached.

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

## Sources

- [Proposal 0082: anti-aliasing and alpha](0082-antialiasing-and-alpha.md)
- [Proposal 0093: what increment 11 left](0093-what-increment-11-left.md)
- Brian Karis, "High Quality Temporal Supersampling", SIGGRAPH 2014: moving the view round a pattern, and squeezing bright light before blending
- Marco Salvi, "An Excursion in Temporal Supersampling", GDC 2016: holding what's kept to the mean and spread of the pixels round it
