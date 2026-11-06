"""Vesper Deep: a city under a ruined town (proposal 0097).

You come in by the broken gate of Vesper, walk its dead street to the square and
on to the roofless minster, and go down under the altar. A long shaft drops you
on the High Terrace, with the whole lamplit city below. A stair leads down to the
Market, a bridge across the canal to the Foundry, whose wheel opens the sluice on
the west bridge, and the walkways climb round the Fall in the east. The Pumpworks,
past the sluice, are the way on.

It's on a grid of tiles, so its light and paths stream.
"""
from kit import Level

LIME = 'qg03_elder_crypt_wall_blocks'
LIME_DAMP = 'qg03_elder_crypt_wall_damp'
LIME_BRICK = 'qg03_elder_crypt_wall_brick'
PAVE = 'qg03_elder_crypt_floor_flags'
TILE = 'qg03_elder_crypt_floor_tiles'
ROCK = 'qg03_elder_crypt_rock'
CARVED = 'qg03_elder_crypt_trim_carved'
BRONZE = 'qg03_elder_crypt_trim_metal'
GREY = 'uc03_apothecary_wall_blocks'
GREY_FLOOR = 'uc03_apothecary_floor_flags'
COPPER = 'uc03_apothecary_metal_plate'
COPPER_TRIM = 'uc03_apothecary_trim_metal'
TIMBER = 'uc03_apothecary_wood_planks'
GREEN = 'uc01_blight_sewers_metal_plate'
GREEN_TRIM = 'uc01_blight_sewers_trim_metal'
MOSSY = 'uc01_blight_sewers_wall_blocks'
FLAGS = 'uc01_blight_sewers_floor_flags'
DOOR = 'uc01_blight_sewers_door'
WINDOW = 'uc01_blight_sewers_grille_opaque'
LAVA = 'qg02_iron_cathedral_liquid'
RUST = 'qg02_iron_cathedral_metal_plate'
BASALT = 'qg02_iron_cathedral_wall_blocks'
BASALT_FLOOR = 'qg02_iron_cathedral_floor_flags'
ROOF = 'qg01_ashen_abbey_wood_planks'

GROUND = 1536
QUAY = 64
TERRACE = 320

level = Level(
    'vesper_deep',
    (0, 0, -320, 8192, 6144, GROUND),
    ROCK,
    {'ambient': 44, 'light_spacing': 32, 'bounce': 1, 'cell_size': 1024, 'sun_intensity': 170,
     'sun_crisp': True, 'wake_reach': 3072},
    blocks='''sky {
    zenith = 0.3 0.42 0.62
    horizon = 0.86 0.78 0.66
    ground = 0.3 0.28 0.26
    sun_direction = 0.45 -0.55 0.62
    sun_color = 1 0.9 0.74
    clouds { color = 0.9 0.86 0.82 coverage = 0.5 scale = 1 }
}
fog { color = 0.2 0.27 0.3 density = 0.00012 height = 1100 falloff = 0.0006 }
tile_grid { size = 2048 }''')


def lantern(x, y, z, intensity=300, color=(1, 0.74, 0.46)):
    level.prefab('lantern_post', (x, y, z), intensity=intensity, color=list(color), material=GREEN_TRIM)


def glow(x, y, z, intensity, color):
    level.entity('light', (x, y, z), intensity=intensity, color=list(color))


def elevator(name, x0, y0, x1, y1, top, bottom, wait=4, speed=200, material=COPPER):
    """A platform going up and down for ever between two stops."""
    level.moving('func_train', [(x0, y0, top - 8, x1, y1, top)], material, name=name,
                 target=f'{name}_top', speed=speed)
    level.entity('path_corner', (x0, y0, top - 8), targetname=f'{name}_top', target=f'{name}_bottom', wait=wait)
    level.entity('path_corner', (x0, y0, bottom - 8), targetname=f'{name}_bottom', target=f'{name}_top', wait=wait)


# --- The ruined quarter ---------------------------------------------------
level.carve(0, 0, GROUND, 8192, 6144, GROUND + 1200, floor=FLAGS, wall=LIME)
# Nobody leaves: the town wall is broken, but what's past it isn't for walking.
for x0, y0, x1, y1 in ((2240, 96, 5952, 112), (2240, 4288, 5952, 4304), (2240, 96, 2256, 4304), (5936, 96, 5952, 4304)):
    level.box('hidden', x0, y0, GROUND, x1, y1, GROUND + 640)

