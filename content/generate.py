"""Makes the test content's images and materials.

The test maps need art that isn't anyone else's, so this draws it. Run it from
the repository root. It needs nothing but Python, and writes `content/textures`
and `content/materials`.
"""
import math
import os
import random
import struct
import zlib

SIZE = 256
ROOT = os.path.dirname(os.path.abspath(__file__))
TEXTURES = os.path.join(ROOT, 'textures')
MATERIALS = os.path.join(ROOT, 'materials')
MESHES = os.path.join(ROOT, 'meshes')


def write_png(name, pixels, size=SIZE):
    """Writes RGBA rows as a PNG."""
    raw = bytearray()
    for y in range(size):
        raw.append(0)
        for x in range(size):
            raw.extend(pixels(x, y))
    def chunk(kind, data):
        body = kind + data
        return struct.pack('>I', len(data)) + body + struct.pack('>I', zlib.crc32(body))
    header = struct.pack('>IIBBBBB', size, size, 8, 6, 0, 0, 0)
    out = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', header) + chunk(b'IDAT', zlib.compress(bytes(raw), 9)) + chunk(b'IEND', b'')
    with open(os.path.join(TEXTURES, name + '.png'), 'wb') as f:
        f.write(out)


def clamp(value):
    return max(0, min(255, int(value)))


def noise_table(seed):
    rng = random.Random(seed)
    return [[rng.random() for _ in range(64)] for _ in range(64)]


def smooth(table, x, y, cells):
    """Wrapping value noise."""
    fx, fy = x / SIZE * cells, y / SIZE * cells
    ix, iy = int(fx), int(fy)
    tx, ty = fx - ix, fy - iy
    tx, ty = tx * tx * (3 - 2 * tx), ty * ty * (3 - 2 * ty)
    a = table[iy % cells][ix % cells]
    b = table[iy % cells][(ix + 1) % cells]
    c = table[(iy + 1) % cells][ix % cells]
    d = table[(iy + 1) % cells][(ix + 1) % cells]
    return (a * (1 - tx) + b * tx) * (1 - ty) + (c * (1 - tx) + d * tx) * ty


def fractal(table, x, y):
    return (smooth(table, x, y, 4) * 0.5 + smooth(table, x, y, 8) * 0.3
            + smooth(table, x, y, 16) * 0.15 + smooth(table, x, y, 32) * 0.05)


NOISE = noise_table(1)
NOISE2 = noise_table(2)


def brick_height(x, y):
    """Height of a running bond brick wall: 1 on bricks, 0 in the mortar."""
    row = y // 32
    bx = (x + (32 if row % 2 else 0)) % 64
    by = y % 32
    edge = min(bx, 64 - bx, by, 32 - by)
    return min(1.0, max(0.0, (edge - 2) / 3.0))


def height_normal(height):
    """A normal map from a height function."""
    def pixel(x, y):
        dx = height((x + 1) % SIZE, y) - height((x - 1) % SIZE, y)
        dy = height(x, (y + 1) % SIZE) - height(x, (y - 1) % SIZE)
        length = math.sqrt(dx * dx * 16 + dy * dy * 16 + 1)
        return (clamp((-dx * 4 / length * 0.5 + 0.5) * 255), clamp((-dy * 4 / length * 0.5 + 0.5) * 255), clamp((1 / length * 0.5 + 0.5) * 255), 255)
    return pixel


def brick_base(x, y):
    h = brick_height(x, y)
    n = fractal(NOISE, x, y)
    row, col = y // 32, ((x + (32 if (y // 32) % 2 else 0)) // 64) % 4
    tone = 0.8 + 0.2 * ((row * 7 + col * 13) % 5) / 4
    if h < 0.5:
        return (clamp(90 + n * 40), clamp(88 + n * 40), clamp(84 + n * 40), 255)
    return (clamp((130 + n * 70) * tone), clamp((62 + n * 40) * tone), clamp((48 + n * 30) * tone), 255)


