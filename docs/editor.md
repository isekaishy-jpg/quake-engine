# Making a level

The editor opens on a floor plan. You make a level by clicking squares. Nothing needs dragging, holding down or doing quickly.

```bash
cargo run --release -p engine_editor
```

## Your first level

1. **Room** is already chosen. Click squares to make a room. Click a square again to take it away.
2. Choose **Start**, and click where the player begins.
3. Choose **Light**, and click a floor.
4. Press **Play**.

The panel on the right says what's still needed. Play can't be pressed until the level can be played. It saves your level, names it if it has no name, lights it, and opens the game on it. The game stays open until you close it.

## The tools

| Tool | What it does |
|---|---|
| Room | Makes floor. Walls and a ceiling follow |
| Water | Makes a pool you can swim in |
| Start | Where the player begins. There's only one |
| Light | A light on the ceiling. Dark rooms need one |
| Door | Goes in a gap one square wide. It opens when someone walks up to it |
| Health, Ammo | Things to pick up |
| Enemy | Something to fight |
| Fire | A flame that gives light |
| Rub out | Takes a square away, and what's on it |

**What it's made of** changes how the whole level looks at once.

**Undo** and **Redo** take back any mistake, as many times as you like.

**See it** shows the level in 3D, with four buttons to walk round. **Back to plan** returns.

## Making it easier to use

- **Bigger** and **Smaller** change the size of everything, up to three times.
- **Plain colours** gives white on black.
- Everything works with only a mouse, and everything works with only a keyboard.

| Key | What it does |
|---|---|
| Arrows | Move round the plan |
| Space | Use the tool here |
| 1 to 9, 0 | Choose a tool |
| U | Undo |
| P | Play |
| Tab | Go through the buttons. Space presses the one that's lit |
| Esc | Leave the buttons and go back to the plan |

## Finding the game

Play looks for the game next to the editor, and for the game's data in the usual Steam folders. If yours is somewhere else, set `ENGINE_GAME` to the game and `ENGINE_BASEDIR` to the data.

## Going further

**Advanced** opens the full editor, and **Easy** comes back. A level made another way opens in Advanced.

A plan makes ordinary brushes and entities, so Advanced can see all of it. Painting the plan again remakes what the plan made, and leaves what you built by hand alone.

## For agents

The plan is ordinary operations, so anything here can be done without the window.

| Operation | Arguments |
|---|---|
| `plan_read` | None. Answers with the plan and what's missing |
| `plan_paint` | `cells`, a list of `[x, y]`, and `kind`: `floor`, `water` or `none` |
| `plan_thing` | `cells`, and `thing`: `start`, `light`, `door`, `health`, `ammo`, `enemy`, `fire` |
| `plan_look` | `look`, the start of a material set's name |
| `play` | None. Opens the game on the saved map, and leaves it open |
| `wear` | Optional `amount` from 0 to 1, and `seed`. Makes a level look used, from its real shape, and replaces what it made before |

`playtest` is different from `play`. It runs the game for some seconds and reports whether it stayed up, so it's for checking and not for playing.