# The town wall and its gate.
level.prefab('arch', (4096, 256, GROUND), width=224, spring=176, length=704, height=384, thickness=64,
             material=LIME, stone=LIME_BRICK, segments=10)
for x, length, seed in ((2432, 1312, 2), (4448, 1312, 3)):
    level.prefab('broken_wall', (x, 224, GROUND), length=length, height=336, thickness=64, material=LIME,
                 seed=seed, ragged=210)
for x in (3744, 4448):
    level.prefab('tower', (x, 256, GROUND), width=160, height=448, material=LIME, trim=LIME_BRICK, cap=ROOF)

# The street north from the gate, with what's left of its houses.
street = [(3392, 480, 'east', 1), (3360, 800, 'east', 2), (3408, 1120, 'east', 3),
          (4480, 480, 'west', 4), (4448, 800, 'west', 5), (4496, 1120, 'west', 6),
          (2880, 640, 'south', 7), (5024, 640, 'south', 8), (2848, 1060, 'east', 9), (5088, 1040, 'north', 10),
          (2880, 1900, 'east', 11), (5040, 1880, 'west', 12), (2880, 2280, 'north', 13), (5040, 2300, 'south', 14)]
for x, y, door, seed in street:
    level.prefab('house_shell', (x, y, GROUND), width=320 + seed % 3 * 32, depth=256, height=240 - seed % 4 * 24,
                 material=LIME_DAMP if seed % 2 else LIME_BRICK, door=door, seed=seed, beams=TIMBER)
for x, y, size, seed in ((4016, 620, 56, 21), (4212, 900, 48, 22), (3990, 1200, 60, 23), (3560, 1560, 44, 24),
                         (4660, 1500, 52, 25), (4400, 2100, 56, 26), (3700, 2120, 46, 27)):
    level.prefab('rubble', (x, y, GROUND), radius=size * 3, count=11, size=size, material=LIME, seed=seed)

# The square, and its dry fountain.
level.box(PAVE, 3456, 1408, GROUND, 4736, 2112, GROUND + 4)
level.prefab('well', (4096, 1760, GROUND + 4), radius=176, height=40, sides=16, material=LIME)
level.prefab('column', (4096, 1760, GROUND + 4), radius=20, height=176, sides=8, material=LIME, trim=BRONZE)
level.prefab('broken_wall', (3488, 2064, GROUND), length=320, height=200, thickness=32, material=LIME_BRICK, seed=31)
level.prefab('broken_wall', (4384, 1440, GROUND), length=320, height=160, thickness=32, material=LIME_BRICK, seed=32)

# The Minster: a roofless nave, its west front still standing.
level.prefab('arch', (4096, 2240, GROUND), width=256, spring=256, length=896, height=640, thickness=64,
             material=LIME, stone=CARVED, segments=12)
for x, seed in ((3648, 41), (4512, 42)):
    level.box(LIME, x - 16, 2240, GROUND, x + 16, 3456, GROUND + 96)
    for y in range(2272, 3456, 192):
        level.prefab('column', (x + (64 if x < 4096 else -64), y + 96, GROUND), radius=30,
                     height=300 + (y * 7 + seed) % 220, sides=8, material=LIME, trim=BRONZE)
level.prefab('broken_wall', (3632, 3456, GROUND), length=896, height=560, thickness=48, material=LIME, seed=43, holes=2)
level.prefab('broken_wall', (3648, 2272, GROUND), turn=90, length=1184, height=480, thickness=32,
             material=LIME, seed=44, holes=4)
level.prefab('broken_wall', (4544, 2272, GROUND), turn=90, length=1184, height=420, thickness=32,
             material=LIME, seed=45, holes=4)
level.prefab('rubble', (3900, 2700, GROUND), radius=200, count=14, size=60, material=LIME, seed=46)
level.prefab('rubble', (4320, 3050, GROUND), radius=170, count=12, size=52, material=LIME, seed=47)
# The altar, and the stair down behind it.
level.box(LIME, 3968, 3040, GROUND, 4224, 3136, GROUND + 56)
level.box(BRONZE, 3952, 3024, GROUND, 4240, 3152, GROUND + 8)
level.entity('info_player_start', (4096, 160, GROUND + 24), angle=90)