def stone_base(x, y):
    n = fractal(NOISE, x, y)
    return (clamp(90 + n * 90), clamp(92 + n * 88), clamp(96 + n * 84), 255)


def stone_height(x, y):
    return fractal(NOISE, x, y)


def metal_base(x, y):
    n = fractal(NOISE2, x, y)
    seam = 1 if (x % 128 < 3 or y % 128 < 3) else 0
    bolt = 1 if (((x % 128) - 12) ** 2 + ((y % 128) - 12) ** 2) < 25 else 0
    v = 104 + n * 60 - seam * 50 + bolt * 50
    return (clamp(v), clamp(v * 1.02), clamp(v * 1.08), 255)


def metal_height(x, y):
    seam = 0.0 if (x % 128 < 3 or y % 128 < 3) else 1.0
    bolt = 0.5 if (((x % 128) - 12) ** 2 + ((y % 128) - 12) ** 2) < 25 else 0.0
    return min(1.0, seam * 0.6 + bolt)


def metal_specular(x, y):
    n = fractal(NOISE2, x, y)
    return (clamp(150 + n * 80), clamp(150 + n * 80), clamp(170 + n * 80), clamp(90 + n * 160))


def grass_base(x, y):
    n = fractal(NOISE2, x, y)
    return (clamp(50 + n * 50), clamp(100 + n * 90), clamp(40 + n * 30), 255)


def mask_blob(x, y):
    n = fractal(NOISE2, x, y)
    return (clamp((n - 0.3) * 600),) * 3 + (255,)


def glow_panel(x, y):
    on = 40 < x < 216 and 96 < y < 160
    return (clamp(255 if on else 0), clamp(190 if on else 0), clamp(60 if on else 0), 255)


def panel_base(x, y):
    n = fractal(NOISE, x, y)
    v = 50 + n * 30
    return (clamp(v), clamp(v), clamp(v * 1.1), 255)


def grate_base(x, y):
    hole = (x % 32 > 6) and (y % 32 > 6)
    return (70, 72, 78, 0 if hole else 255)


def glass_color(x, y):
    """Stained glass: leaded panes of colour."""
    colors = [(200, 40, 40), (40, 70, 200), (230, 190, 40), (40, 160, 80)]
    cx, cy = x // 32, y // 32
    lead = x % 32 < 3 or y % 32 < 3
    if lead:
        return (20, 20, 22, 255)
    r, g, b = colors[(cx * 3 + cy * 5 + (cx * cy)) % 4]
    return (r, g, b, 170)


def panel_sheet(x, y):
    """A hotspot sheet: a wide trim over two square panels."""
    border = 6
    if y < 64:
        edge = y < border or y >= 64 - border or x < border or x >= SIZE - border
        return (150, 130, 90, 255) if edge else (110, 90, 60, 255)
    half = x // 128
    local_x = x - half * 128
    local_y = y - 64
    edge = local_x < border or local_x >= 128 - border or local_y < border or local_y >= 192 - border
    if edge:
        return (70, 74, 84, 255)
    return (120, 124, 140, 255) if half == 0 else (90, 110, 120, 255)


def lettering(x, y):
    """A decal: a black stencilled X in a ring, on nothing."""
    dx, dy = x - SIZE / 2, y - SIZE / 2
    radius = math.hypot(dx, dy)
    ring = 96 < radius < 116
    cross_mark = radius < 96 and (abs(dx - dy) < 14 or abs(dx + dy) < 14)
    return (15, 15, 18, 235) if ring or cross_mark else (0, 0, 0, 0)


def flow_swirl(x, y):
    """A flow map turning around the image's middle."""
    dx, dy = (x - SIZE / 2) / (SIZE / 2), (y - SIZE / 2) / (SIZE / 2)
    return (clamp(128 - dy * 120), clamp(128 + dx * 120), 128, 255)


