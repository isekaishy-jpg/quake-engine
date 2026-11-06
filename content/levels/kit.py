"""What a level script builds with (proposal 0097).

A level starts as solid rock. `carve` cuts spaces out of it, so walls have real
thickness and nothing can leak, and each surface takes its material from the
space it faces. Detail goes in on top: brushes, entities and prefabs. `save`
writes the map's text, and a batch of prefabs for `engine-tool` to place.
"""
import json
import math
import os
import random
import subprocess

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TOOL = os.path.join(os.path.dirname(ROOT), 'target', 'release', 'engine-tool.exe')

# Which side of a box each direction is: axis, and whether it's the high side.
SIDES = [(0, True), (0, False), (1, True), (1, False), (2, True), (2, False)]


def value_text(value):
    """A value as map text."""
    if value is True or value is False:
        return 'true' if value else 'false'
    if isinstance(value, str):
        return json.dumps(value)
    if isinstance(value, (list, tuple)):
        return ' '.join(value_text(part) for part in value)
    return f'{value:g}' if isinstance(value, float) else str(value)


def fields_text(fields):
    """Fields as map text, on one line."""
    return ' '.join(f'{key} = {value_text(value)}' for key, value in fields.items() if value is not None)


class Space:
    """A carved space, and what its surfaces are."""

    def __init__(self, box, floor, wall, ceiling):
        self.box = box
        self.floor = floor
        self.wall = wall
        self.ceiling = ceiling


def overlap(a, b):
    """Whether two boxes share any volume."""
    return all(a[axis] < b[axis + 3] and b[axis] < a[axis + 3] for axis in range(3))


def minus(solid, hole):
    """What's left of box `solid` with `hole` cut out, as boxes."""
    if not overlap(solid, hole):
        return [solid]
    left = []
    rest = list(solid)
    for axis in range(3):
        if rest[axis] < hole[axis]:
            piece = list(rest)
            piece[axis + 3] = hole[axis]
            left.append(tuple(piece))
            rest[axis] = hole[axis]
        if rest[axis + 3] > hole[axis + 3]:
            piece = list(rest)
            piece[axis] = hole[axis + 3]
            left.append(tuple(piece))
            rest[axis + 3] = hole[axis + 3]
    return left


def write_mesh(name, parts):
    """Writes `content/meshes/<name>.emesh`, from (material, points, uvs, triangles)."""
    def numbers(rows):
        return ' '.join(f'{value:g}' for row in rows for value in row)
    lines = ['format 1 mesh', '']
    for index, (material, points, uvs, triangles) in enumerate(parts):
        lines += [f'part part_{index} {{', f'    material = {material}', f'    points = {numbers(points)}',
                  f'    uvs = {numbers(uvs)}', f'    triangles = {numbers(triangles)}', '}']
    with open(os.path.join(ROOT, 'meshes', name + '.emesh'), 'w', encoding='utf-8', newline='\n') as out:
        out.write('\n'.join(lines) + '\n')


def falling(stops, drop_speed=70, pull=800):
    """The course of water through `stops`, a list of (x, z): it runs level to
    each one, then falls in an arc to the height of the next."""
    out = [stops[0]]
    for (x, z), (_, below) in zip(stops[1:], stops[2:]):
        out.append((x, z))
        seconds = math.sqrt(2 * (z - below) / pull)
        out += [(x - drop_speed * seconds * step / 12, z - (z - below) * (step / 12) ** 2) for step in range(1, 13)]
    return out


def ribbon(course, y0, y1, seed, *, back=0.0, ripple=8.0, drop_speed=70, pull=800):
    """A sheet of water down `course`, a list of (x, z), between y0 and y1, as
    (points, uvs, triangles). Along it, `v` counts seconds, so that a texture
    moving at one speed stretches as the water falls faster."""
    rng = random.Random(seed)
    phases = [rng.uniform(0, math.tau) for _ in range(6)]
    columns = max(2, int((y1 - y0) / 24))
    points, uvs, triangles = [], [], []
    seconds = 0.0
    top = course[0][1]
    for row, (x, z) in enumerate(course):
        before, after = course[max(row - 1, 0)], course[min(row + 1, len(course) - 1)]
        along_x, along_z = after[0] - before[0], after[1] - before[1]
        length = math.hypot(along_x, along_z) or 1.0
        # The side of the water that shows, here to the west or upwards.
        normal_x, normal_z = along_z / length, -along_x / length
        if normal_x > 0 or (normal_x == 0 and normal_z < 0):
            normal_x, normal_z = -normal_x, -normal_z
        if row:
            step = math.hypot(x - course[row - 1][0], z - course[row - 1][1])
            speed = math.sqrt(drop_speed ** 2 + 2 * pull * max(top - z, 0.0)) * 0.5 + 90
            seconds += step / speed
        pinch = 10 * math.sin(row * 0.7 + phases[0])
        for column in range(columns + 1):
            share = column / columns
            y = y0 + (y1 - y0) * share + pinch * (1 - 2 * share)
            bulge = sum(math.sin(y * rate + phases[index + 1] + row * 0.3)
                        for index, rate in enumerate((0.021, 0.047, 0.093))) / 3
            lift = bulge * ripple - back
            points.append((x + normal_x * lift, y, z + normal_z * lift))
            uvs.append((y / 256, -seconds))
    for row in range(len(course) - 1):
        for column in range(columns):
            a = row * (columns + 1) + column
            b, c, d = a + 1, a + columns + 1, a + columns + 2
            triangles += [(a, b, d), (a, d, c), (a, d, b), (a, c, d)]
    return points, uvs, triangles


