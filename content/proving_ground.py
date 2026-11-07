"""Writes the steps that build the proving ground, the small level everyday testing uses.

It's a real level, and each area is there to test something, with nothing that tests
nothing (item 124 of docs/research/work-list.md). It grows as features land.

    rm content/maps/proving_ground.*
    python content/proving_ground.py > steps.json
    engine-tool content content/maps/proving_ground.emap --batch steps.json
"""

import json

STONE = "qg01_ashen_abbey_wall_blocks"
FLOOR = "qg01_ashen_abbey_floor_flags"
TRIM = "qg01_ashen_abbey_trim_carved"
ROCK = "qg01_ashen_abbey_rock"
CEILING = "qg01_ashen_abbey_ceiling_coffer"
WOOD = "qg01_ashen_abbey_wood_planks"
WATER = "canal_water"

steps = []


def op(tool, **args):
    steps.append({"op": tool, "args": args})


def box(low, high, material, contents=None):
    args = {"cuboid": [*low, *high], "material": material}
    if contents:
        args["contents"] = contents
    op("add_brush", **args)


def shell(low, high, wall, floor, roof, thick=16, gaps=()):
    """A closed room. Each gap is a side (x-, x+, y-, y+) and a span along it, left open."""
    x0, y0, z0 = low
    x1, y1, z1 = high
    t = thick
    if floor:
        box((x0 - t, y0 - t, z0 - t), (x1 + t, y1 + t, z0), floor)
    if roof:
        box((x0 - t, y0 - t, z1), (x1 + t, y1 + t, z1 + t), roof)
    sides = {
        "x-": ((x0 - t, y0, z0), (x0, y1, z1), 1),
        "x+": ((x1, y0, z0), (x1 + t, y1, z1), 1),
        "y-": ((x0 - t, y0 - t, z0), (x1 + t, y0, z1), 0),
        "y+": ((x0 - t, y1, z0), (x1 + t, y1 + t, z1), 0),
    }
    for side, (a, b, axis) in sides.items():
        cuts = sorted(g[1:] for g in gaps if g[0] == side)
        start = a[axis]
        for low_cut, high_cut, top in cuts:
            piece_a, piece_b = list(a), list(b)
            piece_a[axis], piece_b[axis] = start, low_cut
            box(tuple(piece_a), tuple(piece_b), wall)
            over_a, over_b = list(a), list(b)
            over_a[axis], over_b[axis] = low_cut, high_cut
            over_a[2] = top
            box(tuple(over_a), tuple(over_b), wall)
            start = high_cut
        piece_a, piece_b = list(a), list(b)
        piece_a[axis] = start
        box(tuple(piece_a), tuple(piece_b), wall)


def ring(low, high, hole, material):
    """A slab with a hole through it."""
    x0, y0, z0 = low
    x1, y1, z1 = high
    (hx0, hy0), (hx1, hy1) = hole
    box((x0, y0, z0), (hx0, y1, z1), material)
    box((hx1, y0, z0), (x1, y1, z1), material)
    box((hx0, y0, z0), (hx1, hy0, z1), material)
    box((hx0, hy1, z0), (hx1, y1, z1), material)


op("add_entity", type="worldspawn", name="world", fields={
    "ambient": 14,
    "light_spacing": 32,
    "bounce": 0.6,
    "sun_intensity": 170,
    "sun_crisp": True,
})
op("set_block", kind="sky", fields={
    "zenith": [0.22, 0.34, 0.52],
    "horizon": [0.72, 0.68, 0.6],
    "sun_direction": [0.5, 0.35, 0.6],
    "sun_color": [1.0, 0.92, 0.78],
})
op("set_block", kind="fog", fields={"color": [0.45, 0.5, 0.55], "density": 0.0001, "height": 0, "falloff": 0.002})
op("set_block", kind="water", fields={"color": [0.03, 0.1, 0.12], "absorption": 0.008, "caustics": 0.4})

# The yard: open to the sky, for sun, sky and later for terrain coming over old slabs.
box((-1024, -768, -32), (0, 768, 0), FLOOR)
box((-1040, -784, -32), (-1024, 784, 512), ROCK)
box((-1040, -784, -32), (16, -768, 512), ROCK)
box((-1040, 768, -32), (16, 784, 512), ROCK)
box((0, -768, 0), (16, -272, 512), ROCK)
box((0, 272, 0), (16, 768, 512), ROCK)
box((0, -272, 240), (16, 272, 512), ROCK)
box((-1040, -784, 512), (16, 784, 528), "sky", "sky")

# The hall: a building with a door from the yard, for doors, baked light and shadows.
SHAFT = ((192, -64), (320, 64))
shell((0, -256, 0), (640, 256, 224), STONE, None, CEILING, gaps=[("x-", -48, 48, 112)])
ring((-16, -272, -16), (656, 272, 0), SHAFT, FLOOR)
op("place_prefab", prefab="door", at=[-16, 0, 0], params={"width": 96, "height": 112, "material": WOOD, "speed": 200})

# The lower level, down a lift: a real journey, and a place for water.
shell((192, -192, -320), (640, 192, -128), ROCK, ROCK, None)
ring((176, -208, -128), (656, 208, -112), SHAFT, ROCK)
(sx0, sy0), (sx1, sy1) = SHAFT
box((sx0 - 16, sy0 - 16, -112), (sx0, sy1 + 16, -16), ROCK)
box((sx1, sy0 - 16, -112), (sx1 + 16, sy1 + 16, -16), ROCK)
box((sx0, sy0 - 16, -112), (sx1, sy0, -16), ROCK)
box((sx0, sy1, -112), (sx1, sy1 + 16, -16), ROCK)
op("place_prefab", prefab="lift", at=[sx0, sy0, -320], params={"size": 128, "height": 320, "material": WOOD})
box((352, -192, -320), (640, 192, -272), WATER, "water")

op("add_entity", type="info_player_start", origin=[-400, 0, 24], fields={"angle": 0})
op("add_entity", type="light", origin=[320, 0, 180], fields={"light": 300})
op("add_entity", type="light", origin=[420, 0, -160], fields={"light": 260})
op("add_entity", type="effect", origin=[-700, 300, 8], fields={"effect": "campfire"})

# Scene cameras: one that stays and looks, and one that flies along a line.
op("add_entity", type="camera", name="over_the_yard", origin=[-950, -650, 300],
   fields={"look_at": "-400 0 40", "idle": 1})
op("add_spline", name="fly_in", points=[[-950, 600, 320], [-500, 300, 200], [-200, 0, 120], [-60, 0, 60]])
op("add_entity", type="camera", name="fly_to_the_door", origin=[-950, 600, 320],
   fields={"path": "fly_in", "seconds": 6, "look_at": "0 0 60"})
op("place_mesh", source="barrel", origin=[-600, 260, 0])
print(json.dumps(steps, indent=1))
