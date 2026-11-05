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


def water_base(x, y):
    n = fractal(NOISE, x, y)
    return (clamp(20 + n * 40), clamp(70 + n * 60), clamp(110 + n * 80), 255)


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


IMAGES = {
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
    'brick': 'base = textures/brick.png\n    normal = textures/brick_normal.png\n    height = textures/brick_height.png\n    parallax = 0.03',
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
    'water': 'base = textures/water.png\n    kind = liquid',
    'lava': 'base = textures/lava.png\n    kind = liquid',
}


def main():
    os.makedirs(TEXTURES, exist_ok=True)
    os.makedirs(MATERIALS, exist_ok=True)
    for name, pixels in IMAGES.items():
        write_png(name, pixels)
    for name, body in MATERIALS_TEXT.items():
        with open(os.path.join(MATERIALS, name + '.emat'), 'w', encoding='utf-8', newline='\n') as f:
            f.write('format 1 material\n\nsurface {\n    ' + body + '\n}\n')


main()
