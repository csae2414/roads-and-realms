"""Small pre-rendered props for the meadows, shores and quarries (art director round 2, #6), with
the trees' camera and light (tools/art/trees.py, tools/art/prerender.py).

uv run --no-project --with numpy --with pillow python tools/art/props.py assets/art/props

Draw-only decoration: bushes, flowers, stumps, grass tufts, reeds, mushrooms and stones, a dry
and a snowy set, and six granite piles (sizes 1-6) as lit boulders.
"""
import json, sys
from pathlib import Path
import numpy as np
sys.path.insert(0, str(Path(__file__).parent))
import prerender as pr
import trees as tr
from PIL import Image

pr.MAT.update({
    "petal_red": (0.78, 0.18, 0.16),
    "petal_yellow": (0.92, 0.80, 0.22),
    "petal_white": (0.92, 0.92, 0.88),
    "petal_blue": (0.40, 0.48, 0.86),
    "reed": (0.52, 0.56, 0.26),
    "reedhead": (0.36, 0.24, 0.14),
    "grass": (0.36, 0.58, 0.20),
    "cap_red": (0.74, 0.16, 0.12),
    "stem": (0.88, 0.84, 0.74),
    "pebble": (0.58, 0.56, 0.52),
    "granite": (0.66, 0.64, 0.60),
    "bone": (0.86, 0.82, 0.72),
    "ice": (0.80, 0.88, 0.94),
    "wood": (0.70, 0.54, 0.34),
    # the mountains' pale grey-ochre (terrain colour 0xa89a84)
    "cragstone": (0.74, 0.68, 0.58),
})

_tree_texture = pr.texture
def texture(mat, pos, nrm):
    if mat in ("granite", "pebble", "cragstone"):
        # weathered stone: blotches and fine grain, lighter on top
        base = np.array(pr.MAT[mat])[None, :] * np.ones((len(pos), 1))
        k = 0.8 + 0.3 * pr.fbm(pos * 3.0, 2)[:, None] + 0.1 * (pr.fbm(pos * 14.0, 2)[:, None] - 0.5)
        return base * k
    if mat == "wood":
        # a cut log: year rings on top
        r = np.sqrt(pos[:, 0] ** 2 + pos[:, 1] ** 2)
        ring = 0.85 + 0.15 * np.sin(r * 60)[:, None]
        return np.array(pr.MAT[mat])[None, :] * ring
    if mat in pr.MAT and mat.startswith(("petal", "cap", "stem", "bone", "ice", "reed", "grass")):
        base = np.array(pr.MAT[mat])[None, :] * np.ones((len(pos), 1))
        return base * (0.85 + 0.25 * pr.fbm(pos * 5.0, 2)[:, None])
    return _tree_texture(mat, pos, nrm)
pr.texture = texture

S = tr.TreeScene

def bush(leaf, n=3, seed=0, snow=False):
    rng = np.random.default_rng(seed)
    s = S()
    for i in range(n):
        c = (rng.uniform(-0.35, 0.35), rng.uniform(-0.3, 0.3), 0.35 + rng.uniform(0, 0.15))
        s.blob(c, rng.uniform(0.32, 0.45), leaf)
        if snow:
            s.blob((c[0] + 0.03, c[1] - 0.03, c[2] + 0.18), 0.25, "snow", bump=0.08)
    return s

def flowers(petal, seed):
    rng = np.random.default_rng(seed)
    s = bush("leaf_mid", n=2, seed=seed)
    for _ in range(9):
        a = rng.uniform(0, 6.28)
        r = rng.uniform(0.1, 0.45)
        s.blob((np.cos(a) * r, np.sin(a) * r, 0.55 + rng.uniform(0, 0.25)), 0.08, petal, bump=0.02)
    return s

def stump():
    s = S()
    s.cyl((0, 0, 0), 0.3, 0.35, "bark")
    s.cyl((0, 0, 0.35), 0.27, 0.02, "wood")
    s.cyl((0.28, 0.0, 0), 0.07, 0.08, "bark")
    return s

def tuft(mat="grass", seed=0, n=16, h=0.4, r=0.035):
    rng = np.random.default_rng(seed)
    s = S()
    for _ in range(n):
        x, y = rng.uniform(-0.3, 0.3, 2)
        s.pine((x, y, 0), r, h * rng.uniform(0.6, 1.2), mat, bump=0.01)
    return s

def reeds(seed):
    s = tuft("reed", seed, n=12, h=1.1, r=0.05)
    rng = np.random.default_rng(seed + 7)
    for _ in range(4):
        x, y = rng.uniform(-0.2, 0.2, 2)
        s.cyl((x, y, 0.85), 0.045, 0.22, "reedhead")
    return s