# --- Down under the altar -------------------------------------------------
level.carve(3968, 3168, GROUND - 160, 4224, 3456, GROUND, floor=PAVE, wall=LIME_DAMP, ceiling=LIME)
for step in range(9):
    level.box(PAVE, 3968, 3168 + step * 32, GROUND - 160, 4224, 3200 + step * 32, GROUND - step * 16)
level.carve(3840, 3456, GROUND - 160, 4352, 3648, GROUND - 16, floor=PAVE, wall=LIME_DAMP, ceiling=LIME)
glow(4096, 3550, GROUND - 60, 240, (1, 0.76, 0.5))

# --- The Long Shaft -------------------------------------------------------
SHAFT_TOP = GROUND - 160
level.carve(3904, 3648, TERRACE, 4288, 4032, GROUND - 16, floor=PAVE, wall=LIME, ceiling=LIME)
elevator('long_lift', 3968, 3712, 4224, 3968, SHAFT_TOP, TERRACE, wait=5, speed=240)
glow(4096, 3840, SHAFT_TOP + 90, 220, (1, 0.8, 0.55))
for z in range(TERRACE + 128, 1000, 192):
    glow(3930, 3840, z, 150, (1, 0.74, 0.46))
    glow(4262, 3840, z, 150, (1, 0.74, 0.46))

# The stair round it, for those who'd rather walk.
level.carve(4352, 3456, TERRACE, 4736, 3840, GROUND - 16, floor=PAVE, wall=LIME, ceiling=LIME)
level.carve(4288, 3584, GROUND - 160, 4352, 3648, GROUND - 16, floor=PAVE, wall=LIME, ceiling=LIME)
level.prefab('spiral_stairs', (4544, 3648, TERRACE), radius=176, rise=SHAFT_TOP - TERRACE, core=40,
             material=PAVE, post=LIME, start=180, clockwise=True)

# --- The cavern -----------------------------------------------------------
level.carve(640, 640, -160, 7552, 5504, 1024, floor=ROCK, wall=ROCK, ceiling=ROCK)
level.carve(2304, 1216, 1024, 5760, 4096, 1200, floor=ROCK, wall=ROCK, ceiling=ROCK)
level.carve(5760, 2432, 1024, 7360, 4864, 1136, floor=ROCK, wall=ROCK, ceiling=ROCK)
level.carve(832, 1024, 1024, 2304, 3008, 1120, floor=ROCK, wall=ROCK, ceiling=ROCK)

# The shaft and stair stand in the cavern as a great stone pier, with windows.
for x0, y0, x1, y1 in ((3872, 3648, 3904, 4064), (4288, 3840, 4320, 4064), (3872, 4032, 4320, 4064)):
    level.box(LIME, x0, y0, TERRACE, x1, y1, 1024)
for x0, y0, x1, y1 in ((4320, 3424, 4768, 3456), (4736, 3456, 4768, 3840), (4320, 3840, 4768, 3872)):
    level.box(LIME, x0, y0, TERRACE, x1, y1, 1024)
for z in range(TERRACE + 256, 1024, 192):
    level.box(LIME, 3872, 3616, z + 96, 4320, 3648, z + 192)
    level.box(LIME, 3872, 3616, z - 96, 3968, 3648, z + 96)
    level.box(LIME, 4224, 3616, z - 96, 4320, 3648, z + 96)
level.box(LIME, 3872, 3616, TERRACE + 160, 4320, 3648, TERRACE + 256 - 96)
# The arch you step out of.
level.box(LIME, 3872, 3616, TERRACE, 3968, 3648, TERRACE + 160)
level.box(LIME, 4224, 3616, TERRACE, 4320, 3648, TERRACE + 160)
level.box(CARVED, 3856, 3600, TERRACE + 160, 4336, 3648, TERRACE + 176)

# The land. What isn't here is the canal.
CANAL = [(5600, 2400, 7552, 2800), (5600, 1000, 6000, 2400), (2000, 1000, 6000, 1400),
         (2000, 1400, 2400, 3200), (640, 2800, 2000, 3200)]
LAND = [(640, 640, 7552, 1000), (6000, 1000, 7552, 2400), (640, 1000, 2000, 2800), (2400, 1400, 5600, 3200),
        (5600, 2800, 7552, 3200), (2400, 3200, 7552, 5504), (640, 3200, 2400, 5504)]
for x0, y0, x1, y1 in LAND:
    level.box(FLAGS, x0, y0, -160, x1, y1, QUAY)
