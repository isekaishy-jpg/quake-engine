# Proposal 0123: doors and what a name means

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

The open question was how door logic should be revamped, left for after the port played through. The port matches `doors.qc`, with a note that it might feel better with sound. The human's design was wanted, and the sprint makes the call and records it here.

The rules in `doors.qc` hold up. A door opens when something alive comes near, holds open while it's there, turns back when blocked, links to doors touching it, and asks for a key or a shot where it's told to. None of that needed changing, and Quake's own levels depend on it.

Going through how our levels really use doors found the real fault, and it wasn't in the door at all. **In the map format, any entity's name became its `targetname`.** In `doors.qc` and `plats.qc`, having a `targetname` means waiting to be fired. So naming something, which an author does to keep track of it, quietly changed what it did:

| In the levels | What it did | What it should do |
|---|---|---|
| `hall_door`, `first_door` and `rolling` | Never opened: nothing fired them | Open when someone comes near |
| `forge_lift` and `crypt_lift` in Ember Keep | Sat at the top for good, waiting to be fired | Wait at the bottom and rise |
| `lift_1` and others in the demonstration levels | The same | The same |
| `main_gate` in Ember Keep | Had two `targetname` fields, its name first, so the button aiming at `gate_up` may not have found it | Answer to `gate_up` |

Item 18 hit the same trap with the plan's doors, and worked round it. This is its cause. It went unseen because the level tests walk through doors and start a new leg at each lift, which is right for a walkable grid and says nothing of whether they work.

The other finding: no door in any of our levels had `sounds` set, and in `doors.qc` that means silence.

## Design

- **A name is a label, unless something aims at it.** An entity's name becomes a `targetname` only if some entity's `target` or `killtarget` has that name, and never over a `targetname` of its own. The name is kept in a `name` field either way.
- **Doors in our levels make a sound.** All 13 have `sounds = 1`, as does a door on a floor plan. Silence stays the default, as Quake's own levels set what they want.
- **A plan's door is quicker,** at a speed of 200 and not 100, so it isn't in the way.
- **[A guide](../doors.md)** says what doors and lifts do, and which fields change it.
- Door behaviour itself stays as `doors.qc` has it.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** nothing can fire an entity by a name only a script knows. A level's script that wants that sets `targetname` itself, as `main_gate` does. No level's script does today.
- **Acceptance criteria:** a named door opens for what comes near it, with a walker going through in a real server, named lifts start at the bottom, and doors that a button fires still wait for it.
- **Future extension:** for the human: whether doors should do something `doors.qc` doesn't.

## Review

`it_goes_through_a_door_with_a_name` fails with every name a target again. In the real levels, `forge_lift` and `crypt_lift` start at the bottom, and the Ember Keep's main gate answers to `gate_up` and stays shut until then.

No person has walked those levels' lifts since this. They're worth trying, as they've never worked in play until now.
