#!/usr/bin/env python3
"""Crop each window screenshot to its window: the windows' panel colour, the largest patch of it
that isn't the step list at the top right, a margin round it.  manual/crop.py de"""
import sys
from collections import deque
from pathlib import Path
from PIL import Image

IMG = Path(__file__).resolve().parent / "build" / (sys.argv[1] if len(sys.argv) > 1 else "de") / "img"
PANEL = (64, 48, 30)
STEP = 4  # look at every 4th pixel
SHOTS = ["build", "build_mid", "options", "saves", "help", "hq", "sawmill", "guardhouse", "main", "inventory",
         "distribution", "transport", "tools", "military", "farm", "buildings", "map", "mine", "stats",
         "stats_wares", "sea", "shipyard", "briefing", "editor"]


def window(im):
    w, h = im.size
    gw, gh = w // STEP, (h - 48) // STEP
    px = im.load()
    on = [[max(abs(a - b) for a, b in zip(px[x * STEP, y * STEP], PANEL)) <= 3 for x in range(gw)] for y in range(gh)]
    seen = [[False] * gw for _ in range(gh)]
    best = None
    for y0 in range(gh):
        for x0 in range(gw):
            if not on[y0][x0] or seen[y0][x0]:
                continue
            q, n, box = deque([(x0, y0)]), 0, [x0, y0, x0, y0]
            seen[y0][x0] = True
            while q:
                x, y = q.popleft()
                n += 1
                box = [min(box[0], x), min(box[1], y), max(box[2], x), max(box[3], y)]
                # (gaps of a cell or two inside a window: text, icons)
                for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1), (2, 0), (-2, 0), (0, 2), (0, -2), (3, 0), (0, 3), (-3, 0), (0, -3)):
                    nx, ny = x + dx, y + dy
                    if 0 <= nx < gw and 0 <= ny < gh and on[ny][nx] and not seen[ny][nx]:
                        seen[ny][nx] = True
                        q.append((nx, ny))
            steps_list = box[0] * STEP > 990 and box[3] * STEP < 320
            if not steps_list and (best is None or n > best[0]):
                best = (n, box)
    if not best:
        return None
    m = 10
    b = best[1]
    # (the title bar above the panel is darker: take it too)
    return (max(b[0] * STEP - m, 0), max(b[1] * STEP - m - 34, 0), min(b[2] * STEP + m + STEP, w), min(b[3] * STEP + m + STEP, h))


for name in SHOTS:
    f = IMG / f"{name}.png"
    if not f.exists():
        print("missing", name)
        continue
    im = Image.open(f).convert("RGB")
    box = window(im)
    if box:
        im.crop(box).save(IMG / f"{name}_win.png")
    print(name, box)