for x0, y0, x1, y1 in CANAL:
    level.liquid('water', 'water', x0, y0, -160, x1, y1, -24)
    level.box(MOSSY, x0, y0, -160, x1, y0 + 8, QUAY)
    level.box(MOSSY, x0, y1 - 8, -160, x1, y1, QUAY)

# The High Terrace.
level.box(PAVE, 3000, 3392, QUAY, 5408, 4416, TERRACE)
level.box(LIME_BRICK, 2984, 3376, QUAY, 5424, 3392, TERRACE + 8)
for x in range(3000, 5408, 96):
    if not 3880 <= x <= 4300:
        level.box(LIME, x, 3392, TERRACE, x + 16, 3408, TERRACE + 40)
level.box(BRONZE, 3000, 3392, TERRACE + 40, 3880, 3408, TERRACE + 46)
level.box(BRONZE, 4304, 3392, TERRACE + 40, 5408, 3408, TERRACE + 46)
for x in (3300, 3760, 4432, 4900, 5300):
    lantern(x, 3440, TERRACE, 340)
lantern(3180, 4300, TERRACE, 300)
lantern(5200, 4300, TERRACE, 300)

# The Grand Stair down to the city.
for step in range(16):
    level.box(LIME, 3904, 3392 - (step + 1) * 32, QUAY, 4288, 3392 - step * 32, TERRACE - step * 16)
level.box(LIME_BRICK, 3872, 2880, QUAY, 3904, 3392, TERRACE)
level.box(LIME_BRICK, 4288, 2880, QUAY, 4320, 3392, TERRACE)

# --- The Central Ward -----------------------------------------------------
houses = [(2560, 1520, 256, 224, 240), (2880, 1520, 288, 224, 288), (3232, 1520, 224, 224, 224),
          (4576, 1520, 256, 224, 256), (4896, 1520, 288, 224, 224), (5264, 1520, 224, 224, 272),
          (2560, 2880, 288, 224, 256), (2912, 2880, 256, 240, 304), (3248, 2880, 256, 224, 240),
          (4576, 2880, 256, 224, 272), (4880, 2880, 288, 240, 224), (5216, 2880, 256, 224, 256),
          (2496, 1856, 224, 288, 256), (2496, 2240, 224, 288, 304), (5216, 1856, 256, 288, 240),
          (5216, 2240, 256, 288, 288)]
for index, (x, y, width, depth, height) in enumerate(houses):
    level.prefab('city_house', (x, y, QUAY), width=width, depth=depth, height=height,
                 material=(GREY, LIME_DAMP, MOSSY)[index % 3], roof=ROOF, door=DOOR, window=WINDOW,
                 trim=BRONZE)

# The Market.
level.box(GREY_FLOOR, 3200, 1840, QUAY, 4960, 2720, QUAY + 4)
for index, (x, y) in enumerate(((3360, 2500), (3560, 2500), (3760, 2500), (4380, 2500), (4580, 2500), (4780, 2500),
                                (3360, 2000), (3560, 2000), (4580, 2000), (4780, 2000))):
    level.prefab('market_stall', (x, y, QUAY + 4), width=160, depth=96, post=TIMBER, counter=TIMBER,
                 cloth=(COPPER, GREEN, RUST)[index % 3])
level.prefab('well', (4096, 2260, QUAY + 4), radius=120, height=36, sides=14, material=LIME)
level.liquid('water', 'water', 4000, 2164, QUAY + 4, 4192, 2356, QUAY + 24)
for x, y in ((3240, 1880), (3240, 2680), (4920, 1880), (4920, 2680), (3900, 1880), (4300, 2680)):
    lantern(x, y, QUAY + 4, 360)
level.prefab('sweeping_light', (4096, 2260, QUAY), reach=420, height=200, radius=620, color=[1, 0.72, 0.42], period=14)

# Quays along the long reach, with lamps.
for x in range(2500, 5600, 448):
    lantern(x, 1360 + 40 + 16, QUAY, 280)
    lantern(x + 224, 980, QUAY, 280)
level.prefab('quay_steps', (3400, 1400 - 128, QUAY), width=160, material=PAVE)

# --- The bridges ----------------------------------------------------------
level.prefab('stone_bridge', (4288, 1000, QUAY), turn=90, length=400, width=256, rise=40, depth=224,
             material=PAVE, parapet=LIME)
