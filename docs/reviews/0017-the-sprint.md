# Review 0017: the sprint through the list

Scope: all 28 items in [the sprint](../research/sprint.md), with proposals 0102 to 0129, and the final rounds.

## Where it stands

Every item went through the workflow. `cargo xtask check` passes, and so do all 73 ignored tests, on a fresh bake of every level with the real game data. Nothing has been pushed.

## Mistakes made along the way

These are worth keeping, as each came from a habit and not from bad luck.

| What happened | What it cost | What changed |
|---|---|---|
| Item 11 was committed after a failed check, in a chained command | An amended commit | Commits are gated on the check's exit code |
| Item 18 was committed and marked done before its checks had run | Play, doors, Bigger, walls and the Advanced camera all shipped broken in `26c7282`, fixed in `dc26f87` | Nothing's done until it's been tried in the real program |
| A test called `a_door_blocks_the_way_through_and_opens` never opened a door | Hid a door that couldn't open | A test's name has to be what it does |
| The ignored level tests weren't run for nine items | A real seam in the light from item 11 sat unseen until item 20 | They're run by hand after anything that touches a bake or a level |
| The proposal for item 18 said Bigger and Advanced had been checked, from clicks that had landed on other buttons | A false claim, for a short while, which going back caught | Look at what a click did, not just that it was sent |

## Faults found by the final rounds

| # | Fault | Fix |
|---|---|---|
| 1 | An error in the UI script was dropped each frame, so a broken status bar just vanished | It's said, once |
| 2 | A block after a list on one line of a map was read as another value | A list ends where a block starts |
| 3 | No real level had a zone, so names on going in and zone sound had never run in one | Both levels have them |
| 4 | What's behind a wall used a trace the size of a player. Eyes under a low ceiling muffled every sound, and anything against a wall counted as behind it | A thin line through the map |
| 5 | The view clock held the wait for the first snapshot against the network, and took a new level's clock for a stale one | Both handled |
| 6 | A wear that failed left a level without its old wear | It's put back |
| 7 | Quake's `config.cfg` printed 31 unknown commands at every start | They're accepted |

Found while working on the items, and bigger than the items were:

- **Faces disagreed on light at joins,** from item 11: `copy_sample` copied layers by place in a list that each face now ranks for itself.
- **Naming a door or lift broke it,** from the start of the map format. Three doors could never open, and named lifts sat at the top for good. Ember Keep's two lifts have never worked in play until now.
- **A player touched what's at the middle of the world** when a level asked everything to touch again.
- **The nearest place on a walkable grid could be on the level's roof.**
- **Hits fell to 92%** with a longer view delay, until the server's rewind allowed for it.

## Performance

At 2560 by 1400 on a GTX 1070, in Vesper Deep:

| What | Result |
|---|---|
| Flying through at speed | About 220 frames a second, worst frame under 6 ms, no hitches |
| Frame time | 4.33 ms, of which layers and hex tiling are 0.35 ms |
| 800 decals | 0.72 ms as a draw each, and nothing that can be measured now |
| Loading Wide World | 11.2 s to 4.5 s |
| Loading Vesper Deep | 2.0 s to 1.55 s |
| 8 kHz mouse | 8,000 host steps a second down to one for each frame |

No big item is left. What's left of a load is spread out: collision, reading the map, and uploading it.

## How far the review went

The final rounds went by what can go wrong, and not by reading all 18,500 new lines one at a time. New code from items 18 to 28 was reread. Items 1 to 17 were held to their tests and every ignored test. `engine_host`, `engine_worker` and `engine_cache` were reviewed by kind of fault in [proposal 0125](../proposals/0125-host-worker-cache-review.md).

## Needs a person

| What | Why |
|---|---|
| Whether the floor plan works for people with disabilities | Only they can say |
| Ember Keep's and Vesper Deep's lifts | They've never worked in play until now, and nobody's ridden them |
| Doors with sound | Set, and not heard by anyone |
| How the new mix sounds | Reverb, muffling and ducking are numbers until someone listens |
| Whether a rocket slowing by a fifth feels right | `cl_shot_correction` changes it |
| A real 8 kHz mouse | Needs one |
| Real people on a real network | `net_fake_burst` and the others stand in until then |
| A real person's model for the city | Has a spec in [proposal 0121](../proposals/0121-level-faults.md) |
| The loading spinner | It's only up for a moment, and no capture caught it |

## Not done

These were never on the list, and they came up along the way. They're real work, so they're on [the next list](../research/sprint.md#next) and not left as notes.
