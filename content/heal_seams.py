"""Blends the seams of pictures that should repeat and don't quite.

usage: heal_seams.py [--check] prefix ...

The texture packs came with a thin dark line along each picture's last row and
column, and the two sides of a repeat mostly line up but don't blend. For each
picture that repeats freely:

1. The line is trimmed off.
2. What's left of the difference in tone between opposite edges is spread evenly
   across the picture, so nothing changes suddenly at the edge.
3. The picture is overlapped onto itself and joined along the path where the two
   sides match best, which follows mortar and cracks, with a short crossfade of
   real picture over it. Nothing's mirrored: a mirror reads as a strip of inkblots.

What's overlapped and trimmed leaves a picture of 232, which repeats that often.
It isn't scaled back to 256, as that makes all that's in it bigger and softer. 232
halves evenly three times, for its mips.

Only kinds in `BLENDED` are treated so. A pattern has to stay where it is, or a
checked floor's squares come out uneven and a carved trim is cut through its
figures, so kinds in `BRIDGED` only have the rows of the line drawn again, as a
straight blend from one side of it to the other. Others are shown whole, as a
door is, and are left alone.

Run this once, on pictures as they were delivered. Run `engine-tool`
`derive_maps` afterwards, as the maps come from the picture.
"""
import os
import sys

import numpy as np
from PIL import Image

ROOT = os.path.dirname(os.path.abspath(__file__))
TEXTURES = os.path.join(ROOT, 'textures')
BLENDED = ('_floor_flags.png', '_liquid.png', '_rock.png', '_wall_blocks.png', '_wall_brick.png', '_wall_damp.png',
           '_wood_planks.png')
# Two kinds of planks have a joint at the repeat, with rivets either side of it,
# which an overlap would squeeze together.
BRIDGED = ('_ceiling_coffer.png', '_floor_tiles.png', '_metal_plate.png', '_trim_carved.png', '_trim_metal.png',
           'uc01_blight_sewers_wood_planks.png', 'uc03_apothecary_wood_planks.png')
SIDEWAYS = ('_trim_carved.png', '_trim_metal.png', 'uc01_blight_sewers_wood_planks.png',
            'uc03_apothecary_wood_planks.png')
LINE = 3
OVERLAP = 18
FEATHER = 5
TONE_ROWS = 8
TONE_BLUR = 24


def even_tone(picture):
    """Spreads the difference between the top and bottom edges down the picture."""
    height = picture.shape[0]
    gap = picture[-TONE_ROWS:].mean(axis=0) - picture[:TONE_ROWS].mean(axis=0)
    wrapped = np.concatenate([gap[-TONE_BLUR:], gap, gap[:TONE_BLUR]])
    kernel = np.ones(TONE_BLUR) / TONE_BLUR
    smooth = np.stack([np.convolve(wrapped[:, part], kernel, 'same') for part in range(gap.shape[1])], axis=1)
    smooth = smooth[TONE_BLUR:-TONE_BLUR]
    ramp = (np.arange(height) / (height - 1) - 0.5)[:, None, None]
    return picture - ramp * smooth[None]


def best_path(error):
    """The column to join at on each row, cheapest overall, ending where it starts
    so that the repeat the other way isn't stepped."""
    height, width = error.shape
    best_cost, best = None, None
    for start in range(FEATHER, width - FEATHER, 2):
        cost = error.copy()
        cost[0, :] = np.inf
        cost[0, start] = error[0, start]
        back = np.zeros((height, width), dtype=np.int32)
        for y in range(1, height):
            before = np.stack([np.r_[np.inf, cost[y - 1, :-1]], cost[y - 1], np.r_[cost[y - 1, 1:], np.inf]])
            back[y] = before.argmin(axis=0) - 1
            cost[y] += before.min(axis=0)
        if best_cost is None or cost[height - 1, start] < best_cost:
            path = np.zeros(height, dtype=np.int32)
            x = start
            for y in range(height - 1, -1, -1):
                path[y] = x
                x += back[y, x]
            best_cost, best = cost[height - 1, start], path
    return best


def join_across(picture):
    """`picture`, narrower by `OVERLAP`, with its right edge running on into its left."""
    width = picture.shape[1]
    right, left = picture[:, width - OVERLAP:], picture[:, :OVERLAP]
    path = best_path(((right - left) ** 2).sum(axis=2))
    share = np.clip((np.arange(OVERLAP)[None, :] - path[:, None]) / (2 * FEATHER) + 0.5, 0, 1)
    share = (share * share * (3 - 2 * share))[..., None]
    return np.concatenate([right * (1 - share) + left * share, picture[:, OVERLAP:width - OVERLAP]], axis=1)


def blended(image):
    """`image` with both its repeats blended."""
    picture = np.asarray(image.convert('RGB'), dtype=np.float64)[LINE:-LINE, LINE:-LINE]
    turned = (1, 0, 2)
    picture = even_tone(even_tone(picture).transpose(turned)).transpose(turned)
    picture = join_across(join_across(picture).transpose(turned)).transpose(turned)
    return Image.fromarray(picture.clip(0, 255).round().astype(np.uint8))


def bridge_rows(picture):
    """The rows by the top and bottom edges, drawn again as a blend across them."""
    out = picture.copy()
    height = picture.shape[0]
    before, after = picture[height - 1 - LINE], picture[LINE]
    steps = 2 * LINE + 1
    for step in range(1, steps):
        share = step / steps
        out[(height - 1 - LINE + step) % height] = before * (1 - share) + after * share
    return out


def bridged(image, down=True):
    """`image` with the line along its edges gone, and all else where it was. Bands
    that run sideways, as a trim's and a plank's do, mustn't run together, so those
    are only mended across."""
    picture = np.asarray(image.convert('RGB'), dtype=np.float64)
    if down:
        picture = bridge_rows(picture)
    turned = (1, 0, 2)
    picture = bridge_rows(picture.transpose(turned)).transpose(turned)
    return Image.fromarray(picture.round().astype(np.uint8))


def main():
    check = '--check' in sys.argv
    prefixes = tuple(arg for arg in sys.argv[1:] if arg != '--check')
    for name in sorted(os.listdir(TEXTURES)):
        if not name.startswith(prefixes) or not name.endswith(BLENDED + BRIDGED):
            continue
        print(name)
        if not check:
            path = os.path.join(TEXTURES, name)
            with Image.open(path) as opened:
                image = opened.convert('RGB')
            if name.endswith(BRIDGED):
                bridged(image, not name.endswith(SIDEWAYS)).save(path)
            else:
                blended(image).save(path)


if __name__ == '__main__':
    main()