level.prefab('stone_bridge', (5600, 1600, QUAY), length=400, width=224, rise=40, depth=224,
             material=PAVE, parapet=LIME)
level.prefab('stone_bridge', (2000, 2080, QUAY), length=400, width=224, rise=40, depth=224,
             material=PAVE, parapet=LIME)
# The sluice that bars the west bridge, until the Foundry's wheel lifts it.
level.prefab('sluice_gate', (2424, 2192, QUAY), width=224, height=176, name='sluice_up', material=GREEN, frame=COPPER_TRIM)

# --- The Foundry ----------------------------------------------------------
# A great shed on the east bank, full of fire.
for box in ((6080, 1040, 7472, 1072), (6080, 2288, 7472, 2320), (7440, 1040, 7472, 2320)):
    level.box(BASALT, box[0], box[1], QUAY, box[2], box[3], 640)
level.box(BASALT, 6080, 1040, QUAY, 6112, 1560, 640)
level.box(BASALT, 6080, 1800, QUAY, 6112, 2320, 640)
level.box(BASALT, 6080, 1560, 288, 6112, 1800, 640)
level.box(RUST, 6064, 1024, 640, 7488, 2336, 672)
level.box(BASALT_FLOOR, 6112, 1072, QUAY, 7440, 2288, QUAY + 4)
level.box('qg02_iron_cathedral_rock', 6400, 1296, QUAY, 7200, 1400, QUAY + 4)
level.liquid('lava', LAVA, 6400, 1400, QUAY - 64, 7200, 1480, QUAY + 4 - 16)
level.liquid('lava', LAVA, 6400, 1900, QUAY - 64, 7200, 1980, QUAY + 4 - 16)
for x in (6560, 6880, 7120):
    level.prefab('pipe', (x, 1160, 560), to=[0, 1040, 0], radius=28, material=COPPER, collar=COPPER_TRIM)
    level.prefab('pipe', (x, 1700, QUAY + 4), to=[0, 0, 480], radius=20, material=COPPER, collar=COPPER_TRIM)
for x, y in ((6300, 1220), (6300, 2160), (7300, 1220), (7300, 2160)):
    level.prefab('fire_pit', (x, y, QUAY + 4), size=48)
    glow(x, y, QUAY + 110, 420, (1, 0.5, 0.2))
for x in (6600, 6900):
    glow(x, 1440, QUAY + 40, 340, (1, 0.45, 0.16))
    glow(x, 1940, QUAY + 40, 340, (1, 0.45, 0.16))
level.moving('func_button', [(7424, 1660, QUAY + 44, 7440, 1740, QUAY + 124)], RUST, angle=0,
             target='sluice_up', wait=-1, lip=4, name='foundry_wheel')
level.entity('monster_ogre', (6800, 1700, QUAY + 28), angle=180)
level.entity('monster_army', (6400, 1200, QUAY + 28), angle=90)
level.entity('monster_army', (6500, 2150, QUAY + 28), angle=270)

# --- The Fall, and the walkways ------------------------------------------
# It comes out of the east wall and feeds the canal.
level.liquid('water', 'water', 7488, 2440, -24, 7552, 2760, 860)
level.box(ROCK, 7360, 2400, 860, 7552, 2800, 1024)
for x, y, z, seed in ((6300, 3100, 320, 1), (6300, 3900, 320, 2), (7000, 3100, 320, 3), (7000, 3900, 512, 4),
                      (6300, 4600, 512, 5)):
    level.prefab('tower', (x, y, QUAY), width=224, height=z - QUAY, material=(GREY, LIME, MOSSY)[seed % 3],
                 trim=BRONZE, cap=TIMBER)
# From the terrace, east along the rail to the towers.
level.prefab('walkway', (5408, 3820, TERRACE), length=780, width=112, deck=TIMBER, rail=GREEN_TRIM, post=TIMBER, legs=256)
level.prefab('walkway', (6244, 3212, TERRACE), turn=0, length=112, width=576, deck=TIMBER, rail=GREEN_TRIM,
             post=TIMBER, legs=256, rails=False)
level.prefab('walkway', (6412, 3044, TERRACE), length=476, width=112, deck=TIMBER, rail=GREEN_TRIM, post=TIMBER, legs=256)
# A stair up the corner tower, and a lift to the high walk.
elevator('walk_lift', 6888, 3220, 7112, 3430, 512, TERRACE, wait=3, speed=150, material=GREEN)
level.prefab('walkway', (7000 - 56, 3430, 512), length=112, width=358, deck=TIMBER, rail=GREEN_TRIM, post=TIMBER,
             legs=448, rails=False)
