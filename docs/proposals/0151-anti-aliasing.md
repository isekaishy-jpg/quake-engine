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
| SMAA | 0.01 to 0.02 ms | Smooth, and what's inside textures stays sharp |
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

## Sources

- [Proposal 0082: anti-aliasing and alpha](0082-antialiasing-and-alpha.md)
- [Proposal 0093: what increment 11 left](0093-what-increment-11-left.md)
