"""Turn generated building renders (an AI image tool, green background) into game sprites.

uv run --no-project --with pillow --with numpy python tools/art/post.py <the folder of generated pictures> assets/art/buildings

- chroma key on #00FF00 with despill, soft 1px edge
- green enclosed by the building (a mill's lattice sails, a pen's floor) is see-through as well
  (it was a dark interior colour, for open doors: the sails came out as dark slabs)
- crop, scale to 1.5x the size class width (as prerender.py: crisp up to zoom 3), write PNG +
  manifest.json (the size on the map, and how the picture was made)
"""
import json, sys
from pathlib import Path
import numpy as np
from PIL import Image
from collections import deque

WIDTH = {"hut": 84, "house": 112, "castle": 156, "mine": 84}
HIRES = 1.5
SIZE = {
    "headquarters": "castle", "fortress": "castle", "farm": "castle", "pigfarm": "castle", "donkeybreeder": "castle", "harbor": "castle",
    "granitemine": "mine", "coalmine": "mine", "ironmine": "mine", "goldmine": "mine",
    "barracks": "hut", "guardhouse": "hut", "lookouttower": "hut", "woodcutter": "hut", "fishery": "hut", "quarry": "hut",
    "forester": "hut", "hunter": "hut", "well": "hut",
    # the Roman campaign's holy places (docs/CAMPAIGN-ROME.md)
    "juno_capitol": "castle", "mars": "castle", "carthage_temple": "castle",
    "concordia": "hut", "juno_veii": "hut",
}

def process(src: Path, dst: Path, key: str):
    pic = Image.open(src)
    if pic.mode in ("RGBA", "LA") or "transparency" in pic.info:
        # some models answer with a transparent background instead of the green: lay it on green
        pic = pic.convert("RGBA")
        green = Image.new("RGBA", pic.size, (0, 255, 0, 255))
        pic = Image.alpha_composite(green, pic)
    im = np.asarray(pic.convert("RGB")).astype(np.float32)
    r, g, b = im[..., 0], im[..., 1], im[..., 2]
    # greenness: how much green exceeds the other channels
    gx = g - np.maximum(r, b)
    bg = (gx > 60) & (g > 120)
    h, w = bg.shape
    # flood fill the background from the border; enclosed green is interior
    outside = np.zeros_like(bg)
    q = deque()
    for x in range(w):
        for y in (0, h - 1):
            if bg[y, x]:
                outside[y, x] = True; q.append((y, x))
    for y in range(h):
        for x in (0, w - 1):
            if bg[y, x] and not outside[y, x]:
                outside[y, x] = True; q.append((y, x))
    while q:
        y, x = q.popleft()
        for dy, dx in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            ny, nx = y + dy, x + dx
            if 0 <= ny < h and 0 <= nx < w and bg[ny, nx] and not outside[ny, nx]:
                outside[ny, nx] = True; q.append((ny, nx))
    inside_green = bg & ~outside
    outside = outside | inside_green
    inside_green = np.zeros_like(bg)
    rgb = im.copy()
    # despill: clamp green to max(r, b) where green dominates near edges
    spill = (rgb[..., 1] > np.maximum(rgb[..., 0], rgb[..., 2])) & ~inside_green
    rgb[..., 1] = np.where(spill, np.maximum(rgb[..., 0], rgb[..., 2]) * 0.6 + rgb[..., 1] * 0.4, rgb[..., 1])
    alpha = np.where(outside, 0, 255).astype(np.float32)
    # soften: partial alpha for weakly green pixels at the border
    soft = (~outside) & (gx > 20) & ~inside_green
    alpha[soft] = np.clip(255 - (gx[soft] - 20) * 4, 0, 255)
    out = np.dstack([np.clip(rgb, 0, 255), alpha]).astype(np.uint8)
    ys, xs = np.where(alpha > 10)
    y0, y1, x0, x1 = ys.min(), ys.max() + 1, xs.min(), xs.max() + 1
    img = Image.fromarray(out[y0:y1, x0:x1], "RGBA")
    tw = WIDTH[SIZE.get(key, "house")]
    th = round(img.height * tw / img.width)
    img = img.resize((round(tw * HIRES), round(th * HIRES)), Image.LANCZOS)
    img.save(dst / f"{key}.png")
    # anchor: the building node sits a little above the lowest point of the silhouette
    return {"w": tw, "h": th, "ax": tw / 2, "ay": th - th * 0.14, "source": "prompt"}

def main():
    src, dst = Path(sys.argv[1]), Path(sys.argv[2])
    dst.mkdir(parents=True, exist_ok=True)
    manifest = {}
    mf = dst / "manifest.json"
    if mf.exists():
        manifest = json.loads(mf.read_text())
    for p in sorted(src.glob("*.png")):
        key = p.stem
        manifest[key] = process(p, dst, key)
        print(key, manifest[key])
    mf.write_text(json.dumps(manifest, indent=1, sort_keys=True))

if __name__ == "__main__":
    main()