def puffs(places, seed):
    """Soft quads at `places`, a list of (x, y, z, size), turned this way and
    that about the west, as (points, uvs, triangles)."""
    rng = random.Random(seed)
    points, uvs, triangles = [], [], []
    for x, y, z, size in places:
        turn = rng.uniform(-0.6, 0.6)
        across_x, across_y = math.sin(turn) * size / 2, math.cos(turn) * size / 2
        first = len(points)
        points += [(x - across_x, y - across_y, z), (x + across_x, y + across_y, z),
                   (x + across_x, y + across_y, z + size), (x - across_x, y - across_y, z + size)]
        uvs += [(0, 1), (1, 1), (1, 0), (0, 0)]
        triangles += [(first, first + 1, first + 2), (first, first + 2, first + 3),
                      (first + 2, first + 1, first), (first + 3, first + 2, first)]
    return points, uvs, triangles


class Level:
    """A level being built."""

    def __init__(self, name, bounds, rock, world, blocks=''):
        self.name = name
        self.rock = rock
        self.world = world
        self.blocks = blocks
        self.solids = [tuple(bounds)]
        self.spaces = []
        self.lines = []
        self.ops = []

    def carve(self, x0, y0, z0, x1, y1, z1, floor=None, wall=None, ceiling=None):
        """Cuts a space. A surface with no material of its own is rock."""
        box = (x0, y0, z0, x1, y1, z1)
        self.solids = [piece for solid in self.solids for piece in minus(solid, box)]
        wall = wall or self.rock
        self.spaces.append(Space(box, floor or wall, wall, ceiling or wall))
        return box

    def material_of(self, box, axis, high):
        """What the side of solid `box` shows: the space with most of it."""
        plane = box[axis + 3] if high else box[axis]
        others = [other for other in range(3) if other != axis]
        best, most = self.rock, 0
        for space in reversed(self.spaces):
            touching = space.box[axis] if high else space.box[axis + 3]
            if touching != plane:
                continue
            area = 1
            for other in others:
                area *= max(0, min(box[other + 3], space.box[other + 3]) - max(box[other], space.box[other]))
            if area > most:
                most = area
                if axis == 2:
                    best = space.floor if high else space.ceiling
                else:
                    best = space.wall
        return best

    def solid_text(self, box):
        """A carved solid, each side showing what it faces."""
        faces = []
        for axis, high in SIDES:
            normal = [0, 0, 0]
            normal[axis] = 1 if high else -1
            distance = box[axis + 3] if high else -box[axis]
            material = self.material_of(box, axis, high)
            faces.append(f'    face {{ normal = {value_text(normal)} distance = {distance:g} material = {value_text(material)} }}')
        return 'brush {\n' + '\n'.join(faces) + '\n}'

    def box(self, material, x0, y0, z0, x1, y1, z1, **fields):
        """A plain block of detail."""
        extra = fields_text(fields)
        self.lines.append(
            f'brush {{ material = {value_text(material)} cuboid = {value_text([x0, y0, z0, x1, y1, z1])} {extra} }}')

    def liquid(self, kind, material, x0, y0, z0, x1, y1, z1):
        """A body of liquid: `water`, `slime` or `lava`."""
        self.box(material, x0, y0, z0, x1, y1, z1, contents=kind)

    def entity(self, kind, origin=None, name=None, **fields):
        """A point entity."""
        fields = {'type': kind, 'origin': origin, **fields}
        label = f' {name}' if name else ''
        self.lines.append(f'entity{label} {{ {fields_text(fields)} }}')

    def moving(self, kind, boxes, material, name=None, **fields):
        """An entity made of brushes, such as a door."""
        label = f' {name}' if name else ''
        body = '\n'.join(
            f'    brush {{ material = {value_text(material)} cuboid = {value_text(list(box))} }}' for box in boxes)
        self.lines.append(f'entity{label} {{ type = {value_text(kind)} {fields_text(fields)}\n{body}\n}}')

    def mesh(self, source, at=(0, 0, 0), **fields):
        """A mesh from `content/meshes`, placed in the world."""
        fields = {'source': source, 'origin': at, **fields}
        self.lines.append(f'mesh {{ {fields_text(fields)} }}')

    def prefab(self, which, at, /, **params):
        """A prefab, placed by the platform's tools when the level's saved."""
        self.ops.append({'op': 'place_prefab', 'args': {'prefab': which, 'at': list(at), 'params': params}})

    def save(self):
        """Writes the map, then has the tools place its prefabs."""
        world = fields_text({'type': 'worldspawn', **self.world})
        text = ['format 1 map', '', f'# Written by levels/{self.name}.py, and best changed there.',
                f'entity world {{ {world} }}', self.blocks]
        text.extend(self.solid_text(box) for box in self.solids)
        text.extend(self.lines)
        path = os.path.join(ROOT, 'maps', f'{self.name}.emap')
        with open(path, 'w', newline='\n') as out:
            out.write('\n'.join(text) + '\n')
        batch = os.path.join(ROOT, 'levels', f'{self.name}.json')
        with open(batch, 'w', newline='\n') as out:
            json.dump(self.ops, out, indent=0)
        result = subprocess.run([TOOL, ROOT, path, '--batch', batch], capture_output=True, text=True)
        failures = [line for line in result.stdout.splitlines() if 'error' in line]
        print(f'{self.name}: {len(self.solids)} carved solids, {len(self.lines)} details, '
              f'{len(self.ops)} prefabs, {len(failures)} failures, exit {result.returncode}')
        for line in failures[:8] + result.stderr.splitlines()[:8]:
            print(' ', line[:200])