FONT = {
    'Q': ['01110', '10001', '10001', '10101', '10010', '01101'],
    'U': ['10001', '10001', '10001', '10001', '10001', '01110'],
    'A': ['01110', '10001', '10001', '11111', '10001', '10001'],
    'K': ['10001', '10010', '11100', '10010', '10001', '10001'],
    'E': ['11111', '10000', '11110', '10000', '10000', '11111'],
}


def stamp_letters(x, y):
    """Height for a stamp: letters cut into a flat surface."""
    word = 'QUAKE'
    scale = 8
    left = (SIZE - len(word) * 6 * scale) // 2
    top = (SIZE - 6 * scale) // 2
    cx, cy = (x - left) // scale, (y - top) // scale
    if 0 <= cy < 6 and 0 <= cx < len(word) * 6 and cx % 6 < 5:
        if FONT[word[cx // 6]][cy][cx % 6] == '1':
            return (0, 0, 0, 255)
    return (128, 128, 128, 255)


def stamp_bricks(x, y):
    """Height for a stamp: rows of bricks standing off a flat surface."""
    row = y // 32
    shift = 32 if row % 2 else 0
    mortar = y % 32 < 4 or (x + shift) % 64 < 4
    return (128, 128, 128, 255) if mortar else (255, 255, 255, 255)


def glass_rays(x, y):
    """The window's stencil: a few chunky panes, with wide frames to cut them."""
    colors = [(255, 70, 50), (70, 110, 255), (255, 210, 60), (70, 220, 120)]
    cx, cy = x // 64, y // 64
    if x % 64 < 24 or y % 64 < 24:
        return (0, 0, 0, 0)
    r, g, b = colors[(cx * 3 + cy * 5 + (cx * cy)) % 4]
    return (r, g, b, 255)


def glass_marks(x, y):
    # Red lines group the window's panes into boxes for occlusion, four across
    # and three down. They're kept apart from the glass, whose panes can be red.
    if x % 64 == 0 or y % 86 == 0:
        return (255, 0, 0, 255)
    return (255, 255, 255, 255)


def water_base(x, y):
    n = fractal(NOISE, x, y)
    return (clamp(20 + n * 40), clamp(70 + n * 60), clamp(110 + n * 80), 96)


def sphere_matcap(x, y):
    """A soft metal sphere of shine."""
    u, v = x / SIZE * 2 - 1, y / SIZE * 2 - 1
    d = u * u + v * v
    z = math.sqrt(max(0.0, 1 - d))
    key = max(0.0, u * -0.5 + v * -0.6 + z * 0.62) ** 12
    fill = 0.35 + 0.35 * z
    return (clamp((fill + key) * 200), clamp((fill + key) * 190), clamp((fill + key) * 170), 255)


def ramp(x, y):
    level = x / SIZE
    band = 0.25 if level < 0.3 else (0.6 if level < 0.65 else 1.0)
    return (clamp(band * 255), clamp(band * 245), clamp(band * 235), 255)


def detail(x, y):
    n = fractal(NOISE2, x * 4 % SIZE, y * 4 % SIZE)
    v = 128 + (n - 0.5) * 80
    return (clamp(v), clamp(v), clamp(v), 255)


def lava_base(x, y):
    n = fractal(NOISE, x, y)
    return (clamp(150 + n * 105), clamp(n * n * 160), clamp(10), 255)


def flame_sheet(x, y):
    """Four flame frames in a 2 by 2 sheet."""
    frame = (x // 128) + (y // 128) * 2
    u = (x % 128) / 128 * 2 - 1
    v = (y % 128) / 128
    wobble = math.sin(v * 9 + frame * 1.6) * 0.14 * (1 - v)
    width = (v ** 0.7) * 0.7
    inside = abs(u - wobble) < width
    heat = max(0.0, 1 - abs(u - wobble) / max(width, 0.01)) * v
    return (clamp(255 * min(1, heat * 3)), clamp(200 * heat * 1.4), clamp(60 * heat), clamp(255 * min(1, heat * 3)) if inside else 0)


def stretched(table, x, y, across, along):
    """Wrapping value noise with its own number of cells each way."""
    fx, fy = x / SIZE * across, y / SIZE * along
    ix, iy = int(fx), int(fy)
    tx, ty = fx - ix, fy - iy
    tx, ty = tx * tx * (3 - 2 * tx), ty * ty * (3 - 2 * ty)
    a = table[iy % along][ix % across]
    b = table[iy % along][(ix + 1) % across]
    c = table[(iy + 1) % along][ix % across]
    d = table[(iy + 1) % along][(ix + 1) % across]
    return (a * (1 - tx) + b * tx) * (1 - ty) + (c * (1 - tx) + d * tx) * ty


STREAKS = noise_table(7)


def foam_of(x, y):
    """How much white water there is, in streaks that run down the image."""
    streaks = (stretched(STREAKS, x, y, 32, 2) * 0.5 + stretched(NOISE, x, y, 64, 4) * 0.3
               + stretched(STREAKS, x, y, 16, 1) * 0.2)
    clumps = stretched(NOISE, x, y, 8, 4)
    return max(0.0, min(1.0, (streaks - 0.42) * 3.2 * (0.45 + clumps)))


def fall_base(x, y):
    """White water over clear, for the front of a fall."""
    foam = foam_of(x, y)
    return (clamp(70 + foam * 150), clamp(96 + foam * 134), clamp(108 + foam * 124), clamp(70 + foam * 175))


def fall_deep(x, y):
    """The body of a fall, behind its white water."""
    n = stretched(STREAKS, x, y, 16, 2) * 0.6 + stretched(NOISE, x, y, 32, 2) * 0.4
    return (clamp(24 + n * 40), clamp(44 + n * 52), clamp(54 + n * 58), 225)


def flow_fall(x, y):
    """Each thread of a fall has its own speed, all of them down the image."""
    speed = stretched(STREAKS, x, 0, 16, 1) * 0.7 + stretched(NOISE, x, 0, 32, 1) * 0.3
    return (128, clamp(128 + (speed - 0.5) * 240), 128, 255)


def flow_outward(x, y):
    """A flow map spreading from the image's middle."""
    dx, dy = (x - SIZE / 2) / (SIZE / 2), (y - SIZE / 2) / (SIZE / 2)
    return (clamp(128 - dx * 120), clamp(128 - dy * 120), 128, 255)


def edge_fade(x, y, wobble):
    """1 in the middle of the image, going to 0 by a ragged edge."""
    dx, dy = (x - SIZE / 2) / (SIZE / 2), (y - SIZE / 2) / (SIZE / 2)
    reach = math.sqrt(dx * dx + dy * dy) + (fractal(NOISE, x, y) - 0.5) * wobble
    return max(0.0, min(1.0, (0.88 - reach) * 3.0))


def fall_foam(x, y):
    """White water spreading on a pool, in a fine lace thinning towards the edge."""
    lace = 1 - abs(smooth(STREAKS, x, y, 32) - smooth(NOISE, x, y, 16)) * 5
    lace = max(0.0, min(1.0, lace * 1.2 - 0.3))
    fade = edge_fade(x, y, 0.5) ** 1.5
    return (clamp(150 + lace * 50), clamp(170 + lace * 46), clamp(178 + lace * 44), clamp(fade * (24 + lace * 130)))


def canal_water(x, y):
    """Dark water, for somewhere with little light to give it colour."""
    n = fractal(NOISE, x, y)
    return (clamp(8 + n * 16), clamp(26 + n * 26), clamp(30 + n * 30), 170)


def fall_mist(x, y):
    """A soft cloud of spray."""
    fade = edge_fade(x, y, 0.9) ** 1.6
    return (178, 198, 208, clamp(fade * (12 + fractal(NOISE, x, y) * 72)))


IMAGES = {
    'fall': fall_base,
    'fall_deep': fall_deep,
    'flow_fall': flow_fall,
    'flow_outward': flow_outward,
    'fall_foam': fall_foam,
    'fall_mist': fall_mist,
    'canal_water': canal_water,
    'glass_marks': glass_marks,
    'stamp_letters': stamp_letters,
    'stamp_bricks': stamp_bricks,
    'panel_sheet': panel_sheet,
    'lettering': lettering,
    'flow_swirl': flow_swirl,
    'glass_rays': glass_rays,
    'brick': brick_base,
    'brick_normal': height_normal(brick_height),
    'brick_height': lambda x, y: (clamp(brick_height(x, y) * 255),) * 3 + (255,),
    'stone': stone_base,
    'stone_normal': height_normal(stone_height),
    'stone_height': lambda x, y: (clamp(stone_height(x, y) * 255),) * 3 + (255,),
    'metal': metal_base,
    'metal_normal': height_normal(metal_height),
    'metal_specular': metal_specular,
    'grass': grass_base,
    'blob_mask': mask_blob,
    'panel': panel_base,
    'panel_glow': glow_panel,
    'grate': grate_base,
    'glass': glass_color,
    'water': water_base,
    'lava': lava_base,
    'matcap': sphere_matcap,
    'cel_ramp': ramp,
    'detail': detail,
    'flame': flame_sheet,
}

MATERIALS_TEXT = {
    'canal_water': 'base = textures/canal_water.png\n    kind = liquid\n    alpha = blend\n    shading = unlit\n    scroll = 0.03 0.02',
    'fall': 'base = textures/fall.png\n    alpha = blend\n    shading = unlit\n    scroll = 0 1.1\n    flow = textures/flow_fall.png\n    flow_strength = 0.5\n    flow_speed = 0.8',
    'fall_deep': 'base = textures/fall_deep.png\n    shading = unlit\n    scroll = 0 0.8',
    'fall_foam': 'base = textures/fall_foam.png\n    alpha = blend\n    shading = unlit\n    flow = textures/flow_outward.png\n    flow_strength = 0.1\n    flow_speed = 0.35',
    'fall_mist': 'base = textures/fall_mist.png\n    alpha = blend\n    shading = unlit\n    flow = textures/flow_outward.png\n    flow_strength = 0.08\n    flow_speed = 0.25',
    'brick': 'base = textures/brick.png\n    normal = textures/brick_normal.png\n    height = textures/brick_height.png\n    parallax = 0.025',
    'stone': 'base = textures/stone.png\n    normal = textures/stone_normal.png\n    hex = true\n    detail = textures/detail.png',
    'metal': 'base = textures/metal.png\n    normal = textures/metal_normal.png\n    specular = textures/metal_specular.png\n    gloss = 48',
    'mossy_stone': 'base = textures/stone.png\n    normal = textures/stone_normal.png\n    height = textures/stone_height.png\n    layer { base = textures/grass.png mask = textures/blob_mask.png }',
    'light_panel': 'base = textures/panel.png\n    emissive = textures/panel_glow.png\n    emissive_strength = 1.5',
    'grate': 'base = textures/grate.png\n    alpha = mask',
    'stained_glass': 'base = textures/glass.png\n    alpha = blend\n    tint = textures/glass.png\n    shading = unlit',
    'cel_stone': 'base = textures/stone.png\n    shading = cel\n    ramp = textures/cel_ramp.png',
    'shiny': 'base = textures/metal.png\n    shading = matcap\n    matcap = textures/matcap.png',
    'triplanar_rock': 'base = textures/stone.png\n    triplanar = true',
    'conveyor': 'base = textures/metal.png\n    scroll = 0 0.5',
    'torch_flame': 'base = textures/flame.png\n    alpha = blend\n    shading = unlit\n    flipbook = 2 2 10',
    'glass_marks': 'base = textures/glass_marks.png',
    'stamp_letters': 'base = textures/stamp_letters.png',
    'stamp_bricks': 'base = textures/stamp_bricks.png',
    'lettering': 'base = textures/lettering.png\n    alpha = blend',
    'painted_stone': 'base = textures/stone.png\n    height = textures/stone_height.png\n    layer { base = textures/grass.png mask = paint }',
    'polished': 'base = textures/metal.png\n    reflect = 0.7',
    'swirl': 'base = textures/lava.png\n    flow = textures/flow_swirl.png\n    flow_strength = 0.2\n    shading = unlit',
    'panel_sheet': 'base = textures/panel_sheet.png\n    hotspots = 0 0 1 0.25  0 0.25 0.5 1  0.5 0.25 1 1',
    'glass_rays': 'base = textures/glass_rays.png',
    'hidden': 'base = textures/grate.png\n    kind = hidden',
    'slime': 'base = textures/water.png\n    kind = liquid',
    'sky': 'base = textures/water.png\n    kind = sky',
    'water': 'base = textures/water.png\n    kind = liquid\n    alpha = blend\n    shading = unlit\n    scroll = 0.05 0.03',
    'lava': 'base = textures/lava.png\n    kind = liquid',
}


def ring(sides, radius, z):
    return [(radius * math.cos(i * 2 * math.pi / sides),
             radius * math.sin(i * 2 * math.pi / sides), z) for i in range(sides)]


def lathe(name, material, rings, sides=12):
    '''A part turned from (radius, height) pairs, closed at each end.'''
    points, uvs, triangles = [], [], []
    for row, (radius, z) in enumerate(rings):
        for i, point in enumerate(ring(sides, radius, z)):
            points.append(point)
            uvs.append((i / sides, row / (len(rings) - 1)))
    for row in range(len(rings) - 1):
        for i in range(sides):
            a = row * sides + i
            b = row * sides + (i + 1) % sides
            c = a + sides
            d = b + sides
            triangles += [(a, b, d), (a, d, c)]
    for row, flip in ((0, True), (len(rings) - 1, False)):
        middle = len(points)
        points.append((0.0, 0.0, rings[row][1]))
        uvs.append((0.5, 0.5))
        for i in range(sides):
            a = row * sides + i
            b = row * sides + (i + 1) % sides
            triangles.append((a, middle, b) if flip else (a, b, middle))
    return (name, material, points, uvs, triangles)


def numbers(rows):
    return ' '.join('%g' % value for row in rows for value in row)


def write_mesh(name, parts, hulls):
    lines = ['format 1 mesh', '']
    for part, material, points, uvs, triangles in parts:
        lines += ['part %s {' % part,
                  '    material = %s' % material,
                  '    points = ' + numbers(points),
                  '    uvs = ' + numbers(uvs),
                  '    triangles = ' + numbers(triangles),
                  '}']
    for hull in hulls:
        lines.append('hull { cuboid = ' + numbers([hull]) + ' }')
    with open(os.path.join(MESHES, name + '.emesh'), 'w', encoding='utf-8', newline='\n') as f:
        f.write('\n'.join(lines) + '\n')


MESH_PARTS = {
    'statue': ([
        lathe('plinth', 'stone', [(40, 0), (40, 16), (30, 24)]),
        lathe('body', 'metal', [(22, 24), (26, 60), (18, 96), (20, 112)]),
        lathe('head', 'shiny', [(6, 112), (14, 122), (15, 134), (8, 146), (1, 150)]),
    ], [(-40, -40, 0, 40, 40, 24), (-24, -24, 24, 24, 24, 150)]),
    'barrel': ([
        lathe('staves', 'metal', [(14, 0), (18, 16), (19, 28), (18, 40), (14, 56)]),
    ], []),
}


def main():
    os.makedirs(TEXTURES, exist_ok=True)
    os.makedirs(MATERIALS, exist_ok=True)
    for name, pixels in IMAGES.items():
        write_png(name, pixels)
    os.makedirs(MESHES, exist_ok=True)
    for name, (parts, hulls) in MESH_PARTS.items():
        write_mesh(name, parts, hulls)
    for name, body in MATERIALS_TEXT.items():
        with open(os.path.join(MATERIALS, name + '.emat'), 'w', encoding='utf-8', newline='\n') as f:
            f.write('format 1 material\n\nsurface {\n    ' + body + '\n}\n')


main()
