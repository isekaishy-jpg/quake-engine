"""Writes the batch that builds the showcase map through the platform's tools:

    engine-tool content content/maps/showcase.emap --batch content/showcase.json

Five halls in a row joined by doorways, each for a family of features, built
almost wholly from prefabs.
"""
import json
import os

ops = []


def op(op_name, **args):
    ops.append({'op': op_name, 'args': args})


def prefab(prefab_name, at, **params):
    op('place_prefab', prefab=prefab_name, at=at, params=params)


op('add_entity', type='worldspawn', fields={
    'sun_intensity': 120, 'sun_size': 12, 'sun_crisp': True, 'ambient': 34, 'light_spacing': 16,
    'bounce': 1, 'cell_size': 512})
op('set_block', kind='sky', fields={
    'zenith': [0.16, 0.34, 0.62], 'horizon': [0.85, 0.8, 0.72], 'sun_direction': [-0.75, 0.1, 0.65]})
op('set_block', kind='fog', fields={'color': [0.8, 0.85, 0.95], 'density': 0.0004, 'height': 0, 'falloff': 0.01})

HALL = 1024
GAP = 32


def hall(index, floor='stone', wall='brick', height=384, ceiling='metal'):
    x = index * (HALL + GAP)
    prefab('room', [x, 0, 0], size=[HALL, 768, height], wall=wall, floor=floor, ceiling=ceiling)
    return x


def door_to_next(x, height=384):
    wall_x = x + HALL
    prefab('doorway', [wall_x, 384, 0], width=768, height=height, opening=128, opening_height=160)


# Hall 0: building blocks and entities. Prefabs of every plain kind.
x = hall(0)
op('add_entity', type='info_player_start', origin=[160, 384, 24], fields={'angle': 0})
prefab('stairs', [320, 64, 0], steps=6, width=128, material='mossy_stone')
prefab('ramp', [640, 64, 0], length=224, height=96, width=128, material='stone')
prefab('railing', [320, 192, 96], length=192)
prefab('pillar', [320, 576, 0], size=64, height=384, material='metal')
prefab('pillar', [576, 576, 0], size=64, height=384, material='cel_stone')
prefab('pillar', [832, 576, 0], size=64, height=384, material='shiny')
prefab('crate', [128, 640, 0], size=48)
prefab('crate', [176, 640, 0], size=48, material='brick')
prefab('crate', [152, 640, 48], size=48, material='mossy_stone')
prefab('item', [256, 320, 0], kind='item_health')
prefab('item', [256, 448, 0], kind='item_armor1')
prefab('item', [320, 384, 0], kind='weapon_supershotgun')
prefab('torch', [8, 192, 96], intensity=240, style=1)
prefab('torch', [8, 576, 96], intensity=240, style=1)
prefab('lamp', [512, 384, 384], intensity=420)
prefab('tell_tale', [480, 384, 0], message='the showcase says hello')
door_to_next(x)
prefab('door', [x + HALL + 8, 384, 0], name='first_door', width=128, height=160)

# Hall 1: surfaces. Decals, stamps, paint, hotspots, reflections and flow.
x = hall(1, floor='stone')
prefab('mirror_floor', [x + 128, 128, 0], size=512)
prefab('pillar', [x + 320, 320, 4], size=64, height=380, material='stone')
prefab('trim_panels', [x + 256, 744, 32], length=480, height=64)
prefab('poster', [x + 120, 750, 180], material='lettering', normal=[0, -1, 0], size=128)
op('add_brush', cuboid=[x + 700, 16, 0, x + 1000, 48, 288], material='stone')
prefab('brick_relief', [x + 780, 48, 170], normal=[0, 1, 0], size=[128, 192])
prefab('carved_sign', [x + 920, 48, 150], normal=[0, 1, 0], size=120, depth=12)
prefab('moss_stroke', [x + 720, 320, 0], size=224)
prefab('lava_slab', [x + 720, 560, 0], size=160)
prefab('lamp', [x + 512, 384, 384], intensity=460)
door_to_next(x)

# Hall 2: light. A stained window, baked motion, flames, moving lights, shadows.
x = hall(2, wall='brick', height=448)
# The outer wall gives way to the window.
prefab('stained_window', [x + 992, 384, 0], width=480, height=320, opening=224, sill=48, top=272, sun=[0.75, 0.1, 0.65])
prefab('statue', [x + 300, 260, 0], scale=1.4, angle=30)
prefab('statue', [x + 300, 520, 0], scale=1.1, angle=120)
prefab('statue', [x + 620, 384, 0], mesh='barrel', scale=2)
prefab('rolling_barrel', [x + 780, 200, 0], name='rolling', travel=128)
prefab('fire_pit', [x + 150, 150, 0], size=36)
prefab('fire_pit', [x + 150, 620, 0], size=36)
prefab('sweeping_light', [x + 460, 384, 0], reach=200, height=120)
prefab('swinging_lamp', [x + 780, 620, 240], length=112)
op('add_entity', type='live_light', origin=[x + 40, 700, 300], fields={
    'color': [0.3, 0.5, 1], 'radius': 700, 'spot_direction': [0.7, -0.7, -0.3], 'spot_angle': 16, 'shadows': True})
# A grille with a lamp behind it, whose boxes bodies and doors can block.
op('add_entity', type='ray_stencil', name='grille', fields={
    'image': 'glass_rays', 'marks': 'glass_marks', 'corner': [x + 512, 80, 280], 'across': [160, 0, 0],
    'down': [0, 0, -200], 'light': [x + 592, 16, 180], 'strength': 0.6, 'reach': 600})
door_to_next(x, 448)

# Hall 3: water, fog, and things that wander.
x = hall(3, floor='stone')
prefab('pool', [x + 320, 192, 0], size=384, depth=224, liquid='water')
prefab('pool', [x + 760, 520, 0], size=160, depth=64, liquid='slime')
prefab('patrol', [x + 96, 96, 0], name='guard_route', size=160)
prefab('seeker', [x + 96, 640, 0], goal=[x + 900, 120, 24])
prefab('monster', [x + 880, 384, 0], kind='monster_army', angle=180)
prefab('monster', [x + 880, 256, 0], kind='monster_dog', angle=180)
prefab('lift', [x + 880, 64, 0], size=112, height=192, name='lift_1')
prefab('portal', [x + 96, 400, 0], name='back_home', to=[160, 384, 40])
prefab('lamp', [x + 512, 384, 384], intensity=420, color=[0.8, 0.9, 1])
prefab('lamp', [x + 200, 180, 384], intensity=300, color=[1, 0.7, 0.5])
prefab('monster', [x + 520, 120, 0], kind='monster_ogre', angle=90)

json.dump(ops, open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'showcase.json'), 'w'), indent=0)
print(len(ops), 'operations in content/showcase.json')
