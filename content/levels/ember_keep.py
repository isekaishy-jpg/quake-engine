"""The Ember Keep: a classic level, in `qg01` and `qg02` (proposal 0097).

You start on a causeway over a lava moat, shut out of the keep. A side door takes
you under it and up into the Great Hall, which everything leaves from and returns
to. The Forge holds the silver key, and a tower over the gate that opens the way
back to the start. The silver door leads down to the Crypt and the gold key, and
a lift from there comes up on the balcony. The gold door is the end.
"""
from kit import Level

STONE = 'qg01_ashen_abbey_wall_blocks'
BRICK = 'qg01_ashen_abbey_wall_brick'
DAMP = 'qg01_ashen_abbey_wall_damp'
ROCK = 'qg01_ashen_abbey_rock'
FLAGS = 'qg01_ashen_abbey_floor_flags'
TILES = 'qg01_ashen_abbey_floor_tiles'
COFFER = 'qg01_ashen_abbey_ceiling_coffer'
CARVED = 'qg01_ashen_abbey_trim_carved'
IRON = 'qg01_ashen_abbey_trim_metal'
PLATE = 'qg01_ashen_abbey_metal_plate'
WOOD = 'qg01_ashen_abbey_wood_planks'
GRILLE = 'qg01_ashen_abbey_grille_opaque'
LAVA = 'qg01_ashen_abbey_liquid'
RELIEF = 'qg01_ashen_abbey_relief_panel'
BASALT = 'qg02_iron_cathedral_wall_blocks'
BASALT_FLOOR = 'qg02_iron_cathedral_floor_flags'
RUST = 'qg02_iron_cathedral_metal_plate'
MOLTEN = 'qg02_iron_cathedral_liquid'
GATE = 'qg02_iron_cathedral_grille_opaque'
CRYPT = 'qg03_elder_crypt_wall_blocks'
CRYPT_DAMP = 'qg03_elder_crypt_wall_damp'
CRYPT_FLOOR = 'qg03_elder_crypt_floor_flags'
CRYPT_WATER = 'qg03_elder_crypt_liquid'

SILVER, GOLD = 16, 8

level = Level(
    'ember_keep',
    (-256, -1792, -512, 3584, 3072, 1408),
    ROCK,
    {'ambient': 8, 'light_spacing': 16, 'bounce': 1, 'cell_size': 512, 'sun_intensity': 90,
     'sun_crisp': True, 'worldtype': 1},
    blocks='''sky {
    zenith = 0.10 0.07 0.10
    horizon = 0.75 0.34 0.16
    sun_direction = -0.55 -0.5 0.45
    sun_color = 1 0.62 0.38
    clouds { coverage = 0.75 }
}
fog { color = 0.5 0.26 0.16 density = 0.00035 height = 0 falloff = 0.004 }''')


def torch(x, y, z, intensity=240):
    level.prefab('torch', (x, y, z), intensity=intensity, style=1)


def lamp(x, y, z, intensity=260, color=(1, 0.72, 0.45)):
    level.entity('light', (x, y, z), intensity=intensity, color=list(color))


def monster(kind, x, y, z, angle=0, flags=None, target=None):
    level.entity(f'monster_{kind}', (x, y, z + 24), angle=angle, spawnflags=flags, targetname=target)


def item(kind, x, y, z):
    level.entity(kind, (x, y, z + 8))


# --- The Causeway ---------------------------------------------------------
# Out of doors, on a neck of rock in a moat of lava, under a burning sky.
level.carve(512, -1600, -256, 2560, -128, 1280, floor=ROCK, wall=ROCK, ceiling='sky')
level.liquid('lava', LAVA, 512, -1600, -256, 2560, -128, -176)
# The neck of rock you start on, and the way across.
level.box(ROCK, 1120, -1600, -256, 1952, -1216, 0)
level.prefab('stone_bridge', (1408, -1216, 0), length=896, width=256, rise=0, depth=256,
             material=FLAGS, parapet=STONE, segments=4)
level.box(FLAGS, 1120, -320, -256, 1952, -128, 0)
level.box(FLAGS, 640, -320, -256, 1120, -128, 0)
# The keep's face over the moat, with arrow slits and buttresses.
for x in (704, 1120, 1952, 2368):
    level.box(STONE, x - 48, -192, 0, x + 48, -128, 768)
    level.box(BASALT, x - 56, -200, 768, x + 56, -128, 800)
level.box(STONE, 512, -160, 0, 2560, -128, 672)
level.box(IRON, 512, -176, 672, 2560, -128, 704)
for x in range(544, 2560, 128):
    level.box(STONE, x, -176, 704, x + 64, -128, 768)

