# Proposal 0149: models that go evenly

Status: accepted and implemented, 2026-11-07. It comes from the human's play session, for [proposal 0142](0142-the-fixtures-models-on-the-mesh-path.md).

## Why

The human said that enemy models are choppy, and always have been.

A tick is a 72nd of a second, but a monster only moves when it thinks, ten times a second. Position was blended across one tick, so a monster hopped and then stood still. Measured frame by frame in E1M1, grunts moved on 18% of frames, and stood still for a median of 17 frames between hops. Their bodies animated smoothly the whole time, which is what made it look wrong.

Online it was worse: a new pose was blended across one tick there too, not a tenth of a second.

## What changes

`ModelGlide` sits between what the client says is there and what's drawn. What moves after standing still is shown going there over a tenth of a second, from where it's shown. What moves every tick is left alone, and once it's going there's no lag. A teleport goes straight there. Turning goes the same way, the short way round.

It keeps a model's frames too, by time, so poses blend the same way online and off. That replaced what the client kept for that.

It's only for what's on screen. Collision, bots and everything else go by where things are.

## What came of it

- **Tried in the game:** in E1M1 grunts now move on 94% of frames, and what's left is mostly them stopping. The biggest move in a frame went from up to 5.8 units to up to 1.4.
- **A monster shows up to a tenth of a second behind where it is.** That's the cost, and at a run it's a few units. Shots go by where it really is.
- **A slow machine isn't caught out.** Standing still takes at least two frames, or long frames would look like it, and what's moving would be left behind.
- **Something starting from rest takes a tenth of a second to catch up,** then it's exact.
- **Needs a person:** whether it now looks right in motion. Numbers can say it moves evenly, but not that it looks good.