level.prefab('walkway', (6412, 3844, 512), length=476, width=112, deck=TIMBER, rail=GREEN_TRIM, post=TIMBER, legs=448)
level.prefab('walkway', (6244, 4012, 512), length=112, width=476, deck=TIMBER, rail=GREEN_TRIM, post=TIMBER,
             legs=448, rails=False)
for x, y, z in ((6300, 3100, 320), (6300, 3900, 320), (7000, 3100, 320), (7000, 3900, 512), (6300, 4600, 512)):
    lantern(x - 80, y - 80, z, 300)
level.entity('item_armor2', (6300, 4600, 520))
level.entity('item_health', (6300, 4520, 520))
# A way down from the walkways to the east bank, by the stair at the terrace end.
elevator('quay_lift', 5960, 3200, 6200, 3400, TERRACE, QUAY, wait=3, speed=150, material=GREEN)
level.box(FLAGS, 6000, 2800, QUAY - 8, 6200, 3200, QUAY)

# --- The Pumpworks --------------------------------------------------------
level.box(GREY, 800, 1120, QUAY, 1840, 1152, 480)
level.box(GREY, 800, 2640, QUAY, 1840, 2672, 480)
level.box(GREY, 800, 1120, QUAY, 832, 2672, 480)
level.box(GREY, 1808, 1120, QUAY, 1840, 2060, 480)
level.box(GREY, 1808, 2320, QUAY, 1840, 2672, 480)
level.box(GREY, 1808, 2060, 272, 1840, 2320, 480)
level.box(GREEN, 784, 1104, 480, 1856, 2688, 512)
level.box(GREY_FLOOR, 832, 1152, QUAY, 1808, 2640, QUAY + 4)
for x in (1000, 1240, 1480):
    level.prefab('pipe', (x, 1200, QUAY + 200), to=[0, 1400, 0], radius=48, material=GREEN, collar=COPPER_TRIM)
    level.prefab('column', (x, 1500, QUAY + 4), radius=30, height=196, sides=8, material=COPPER, trim=COPPER_TRIM)
    level.prefab('column', (x, 2300, QUAY + 4), radius=30, height=196, sides=8, material=COPPER, trim=COPPER_TRIM)
for y in (1400, 2000, 2500):
    glow(1320, y, 400, 360, (0.72, 0.9, 0.8))
level.entity('monster_knight', (1320, 1900, QUAY + 28), angle=0)
level.entity('monster_army', (1000, 1300, QUAY + 28), angle=0)
level.entity('monster_army', (1600, 2500, QUAY + 28), angle=180)
# The way on.
level.carve(320, 1800, QUAY, 640, 1960, 224, floor=PAVE, wall=LIME, ceiling=LIME)
level.box(FLAGS, 640, 1800, QUAY - 8, 832, 1960, QUAY + 4)
level.moving('trigger_changelevel', [(340, 1800, QUAY, 380, 1960, 224)], 'hidden', map='ember_keep')
glow(480, 1880, 190, 280, (1, 0.85, 0.6))

# --- Life in the city -----------------------------------------------------
level.prefab('patrol', (3300, 2200, QUAY + 4), name='market_watch', size=260)
level.prefab('patrol', (4500, 1200 + 240, QUAY), name='quay_watch', size=160)
level.prefab('seeker', (3700, 2740, QUAY + 4), goal=[4400, 2760, QUAY + 28])
level.entity('monster_army', (4096, 3000, QUAY + 28), angle=270)
level.entity('monster_dog', (3700, 2400, QUAY + 28), angle=0)
level.entity('monster_dog', (4400, 2300, QUAY + 28), angle=180)
for x, y, z in ((1000, 4200, 900), (3000, 2400, 900), (5200, 3200, 900), (6800, 1400, 800), (2000, 1500, 800)):
    level.prefab('rock_spire', (x, y, 1024), radius=110, length=300, material=ROCK, seed=x % 97, down=True)
level.prefab('rock_wall', (660, 660, -32), length=6800, height=900, depth=120, material=ROCK, seed=61)
level.prefab('rock_wall', (660, 5480, -32), length=6800, height=900, depth=-120, material=ROCK, seed=62)

level.save()