def mushrooms():
    s = S()
    for a in np.linspace(0, 6.28, 6, endpoint=False):
        x, y = np.cos(a) * 0.4, np.sin(a) * 0.4
        s.cyl((x, y, 0), 0.04, 0.16, "stem")
        s.blob((x, y, 0.17), 0.1, "cap_red", bump=0.02)
    return s

def stones(mat, n, seed, spread=0.35, size=(0.15, 0.3)):
    rng = np.random.default_rng(seed)
    s = S()
    for _ in range(n):
        r = rng.uniform(*size)
        x, y = rng.uniform(-spread, spread, 2)
        # flattened, lumpy boulders
        c = np.array((x, y, r * 0.55))
        s.prims.append((lambda p, c=c, r=r: (np.linalg.norm((p - c) * np.array([1, 1, 1.5]), axis=-1) - r
                                             + (pr.fbm(p * 3.5, 2) - 0.5) * r * 0.9) * 0.6, mat))
    return s

def granite(size):
    # a pile: more and bigger boulders the more stone is left
    return stones("granite", 2 + size, 100 + size, spread=0.25 + size * 0.07, size=(0.22 + size * 0.02, 0.32 + size * 0.05))

def crag(seed):
    """A rock outcrop: a few tilted, weathered blocks of mountain stone (art director round 2, #4)."""
    rng = np.random.default_rng(seed)
    s = S()
    for _ in range(3 + seed % 2):
        hx, hy, hz = rng.uniform(0.2, 0.4), rng.uniform(0.15, 0.3), rng.uniform(0.12, 0.3)
        c = np.array((rng.uniform(-0.3, 0.3), rng.uniform(-0.25, 0.25), hz * 0.8))
        a = rng.uniform(-0.5, 0.5)
        rot = np.array([[np.cos(a), -np.sin(a), 0], [np.sin(a), np.cos(a), 0], [0, 0, 1]])
        tilt = rng.uniform(-0.25, 0.25)
        def f(p, c=c, rot=rot, half=np.array((hx, hy, hz)), tilt=tilt):
            q = (p - c) @ rot
            q = q + np.stack([q[..., 2] * tilt, np.zeros_like(q[..., 0]), np.zeros_like(q[..., 0])], -1)
            return (pr.sd_box(q, np.zeros(3), half) + (pr.fbm(p * 4.0, 2) - 0.5) * 0.12) * 0.7
        s.prims.append((f, "cragstone"))
    return s

def bones():
    s = stones("pebble", 2, 9, size=(0.1, 0.16))
    s.cyl((0.15, 0.1, 0.05), 0.04, 0.05, "bone")
    s.blob((-0.1, -0.15, 0.06), 0.09, "bone", bump=0.03)
    return s

PROPS = {
    "bush1": lambda: bush("leaf_dark", 3, 1),
    "bush2": lambda: bush("leaf_mid", 2, 2),
    "bush3": lambda: bush("leaf_light", 4, 3),
    "flowers1": lambda: flowers("petal_red", 4),
    "flowers2": lambda: flowers("petal_yellow", 5),
    "flowers3": lambda: flowers("petal_white", 6),
    "stump": stump,
    "tuft1": lambda: tuft(seed=7),
    "tuft2": lambda: tuft(seed=8, n=12, h=0.3),
    "reeds1": lambda: reeds(11),
    "reeds2": lambda: reeds(12),
    "mushrooms": mushrooms,
    "stones1": lambda: stones("pebble", 3, 21),
    "stones2": lambda: stones("pebble", 2, 22),
    "drybush": lambda: bush("dry", 3, 31),
    "bones": bones,
    "snowbush": lambda: bush("frost", 3, 41, snow=True),
    "icestones": lambda: stones("ice", 3, 42),
}
for n in range(1, 7):
    PROPS[f"granite{n}"] = (lambda n=n: granite(n))
for n in range(1, 5):
    PROPS[f"crag{n}"] = (lambda n=n: crag(n))

def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    manifest = {}
    for key, make in PROPS.items():
        big = key.startswith(("granite", "crag"))
        img = pr.render(make(), extent=(2.4, 2.4), res=(160, 160), center=(0, 0, 0.3))
        bbox = img.getbbox()
        if bbox is None:
            continue
        img = img.crop(bbox)
        tw = 56 if big else 30
        th = max(1, round(img.height * tw / img.width))
        img = img.resize((tw, th), Image.LANCZOS)
        img.save(out / f"{key}.png")
        manifest[key] = {"w": tw, "h": th}
        print(key, tw, th)
    (out / "manifest.json").write_text(json.dumps(manifest, indent=1, sort_keys=True))

if __name__ == "__main__":
    main()
