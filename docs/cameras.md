# Cameras

The engine does the work of a camera, every drawn frame. What's wanted is said in a line of text, and that line is the same everywhere. See [proposal 0138](proposals/0138-cameras.md).

| Camera | Means |
|---|---|
| `first` | From the player's own eyes |
| `third` | Behind the player, on a boom |
| `third 96 8 16` | A boom 96 long, 8 over the eye and 16 to the side |
| `fixed x y z yaw pitch idle` | In one place. `idle` is how much it sways, with 1 what `V_AddIdle` does |
| `scene name` | A `camera` in the level, by its name |

## Saying which

- **In a game's scripts:** `engine.camera("third", 0.5)` changes for everyone, over half a second. A third argument is one player's id.
- **In the UI script:** `ui.camera("third", 0.5)`. `level_started` is called as each level begins, which is where a game chooses what it plays from.
- **At the console:** `camera third`.

Mouse look never goes through any of this. A script says what the camera is, and the engine runs it.

## A boom

It follows the player, comes in at once for a wall so that nothing's seen through it, and eases back out. With no room at all it snaps in to the player's own eyes, and they see their weapon. The player's own model is drawn whenever the camera's outside it.

## A camera in a level

```text
entity over_the_yard {
    type = camera
    origin = [-950 -650 300]
    look_at = "-400 0 40"
    idle = 1
}

spline fly_in {
    point = [-950 600 320]
    point = [-200 0 120]
}

entity fly_to_the_door {
    type = camera
    path = "fly_in"
    seconds = 6
    look_at = "0 0 60"
}
```

| Field | Means |
|---|---|
| `origin` | Where it is |
| `yaw`, `pitch` | Which way it looks |
| `look_at` | What it keeps looking at, in place of those |
| `idle` | How much it sways |
| `path` | A spline it moves along |
| `seconds` | How long that takes, after which it stays at the end |

Show one with `camera scene fly_to_the_door`. `validate` says if its path isn't there.
