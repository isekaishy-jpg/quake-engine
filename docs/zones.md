# Zones

A zone is a named part of a map that things hang from. See [proposal 0116](proposals/0116-zones.md).

```text
entity town {
    type = zone
    title = "Vesper Deep"
    music = 4
    sound = ambience/drip1.wav
    brush { material = hidden cuboid = 0 0 -512 8192 8192 2048 }
}
entity market {
    type = zone
    title = "The Market"
    parent = town
    light = [1.1 1 0.9]
    brush { material = hidden cuboid = 3800 2200 0 4600 3000 600 }
}
```

The brushes are the zone's shape, as many as it takes. They're never seen or touched.

| Field | Means |
|---|---|
| `title` | What's shown on going in |
| `parent` | The zone it's part of |
| `music` | A track by number, or a file |
| `sound` | A sound that goes round and round in it |
| `light` | How much light reaches the eye: one number, or `[red green blue]` |
| `priority` | What wins where zones overlap. Without it, the smaller does |
| `light_set` | A [set of light](sky.md#sets-of-light) of its own, for a day |
| `light_blend` | How far in it takes for that to be all there is, 256 unless it says |

A sub zone is a zone inside another. Going in shows its title, and coming back out to the zone round it doesn't show that zone's again. What a sub zone doesn't set comes from its parent. Going from a zone to nowhere leaves its music playing, and light goes back to as it was, over a second and a half.

A script can ask with `engine.zone_at(origin)`, which gives a zone's name.
