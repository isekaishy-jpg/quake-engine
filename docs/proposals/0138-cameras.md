# Proposal 0138: cameras

Status: accepted and implemented, 2026-11-06, the [work list](../research/work-list.md), items 56 to 60.

## Research

The human asked for cameras as scripts, a way to set them up, a core third person camera with a spring boom, pushing in and snapping in, scene cameras, and said the intermission has less movement than the original. And that the engine should have all of it, with scripts setting cameras up.

The view was worked out in one place in `view.rs`, from the eye, with a sway when frozen, and there was no other camera.

What makes a third person camera usable is much the same across engines:

- A **boom** from a pivot near the character, back along the view. It's checked against the world with something wider than a line, or the near plane clips corners.
- **In at once, out slowly.** Coming in has to be immediate, as a camera easing into a wall shows what's behind it. Going back out eases, or it pumps at every pillar.
- **Lag on the pivot** takes the jitter out of what's followed, without losing the view's response.
- **Snapping in** when there's no room, as a camera in a character's head is worse than first person.
- **Blending** between cameras, the short way round.

Two things have to hold. Mouse look can't go through a script, or it adds lag and loses what's deterministic about it. And a camera is per player and per frame, so it's the client's: a server only needs to say what's wanted.

On the intermission: `view_idle` has the numbers of `V_AddIdle` at a scale of 1, and it did run. So it wasn't missing, as this list first said.

## Design

- **`Cameras`** in `engine_client` holds which camera is in use and what it remembers. Each frame it takes a `CameraSubject` and gives a `CameraShot`. First person is exactly the eye, with nothing added.
- **One line of text says what's wanted:** `first`, `third 96 8 0`, `fixed x y z yaw pitch idle`, `scene name`. So scripts, the UI, levels and the console all say it the same way, and it's easy to send.
- **`engine.camera(text, blend, target)`** in game scripts goes down the wire as `ServerMessage::Camera`. **`ui.camera(text, blend)`** is the same for a UI script, which is what chooses first person, in `level_started`, a new hook.
- **The intermission is a script's now:** `hold_at_camera` asks for a fixed camera that holds still, and the sway left `view.rs`.
- **`camera` entities** are scene cameras, staying put or following a spline, optionally looking at a point. `validate` checks their paths.
- **The player's model** is drawn when the camera's outside it, and the weapon when it isn't.
- **`camera`** at the console is there for trying things out.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a boom is checked by five lines, not a real sweep of a shape. They're wide enough at 10 units, and nothing needs more yet.
- **Acceptance criteria:** first person is unchanged. A boom follows, comes in for a wall, eases out and snaps in. Fixed and scene cameras work, from a script and from a level, and blends take the short way.
- **Future extension:** none planned.

## Review

Third person was looked at in the proving ground, in the open and backed against a wall, with Quake's player model standing in. 8 tests hold what cameras do. A camera setup tab is item 61.

The end of a level was measured for real: a level was finished in the game, and 14 pictures taken over 6 seconds. The view moved 7 pixels across and 7 down at 1280 by 720, in a full cycle of pitch and two of yaw. That's what `V_AddIdle` at a scale of 1 gives, plus or minus 0.3 degrees, so the movement matches the original's code. All of the source was then searched, in `WinQuake`, which GLQuake shares, in `QW` and in the QuakeC: `V_CalcIntermissionRefdef` and `V_AddIdle` are all that moves that view, with the same numbers in each. The rerelease has no source to read. The human then watched videos of the game, which show that view still, and there are many versions of Quake. So the end of a level is **static** now, with an `idle` of 0, by the human's choice. `idle` stays for any camera that wants it.
