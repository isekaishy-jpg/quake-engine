"""Writes `maps/wide_world.emap`: `tile_world`'s yards repeated over a 16 by 16
grid, which is enough to show that light and paths stream.

Only the map's text is written. Its tiles are `tile_world`'s, reused, so a yard
keeps walls on the outside of the grid and gaps everywhere else.
"""
import os
import shutil

ROOT = os.path.dirname(os.path.abspath(__file__))
MAPS = os.path.join(ROOT, 'maps')
SIDE = 16


def place(at):
    """Which of a 3 by 3 grid's columns stands for `at` on a bigger one."""
    if at == 0:
        return 0
    return 2 if at == SIDE - 1 else 1


lines = [
    'format 1 map',
    '',
    '# tile_world\'s yards, 16 by 16, to show streaming. Written by build_wide_world.py.',
    'entity world {',
    '    type = worldspawn',
    '    sun_intensity = 120',
    '    sun_crisp = true',
    '    ambient = 36',
    '    light_spacing = 32',
    '    cell_size = 1024',
    '    wake_reach = 1800',
    '}',
    'sky {',
    '    zenith = 0.16 0.34 0.62',
    '    horizon = 0.85 0.8 0.72',
    '    sun_direction = 0.4 0.3 0.85',
    '    clouds { coverage = 0.4 }',
    '}',
    'tile_grid { size = 2048 }',
]
for y in range(SIDE):
    for x in range(SIDE):
        lines.append(f'tile {{ x = {x} y = {y} file = "tile_world/{place(x)}_{place(y)}" }}')
lines.append('entity start { type = info_player_start origin = 1024 400 24 angle = 90 }')
with open(os.path.join(MAPS, 'wide_world.emap'), 'w', newline='\n') as out:
    out.write('\n'.join(lines) + '\n')
shutil.copyfile(os.path.join(MAPS, 'tile_world.luau'), os.path.join(MAPS, 'wide_world.luau'))
print(SIDE * SIDE, 'tiles in maps/wide_world.emap')
