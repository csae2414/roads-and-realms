"""Turn generated ship pictures (transparent background) into the game's ship sprites.

uv run --no-project --with pillow --with numpy python tools/art/post_ship.py <pictures> <marks.json> assets/art/ships

marks.json, per picture key, in fractions of the generated picture:
  "water": [[x0, y0], [x1, y1]]  the waterline, a straight cut: what lies below it is under water
  "sail":  four corners of the main sail (top left, top right, bottom right, bottom left)
  "mast":  the main masthead (where the owner's pennant flies)
(the manifest also keeps the waterline's two ends, "keel": the stern and the bow)

- cut below the waterline with a soft edge (the hull's lower part showed as a keel in the air)
- crop, scale to 1.5x the ship's width on the map, write PNG + manifest.json: the size on the
  map, the anchor (the waterline's middle) and the sail and masthead in the cropped picture
"""
import json, sys
from pathlib import Path
import numpy as np
from PIL import Image

WIDTH = 150
HIRES = 1.5
FADE = 0.012  # the soft edge above the waterline, a share of the picture's height


def process(src: Path, dst: Path, key: str, mk: dict) -> dict:
    img = Image.open(src).convert("RGBA")
    a = np.asarray(img).astype(np.float32)
    h, w = a.shape[:2]
    (x0, y0), (x1, y1) = mk["water"]
    xs = np.arange(w) / w
    line = (y0 + (y1 - y0) * (xs - x0) / (x1 - x0)) * h  # the waterline's row in each column
    rows = np.arange(h)[:, None]
    keep = np.clip((line[None, :] - rows) / (FADE * h), 0.0, 1.0)
    a[..., 3] *= keep
    ys, xs_ = np.where(a[..., 3] > 10)
    cy0, cy1, cx0, cx1 = ys.min(), ys.max() + 1, xs_.min(), xs_.max() + 1
    crop = Image.fromarray(a[cy0:cy1, cx0:cx1].astype(np.uint8), "RGBA")
    tw = WIDTH
    th = round(crop.height * tw / crop.width)
    crop.resize((round(tw * HIRES), round(th * HIRES)), Image.LANCZOS).save(dst / f"{key}.png")
    # fractions of the generated picture -> fractions of the cropped one
    fx = lambda x: (x * w - cx0) / (cx1 - cx0)
    fy = lambda y: (y * h - cy0) / (cy1 - cy0)
    mx = (x0 + x1) / 2
    my = (y0 + (y1 - y0) * (mx - x0) / (x1 - x0))
    return {
        "w": tw, "h": th, "ax": round(fx(mx) * tw, 2), "ay": round(fy(my) * th, 2),
        "sail": [[round(fx(x), 4), round(fy(y), 4)] for x, y in mk["sail"]],
        "mast": [round(fx(mk["mast"][0]), 4), round(fy(mk["mast"][1]), 4)],
        # the waterline's two ends (left, right): stern and bow, for the wake and the bow's foam
        "keel": [[round(fx(x0), 4), round(fy(y0), 4)], [round(fx(x1), 4), round(fy(y1), 4)]],
        "source": "prompt",
    }


def main():
    src, marks, dst = Path(sys.argv[1]), json.loads(Path(sys.argv[2]).read_text()), Path(sys.argv[3])
    dst.mkdir(parents=True, exist_ok=True)
    mf = dst / "manifest.json"
    manifest = json.loads(mf.read_text()) if mf.exists() else {}
    for key, mk in sorted(marks.items()):
        manifest[key] = process(src / f"{key}.png", dst, key, mk)
        print(key, manifest[key]["w"], manifest[key]["h"])
    mf.write_text(json.dumps(manifest, indent=1, sort_keys=True))


if __name__ == "__main__":
    main()
