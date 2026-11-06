# Doors and lifts

A door is an entity with brushes of its own and `type = "func_door"`. A lift is the same with `func_plat`.

## What a door does

It opens when something alive comes within 60 units, and holds open while it's there. After `wait` seconds with nobody near it closes. If something's in the way as it closes, it hurts them and opens again. Doors that touch work as one.

| Field | What it does | If it's left out |
|---|---|---|
| `angle` | Which way it moves. -1 is up and -2 is down | 0, along x |
| `speed` | Units a second | 100 |
| `wait` | Seconds before it closes. -1 stays open | 3 |
| `lip` | How much of it stays showing when open | 8 |
| `sounds` | 1 stone, 2 machine, 3 rough stone, 4 chains | Silent |
| `health` | Has to be shot open | Doesn't |
| `message` | Said when someone can't open it | Nothing |
| `spawnflags` | 1 starts open, 4 won't link, 8 needs the gold key, 16 the silver key, 32 toggles | None |

## Opening something from somewhere else

Give it a name, and aim something at that name:

```text
entity gate {
    type = "func_door"
    brush { ... }
}

entity {
    type = "func_button"
    target = "gate"
    brush { ... }
}
```

Now the door waits for the button, and doesn't open by itself.

**A name on its own is only a label.** A door called `gate` that nothing aims at opens when someone comes near, like any other. It's only when something's `target` or `killtarget` names it that it waits to be fired.

## What a lift does

Place a lift at the top of its travel. It starts at the bottom, rises when someone steps on it, and comes back down after 3 seconds at the top.

| Field | What it does | If it's left out |
|---|---|---|
| `height` | How far it travels | Its own height, less 8 |
| `speed` | Units a second | 150 |

If something aims at a lift, it starts at the top and waits to be fired before it comes down.

Things that walk use lifts too. See [proposal 0122](proposals/0122-paths-that-leap.md).
