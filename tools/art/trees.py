"""Pre-rendered trees with the building renderer's camera and light (tools/art/prerender.py).

uv run --no-project --with numpy --with pillow python tools/art/trees.py assets/art/trees

Crowns are bumpy spheres (an SDF displaced by noise), conifers stacked cones; trunks are
cylinders. One picture per kind; the game scales it by growth and sways its top.
"""
import json, math, sys
from pathlib import Path
import numpy as np
sys.path.insert(0, str(Path(__file__).parent))
import prerender as pr
from PIL import Image

pr.MAT.update({
    "bark": (0.36, 0.25, 0.15),
    "birchbark": (0.86, 0.84, 0.78),
    "leaf_dark": (0.20, 0.40, 0.15),
    "leaf_mid": (0.30, 0.52, 0.18),
    "leaf_light": (0.46, 0.62, 0.24),
    "needle": (0.12, 0.32, 0.16),
    "olive": (0.40, 0.42, 0.20),
    "dry": (0.50, 0.44, 0.22),
    "frost": (0.62, 0.68, 0.64),
    "snow": (0.95, 0.96, 0.98),
})

_base_texture = pr.texture
def texture(mat, pos, nrm):
    col = _base_texture(mat, pos, nrm) if mat in ("bark", "birchbark") else None
    if col is None:
        base = np.array(pr.MAT[mat])[None, :] * np.ones((len(pos), 1))
        # leaf clusters: light and dark speckle
        k = 0.78 + 0.45 * pr.fbm(pos * 6.0, 2)[:, None]
        col = base * k
    if mat == "birchbark":
        marks = (pr.fbm(pos * np.array([3.0, 3.0, 14.0]), 2) > 0.62)[:, None]
        col = np.where(marks, col * 0.25, col)
    return col
pr.texture = texture

class TreeScene(pr.Scene):
    def blob(self, c, r, mat, bump=0.22):
        c = np.array(c)
        self.prims.append((lambda p, c=c: (np.linalg.norm(p - c, axis=-1) - r + (pr.fbm(p * 2.6, 2) - 0.5) * bump * 2) * 0.7, mat))
        return self

    def pine(self, c, r, h, mat, bump=0.12):
        c = np.array(c)
        self.prims.append((lambda p, c=c: (pr.sd_cone(p, c, r, h) + (pr.fbm(p * 4.0, 2) - 0.5) * bump * 2) * 0.7, mat))
        return self

def broadleaf(leaf, bark="bark", lean=0.0):
    s = TreeScene()
    s.cyl((0, 0, 0), 0.16, 1.6, bark)
    for c, r in [((lean, 0, 2.2), 0.95), ((-0.55, 0.25, 1.85), 0.62), ((0.55, -0.2, 1.9), 0.66), ((0.1, 0.35, 2.7), 0.6)]:
        s.blob(c, r, leaf)
    return s

def conifer(needle, snow=False):
    s = TreeScene()
    s.cyl((0, 0, 0), 0.12, 0.8, "bark")
    for z, r, h in [(0.5, 0.95, 1.5), (1.25, 0.75, 1.3), (1.95, 0.5, 1.1)]:
        s.pine((0, 0, z), r, h, needle)
        if snow:
            s.pine((0.04, -0.04, z + h * 0.42), r * 0.5, h * 0.58, "snow", bump=0.06)
    return s

TREES = {
    "oak": lambda: broadleaf("leaf_dark"),
    "beech": lambda: broadleaf("leaf_mid", lean=0.1),
    "birch": lambda: broadleaf("leaf_light", bark="birchbark", lean=-0.1),
    "pine": lambda: conifer("needle"),
    "olive": lambda: broadleaf("olive"),
    "drypine": lambda: conifer("dry"),
    "frost": lambda: broadleaf("frost"),
    "snowfir": lambda: conifer("needle", snow=True),
}

def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    manifest = {}
    for key, make in TREES.items():
        img = pr.render(make(), extent=(3.4, 4.2), res=(220, 272), center=(0, 0, 1.6))
        bbox = img.getbbox()
        img = img.crop(bbox)
        tw = 72
        th = round(img.height * tw / img.width)
        img = img.resize((tw, th), Image.LANCZOS)
        img.save(out / f"{key}.png")
        manifest[key] = {"w": tw, "h": th}
        print(key, tw, th)
    (out / "manifest.json").write_text(json.dumps(manifest, indent=1, sort_keys=True))

if __name__ == "__main__":
    main()
