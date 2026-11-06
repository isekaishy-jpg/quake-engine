"""Mends the seams of pictures that should repeat and don't quite.

usage: heal_seams.py [--check] prefix ...

A jump between a picture's last row and its first is a seam only if it stands
out: a brick wall has a jump at every line of mortar, and one more at the edge
is what it should have. So a picture is mended only where the jump over its edge
is bigger than any inside it. Rows or columns near the edge then fade into their
mirror over the edge, which meets in the middle and leaves the rest alone.

That can't tell a seam from a line that's meant to fall on the edge, as a checked
floor's does, so it only looks at what has no such lines.

Run `engine-tool` `derive_maps` afterwards, as the maps come from the picture.
"""
import os
import sys

from PIL import Image

ROOT = os.path.dirname(os.path.abspath(__file__))
TEXTURES = os.path.join(ROOT, 'textures')
BAND = 10
UNSHAPED = ('_liquid.png', '_rock.png', '_wall_damp.png', '_metal_plate.png')


def row_jumps(pixels, width, height):
    """How far each row is from the row after it, wrapping, on average."""
    jumps = []
    for y in range(height):
        below = (y + 1) % height
        total = 0
        for x in range(width):
            total += sum(abs(a - b) for a, b in zip(pixels[x, y][:3], pixels[x, below][:3]))
        jumps.append(total / width)
    return jumps


def has_seam(image):
    """Whether the jump between the last row and the first stands out."""
    width, height = image.size
    jumps = row_jumps(image.load(), width, height)
    return jumps[-1] > max(jumps[:-1])


def heal_rows(image):
    """Fades the rows by the top and bottom edges into each other."""
    width, height = image.size
    source = image.copy().load()
    pixels = image.load()
    for step in range(BAND):
        keep = 0.5 + 0.5 * (step + 0.5) / BAND
        for y, other in ((step, height - 1 - step), (height - 1 - step, step)):
            for x in range(width):
                pixels[x, y] = tuple(round(a * keep + b * (1 - keep)) for a, b in zip(source[x, y], source[x, other]))


def main():
    check = '--check' in sys.argv
    prefixes = tuple(arg for arg in sys.argv[1:] if arg != '--check')
    for name in sorted(os.listdir(TEXTURES)):
        if not name.startswith(prefixes) or not name.endswith(UNSHAPED):
            continue
        path = os.path.join(TEXTURES, name)
        with Image.open(path) as opened:
            image = opened.convert('RGBA')
        down = has_seam(image)
        turned = image.transpose(Image.Transpose.TRANSPOSE)
        across = has_seam(turned)
        if not (down or across):
            continue
        print(name, 'rows' if down else '', 'columns' if across else '')
        if check:
            continue
        if down:
            heal_rows(image)
        if across:
            turned = image.transpose(Image.Transpose.TRANSPOSE)
            heal_rows(turned)
            image = turned.transpose(Image.Transpose.TRANSPOSE)
        image.save(path)


main()