# --- The gate -------------------------------------------------------------
# Seen from the start, shut. The tower's button lifts it, late on.
level.carve(1408, -128, 0, 1664, 256, 224, floor=FLAGS, wall=STONE, ceiling=COFFER)
level.moving('func_door', [(1408, -136, 0, 1664, -120, 224)], GATE, name='main_gate',
             targetname='gate_up', angle=-1, wait=-1, speed=80, lip=8)
torch(1360, -200, 112)
torch(1712, -200, 112)

# --- The Undercroft -------------------------------------------------------
# The only way in: a door at the end of the ledge, and stairs down.
level.carve(640, -128, -128, 896, 576, 128, floor=FLAGS, wall=BRICK, ceiling=STONE)
level.moving('func_door', [(704, -136, 0, 832, -120, 128)], PLATE, angle=-1, speed=100, wait=4, lip=4)
level.carve(704, -128, 0, 832, 0, 128, floor=FLAGS, wall=BRICK, ceiling=STONE)
for step in range(8):
    level.box(FLAGS, 640, -128 + step * 32, -128, 896, -128 + step * 32 + 32, -step * 16)
torch(652, 256, -32)
torch(884, 256, -32)
monster('army', 768, 500, -128, angle=270)

# The pillared hall, low and dark, where the first real fight is.
level.carve(256, 576, -128, 1088, 1088, 160, floor=FLAGS, wall=DAMP, ceiling=STONE)
for x in (416, 672, 928):
    for y in (704, 960):
        level.prefab('column', (x, y, -128), radius=26, height=288, sides=8, material=STONE, trim=IRON)
monster('army', 480, 832, -128, angle=0)
monster('dog', 864, 800, -128, angle=180)
monster('dog', 864, 880, -128, angle=180)
item('item_shells', 320, 640, -128)
item('item_health', 320, 1020, -128)
lamp(544, 832, 120, 180)
lamp(960, 640, 120, 140)
torch(268, 832, -32)

# Up into the Great Hall.
level.carve(1088, 736, -128, 1216, 928, 96, floor=FLAGS, wall=BRICK, ceiling=STONE)
for step in range(8):
    level.box(FLAGS, 1088 + step * 16, 736, -128, 1088 + step * 16 + 16, 928, -128 + (step + 1) * 16)

# --- The Great Hall -------------------------------------------------------
# The hub. Everything leaves from here and comes back.
level.carve(1216, 256, 0, 1856, 1408, 576, floor=TILES, wall=STONE, ceiling=COFFER)
for y in (416, 672, 928, 1184):
    for x in (1344, 1728):
        level.prefab('column', (x, y, 0), radius=34, height=576, sides=10, material=STONE, trim=CARVED)
# A balcony round three sides, reached from the Crypt's lift.
level.box(FLAGS, 1216, 256, 256, 1296, 1408, 272)
level.box(FLAGS, 1776, 256, 1856 - 1856 + 256, 1856, 1408, 272)
level.box(FLAGS, 1296, 1328, 256, 1776, 1408, 272)
for y in range(256, 1408, 64):
    level.box(IRON, 1296, y, 272, 1300, y + 8, 304)
    level.box(IRON, 1772, y, 272, 1776, y + 8, 304)
level.box(IRON, 1296, 256, 304, 1300, 1328, 308)
level.box(IRON, 1772, 256, 304, 1776, 1328, 308)
# Light falling through a high window over the gold door.
level.box(CARVED, 1216, 1392, 224, 1856, 1408, 256)
lamp(1536, 832, 480, 420)
lamp(1536, 500, 300, 240)
lamp(1536, 1200, 300, 240)
torch(1228, 544, 96)
torch(1228, 1088, 96)
torch(1844, 544, 96)
torch(1844, 1088, 96)
monster('knight', 1536, 800, 0, angle=270)
monster('army', 1280, 1320, 256, angle=270)
monster('army', 1800, 1320, 256, angle=270)
item('item_armor1', 1536, 1320, 0)
item('item_spikes', 1280, 340, 0)

# The gold door, at the head of the hall, with the keep's emblem over it.
level.carve(1472, 1408, 0, 1600, 1536, 160, floor=TILES, wall=BASALT, ceiling=STONE)
level.moving('func_door', [(1472, 1412, 0, 1536, 1428, 160)], 'qg02_iron_cathedral_door',
             angle=180, speed=90, wait=-1, spawnflags=GOLD, lip=4)
level.moving('func_door', [(1536, 1412, 0, 1600, 1428, 160)], 'qg02_iron_cathedral_door',
             angle=0, speed=90, wait=-1, spawnflags=GOLD, lip=4)
level.box(RELIEF, 1472, 1400, 176, 1600, 1408, 304)

