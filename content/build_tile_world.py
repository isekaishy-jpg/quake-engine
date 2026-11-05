"""Writes `maps/tile_world.emap` and its tiles: a 3 by 3 grid of 2048 unit tiles,
each a walled yard joined to its neighbours, with something of its own.

Run from anywhere. The tiles are local to their corners, so a yard's text is the
same wherever it sits on the grid.
"""
import os

ROOT = os.path.dirname(os.path.abspath(__file__))
MAPS = os.path.join(ROOT, 'maps')
SIZE = 2048
GAP = 192  # The opening between yards.


def wall_x(x, y0, y1, gap):
    """A wall along y at `x`, with a gap in its middle if it's wanted."""
    middle = (y0 + y1) // 2
    if not gap:
        return [f'brush {{ material = brick cuboid = {x} {y0} -32 {x + 32} {y1} 320 }}']
    half = GAP // 2
    return [
        f'brush {{ material = brick cuboid = {x} {y0} -32 {x + 32} {middle - half} 320 }}',
        f'brush {{ material = brick cuboid = {x} {middle + half} -32 {x + 32} {y1} 320 }}',
    ]


def wall_y(y, x0, x1, gap):
    """A wall along x at `y`, with a gap in its middle if it's wanted."""
    middle = (x0 + x1) // 2
    if not gap:
        return [f'brush {{ material = brick cuboid = {x0} {y} -32 {x1} {y + 32} 320 }}']
    half = GAP // 2
    return [
        f'brush {{ material = brick cuboid = {x0} {y} -32 {middle - half} {y + 32} 320 }}',
        f'brush {{ material = brick cuboid = {middle + half} {y} -32 {x1} {y + 32} 320 }}',
    ]


def tile_text(x, y):
    """The yard at grid place `x`, `y`, from 0 to 2."""
    top = SIZE
    lines = ['format 1 tile', '']
    lines.append(f'brush {{ material = stone cuboid = 0 0 -32 {top} {top} 0 }}')
    lines += wall_x(0, 0, top, x > 0)
    lines += wall_y(0, 0, top, y > 0)
    if x == 2:
        lines += wall_x(top - 32, 0, top, False)
    if y == 2:
        lines += wall_y(top - 32, 0, top, False)
    lines.append(f'mesh {{ source = statue origin = 700 900 0 angles = 0 {(x * 3 + y) * 40} 0 scale = 2 }}')
    lines.append('mesh { source = barrel origin = 1300 500 0 }')
    lines.append(f'entity lamp_{x}_{y} {{ type = light origin = 1024 1024 280 intensity = 620 }}')
    lines.append(
        f'entity flame_{x}_{y} {{ type = flame origin = 400 1600 0 size = 40 }}')
    lines.append(
        f'decal {{ material = lettering origin = 1024 1600 0 normal = 0 0 1 size = 256 angle = {x * 30 + y * 15} }}')
    lines.append(f'spline guard_{x}_{y} {{ closed = true points = 300 300 24  900 300 24  900 700 24  300 700 24 }}')
    lines.append(f'entity guard_{x}_{y} {{ type = patrol_walker origin = 300 300 24 route = guard_{x}_{y} }}')
    return '\n'.join(lines) + '\n'


def main():
    folder = os.path.join(MAPS, 'tile_world')
    os.makedirs(folder, exist_ok=True)
    head = [
        'format 1 map',
        '',
        '# Nine tiles on a grid, for a world in parts. Each is a file of its own.',
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
        f'tile_grid {{ size = {SIZE} }}',
    ]
    for y in range(3):
        for x in range(3):
            name = f'{x}_{y}'
            head.append(f'tile {{ x = {x} y = {y} file = "tile_world/{name}" }}')
            with open(os.path.join(folder, name + '.etile'), 'w', encoding='utf-8', newline='\n') as f:
                f.write(tile_text(x, y))
    head.append('entity start { type = info_player_start origin = 1024 400 24 angle = 90 }')
    with open(os.path.join(MAPS, 'tile_world.emap'), 'w', encoding='utf-8', newline='\n') as f:
        f.write('\n'.join(head) + '\n')
    # The guards' script is the showcase's.
    with open(os.path.join(MAPS, 'showcase.luau'), encoding='utf-8') as f:
        script = f.read()
    with open(os.path.join(MAPS, 'tile_world.luau'), 'w', encoding='utf-8', newline='\n') as f:
        f.write(script)


main()
