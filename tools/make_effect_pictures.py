"""Makes the pictures the effects in content/effects start from: a puff, a spark, a ring, a star,
and sheets of smoke and flame. They're made from noise here, and owe nothing to anything else.

    python tools/make_effect_pictures.py
"""
import math
import os

import numpy as np
from PIL import Image

OUT = os.path.join('content', 'textures')
SIDE = 256


def noise(side, cells, seed):
    """Smooth noise from 0 to 1 that repeats, from a grid of `cells` random values."""
    grid = np.random.default_rng(seed).random((cells, cells))
    at = np.arange(side) / side * cells
    low = np.floor(at).astype(int) % cells
    high = (low + 1) % cells
    t = at - np.floor(at)
    t = t * t * (3 - 2 * t)
    rows = grid[low][:, :] * (1 - t)[:, None] + grid[high][:, :] * t[:, None]
    return rows[:, low] * (1 - t)[None, :] + rows[:, high] * t[None, :]


def clouds(side, seed, octaves=5, first=4):
    """Layers of noise, each finer and fainter than the last."""
    total = np.zeros((side, side))
    weight = 1.0
    weights = 0.0
    for octave in range(octaves):
        total += noise(side, first * 2 ** octave, seed + octave) * weight
        weights += weight
        weight *= 0.5
    return total / weights


def distance(side):
    """How far each pixel is from the middle, with 1 at the edge."""
    at = (np.arange(side) + 0.5) / side * 2 - 1
    return np.sqrt(at[:, None] ** 2 + at[None, :] ** 2)


def smooth(low, high, value):
    t = np.clip((value - low) / (high - low), 0, 1)
    return t * t * (3 - 2 * t)


def save(name, color, alpha):
    rgba = np.dstack([color, alpha])
    picture = Image.fromarray((np.clip(rgba, 0, 1) * 255).astype(np.uint8), 'RGBA')
    picture.save(os.path.join(OUT, name + '.png'))


def white(side):
    return np.ones((side, side, 3))


def puff(side, seed, grown=1.0, thin=0.0):
    """A soft ball of smoke, `grown` as far as it's going, and `thin` as far as it's gone."""
    edge = distance(side) / max(grown, 0.05)
    body = clouds(side, seed)
    shape = 1 - smooth(0.35, 1.0, edge + (body - 0.5) * 0.9)
    return shape * smooth(0.25 + thin * 0.6, 0.75 + thin * 0.3, body + 0.25) * (1 - thin * 0.7)


def sheet(frames, side, make):
    """`frames` by `frames` pictures from `make`, as one."""
    rows = []
    for y in range(frames):
        rows.append(np.hstack([make(y * frames + x, frames * frames) for x in range(frames)]))
    return np.vstack(rows)


def flame(number, count, side):
    """Flame that licks up and thins out."""
    at = number / count
    x = (np.arange(side) + 0.5) / side * 2 - 1
    y = (np.arange(side) + 0.5) / side
    body = clouds(side, 40 + number, 4, 3)
    width = (1 - y[:, None]) * (0.9 - at * 0.4) + 0.05
    upright = 1 - smooth(0.2, 1.0, np.abs(x)[None, :] / np.maximum(y[:, None] * 0.9 + 0.1, 0.05))
    heat = upright * smooth(0.0, 0.5, y[:, None]) * (0.4 + body * 1.1) * (1 - at * 0.6)
    del width
    return np.clip(heat, 0, 1)


def main():
    os.makedirs(OUT, exist_ok=True)
    save('fx_puff', white(SIDE), puff(SIDE, 3))
    edge = distance(SIDE)
    glow = np.exp(-edge * edge * 9)
    save('fx_glow', white(SIDE), glow)
    at = (np.arange(SIDE) + 0.5) / SIDE * 2 - 1
    streak = np.exp(-(at[:, None] ** 2) * 60) * np.exp(-(at[None, :] ** 2) * 2) + glow * 0.3
    save('fx_spark', white(SIDE), np.clip(streak.T, 0, 1))
    ring = np.exp(-((edge - 0.72) ** 2) * 180)
    save('fx_ring', white(SIDE), ring)
    angle = np.arctan2(at[:, None], at[None, :])
    rays = np.abs(np.cos(angle * 3)) ** 40 + np.abs(np.sin(angle * 3)) ** 40
    star = np.clip(rays * np.exp(-edge * 3.5) * 1.6 + glow * 0.7, 0, 1)
    save('fx_star', white(SIDE), star)
    runes = np.zeros((SIDE, SIDE))
    for turn in range(24):
        spoke = turn / 24 * math.tau
        wobble = 0.03 * math.sin(turn * 2.3)
        runes += np.exp(-((edge - 0.8 - wobble) ** 2) * 900) * (np.cos(angle - spoke) > 0.995)
    save('fx_circle', white(SIDE), np.clip(ring + np.exp(-((edge - 0.5) ** 2) * 500) * 0.8 + runes, 0, 1))
    frames = 4
    cell = 128
    smoke = sheet(frames, cell, lambda number, count: puff(cell, 11 + number // 2, 0.45 + 0.55 * number / count, number / count))
    save('fx_smoke_sheet', white(cell * frames), smoke)
    fire = sheet(frames, cell, lambda number, count: flame(number, count, cell))
    save('fx_flame_sheet', white(cell * frames), fire)


if __name__ == '__main__':
    main()