# --- The Forge ------------------------------------------------------------
level.carve(1856, 640, 0, 2176, 800, 160, floor=FLAGS, wall=BASALT, ceiling=STONE)
level.moving('func_door', [(1864, 640, 0, 1880, 800, 160)], RUST, angle=90, speed=120, wait=4, lip=4)
level.carve(2176, 128, -160, 3200, 1280, 448, floor=BASALT_FLOOR, wall=BASALT, ceiling=RUST)
level.liquid('lava', MOLTEN, 2176, 128, -160, 3200, 1280, -48)
# Stone walks over the channels, in a cross.
level.box(BASALT_FLOOR, 2176, 640, -160, 3200, 800, 0)
level.box(BASALT_FLOOR, 2624, 128, -160, 2784, 1280, 0)
level.box(BASALT_FLOOR, 2176, 128, -160, 2336, 320, 0)
# The furnace at the east end.
level.box(RUST, 3040, 560, 0, 3200, 880, 320)
level.box(GATE, 3032, 624, 32, 3040, 816, 224)
level.entity('light', (3000, 720, 128), intensity=500, color=[1, 0.45, 0.15])
# A lift up to the gantry, and the gantry to the key.
level.prefab('lift', (2624, 1120, 0), size=160, height=256, name='forge_lift', material=RUST)
level.prefab('walkway', (2240, 1160, 256), length=384, width=80, deck=WOOD, rail=IRON, post=IRON)
level.prefab('walkway', (2240, 240, 256), length=80, width=920, deck=WOOD, rail=IRON, post=IRON)
level.box(BASALT_FLOOR, 2912, 160, -160, 3120, 368, 256)
level.prefab('walkway', (2320, 240, 256), length=592, width=80, deck=WOOD, rail=IRON, post=IRON)
item('item_key1', 3016, 264, 256)
monster('ogre', 2704, 720, 0, angle=180)
monster('army', 2240, 220, 0, angle=0)
monster('knight', 2704, 1040, 0, angle=270)
item('item_rockets', 2256, 180, 0)
item('item_health', 2256, 260, 0)
for y in (320, 1088):
    level.entity('light', (2704, y, 40), intensity=320, color=[1, 0.5, 0.2])
for x in (2400, 2960):
    level.entity('light', (x, 960, 20), intensity=260, color=[1, 0.42, 0.14])
    level.entity('light', (x, 480, 20), intensity=260, color=[1, 0.42, 0.14])
level.entity('light', (2704, 720, 380), intensity=220, color=[1, 0.6, 0.35])

# A secret behind the furnace.
level.carve(3200, 656, 0, 3392, 784, 128, floor=FLAGS, wall=STONE, ceiling=STONE)
level.moving('func_door_secret', [(3192, 656, 0, 3208, 784, 128)], RUST, angle=0, wait=-1)
level.moving('trigger_secret', [(3216, 656, 0, 3248, 784, 128)], 'hidden', message='You found the furnace vault')
item('item_armor2', 3320, 720, 0)
item('item_rockets', 3360, 680, 0)
lamp(3300, 720, 96, 140)

# --- The Gate Tower -------------------------------------------------------
# From the gantry, a passage to the tower over the gate. Its button opens the
# way back to the start.
level.carve(1856, 288, 256, 2240, 416, 400, floor=FLAGS, wall=STONE, ceiling=STONE)
level.carve(1920, -112, 256, 2240, 288, 512, floor=TILES, wall=STONE, ceiling=COFFER)
level.moving('func_button', [(1928, 80, 320, 1936, 144, 384)], PLATE, angle=180, target='gate_up',
             wait=-1, lip=4)
lamp(2080, 96, 440, 240)
monster('army', 2120, 40, 256, angle=180)
item('item_health', 2180, 200, 256)
# A ladder of light down to the hall's balcony, by the same passage.
level.carve(1856, 288, 256, 1860, 416, 400, floor=FLAGS, wall=STONE, ceiling=STONE)

# --- The silver door, and the Crypt ---------------------------------------
level.carve(1088, 1120, 0, 1216, 1248, 144, floor=FLAGS, wall=CRYPT, ceiling=STONE)
level.moving('func_door', [(1200, 1120, 0, 1216, 1248, 144)], 'qg03_elder_crypt_door', angle=90, speed=90,
             wait=-1, spawnflags=SILVER, lip=4)
# Stairs down to the crypt.
level.carve(832, 1120, -192, 1088, 1248, 144, floor=CRYPT_FLOOR, wall=CRYPT, ceiling=CRYPT)
for step in range(12):
    level.box(CRYPT_FLOOR, 1088 - (step + 1) * 21, 1120, -192, 1088 - step * 21, 1248, -step * 16)
level.carve(128, 1120, -320, 832, 2240, 192, floor=CRYPT_FLOOR, wall=CRYPT_DAMP, ceiling=CRYPT)
level.liquid('water', CRYPT_WATER, 128, 1120, -320, 832, 2240, -224)
# Raised walks between the flooded bays.
level.box(CRYPT_FLOOR, 128, 1120, -320, 832, 1280, -192)
level.box(CRYPT_FLOOR, 400, 1280, -320, 560, 2240, -192)
level.box(CRYPT_FLOOR, 128, 2080, -320, 832, 2240, -192)
for y in (1400, 1656, 1912):
    for x in (272, 688):
        level.prefab('column', (x, y, -320), radius=30, height=512, sides=8, material=CRYPT,
                     trim='qg03_elder_crypt_trim_metal')
# The altar, and the key on it. Taking it opens the walls.
level.box(CRYPT, 416, 2080, -192, 544, 2176, -144)
level.box('qg03_elder_crypt_relief_panel', 400, 2224, -160, 560, 2240, 0)
item('item_key2', 480, 2128, -144)
level.moving('trigger_once', [(400, 2064, -192, 560, 2200, -64)], 'hidden', target='crypt_trap')
for x, side in ((192, 90), (704, 90)):
    level.carve(x - 48, 2240, -192, x + 48, 2368, -48, floor=CRYPT_FLOOR, wall=CRYPT, ceiling=CRYPT)
    level.moving('func_door', [(x - 48, 2232, -192, x + 48, 2248, -48)], CRYPT, angle=-2,
                 targetname='crypt_trap', wait=-1, speed=200)
    monster('zombie', x, 2304, -192, angle=270)
    monster('knight', x, 2336, -192, angle=270)
monster('zombie', 480, 1500, -192, angle=90)
monster('zombie', 480, 1800, -192, angle=90)
monster('wizard', 200, 1500, -160, angle=0)
item('item_cells', 480, 1200, -192)
item('item_health', 760, 2160, -192)
for y in (1400, 1900):
    level.entity('light', (480, y, 40), intensity=200, color=[0.45, 0.9, 0.85])
level.entity('light', (480, 2120, -60), intensity=320, color=[0.5, 1, 0.9])
level.entity('light', (160, 1200, 40), intensity=130, color=[0.5, 0.9, 0.8])
# A flooded way to a secret, under the walk.
level.carve(128, 1696, -320, 400, 1824, -224)
level.carve(-128, 1696, -320, 128, 1824, -128, floor=CRYPT_FLOOR, wall=CRYPT, ceiling=CRYPT)
level.moving('trigger_secret', [(-96, 1696, -300, -64, 1824, -128)], 'hidden', message='A drowned reliquary')
item('item_artifact_invulnerability', -40, 1760, -320)

# The lift out, up to the balcony.
level.carve(832, 2096, -192, 1120, 2240, 272, floor=CRYPT_FLOOR, wall=CRYPT, ceiling=CRYPT)
level.prefab('lift', (896, 2100, -192), size=128, height=464, name='crypt_lift', material=PLATE)
level.carve(1024, 1248, 256, 1216, 2240, 416, floor=FLAGS, wall=STONE, ceiling=STONE)
lamp(1120, 1600, 380, 160)
lamp(1120, 2150, 380, 160)

# --- The Sanctum ----------------------------------------------------------
level.carve(1088, 1536, -64, 1984, 2624, 704, floor=TILES, wall=BASALT, ceiling=COFFER)
level.liquid('lava', MOLTEN, 1216, 1792, -64, 1856, 2368, 0 - 32)
level.box(BASALT_FLOOR, 1088, 1536, -64, 1984, 1792, 0)
level.box(BASALT_FLOOR, 1088, 1792, -64, 1216, 2624, 0)
level.box(BASALT_FLOOR, 1856, 1792, -64, 1984, 2624, 0)
level.box(BASALT_FLOOR, 1216, 2368, -64, 1856, 2624, 0)
for y in (1760, 2000, 2240, 2480):
    for x in (1152, 1920):
        level.prefab('column', (x, y, 0), radius=32, height=704, sides=10, material=BASALT, trim=IRON)
level.entity('light', (1536, 2080, 40), intensity=500, color=[1, 0.42, 0.14])
lamp(1536, 1640, 400, 340, (1, 0.6, 0.4))
lamp(1536, 2480, 400, 340, (1, 0.6, 0.4))
monster('shambler', 1536, 2480, 0, angle=270)
monster('knight', 1160, 2200, 0, angle=0)
monster('knight', 1912, 2200, 0, angle=180)
item('item_health', 1140, 1600, 0)
item('item_shells', 1940, 1600, 0)
# The way out.
level.carve(1472, 2624, 0, 1600, 2880, 160, floor=TILES, wall=BASALT, ceiling=STONE)
level.moving('trigger_changelevel', [(1472, 2800, 0, 1600, 2880, 160)], 'hidden', map='vesper_deep')
lamp(1536, 2740, 120, 300, (1, 0.85, 0.7))

level.entity('info_player_start', (1536, -1400, 24), angle=90)
level.entity('info_intermission', (1536, -1200, 200), mangle=[10, 90, 0])

level.save()
