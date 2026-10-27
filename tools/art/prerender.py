"""Pre-rendered building sprites without an image model: a tiny SDF ray marcher.

uv run --no-project --with numpy --with pillow python tools/art/prerender.py OUTDIR [key ...]

Each building is a list of primitives (boxes, gable roofs, cylinders, cones) with procedural
materials. The camera looks from the front-left, about 30 degrees down, like the game's view;
light comes from the upper left. Output: RGBA PNG per building + manifest entries.
"""
import json, math, sys
from pathlib import Path
import numpy as np
from PIL import Image

# ---------------------------------------------------------------- noise and materials

def hash3(p):
    return np.modf(np.sin(p[..., 0] * 127.1 + p[..., 1] * 311.7 + p[..., 2] * 74.7) * 43758.5453)[0] % 1.0

def vnoise(p):
    i = np.floor(p)
    f = p - i
    f = f * f * (3 - 2 * f)
    out = 0
    for dx in (0, 1):
        for dy in (0, 1):
            for dz in (0, 1):
                w = (f[..., 0] if dx else 1 - f[..., 0]) * (f[..., 1] if dy else 1 - f[..., 1]) * (f[..., 2] if dz else 1 - f[..., 2])
                out = out + w * hash3(i + np.array([dx, dy, dz]))
    return out

def fbm(p, oct=3):
    s, a, n = 0, 0.5, 0
    for _ in range(oct):
        s = s + a * vnoise(p)
        n += a
        a *= 0.5
        p = p * 2.03
    return s / n

MAT = {
    "plaster": (0.86, 0.80, 0.66),
    "timber": (0.45, 0.30, 0.17),
    "logs": (0.52, 0.34, 0.18),
    "stone": (0.66, 0.64, 0.60),
    "sandstone": (0.82, 0.62, 0.36),
    "redtile": (0.66, 0.26, 0.18),
    "slate": (0.33, 0.35, 0.42),
    "thatch": (0.76, 0.62, 0.32),
    "door": (0.30, 0.19, 0.10),
    "window": (0.12, 0.10, 0.10),
    "metal": (0.55, 0.56, 0.60),
    "gold": (0.92, 0.76, 0.25),
    "coal": (0.10, 0.10, 0.11),
    "ore": (0.55, 0.30, 0.22),
    "rock": (0.50, 0.46, 0.40),
    "water": (0.20, 0.35, 0.60),
    "grain": (0.86, 0.72, 0.30),
    "fire": (1.00, 0.55, 0.15),
    "canvas": (0.90, 0.88, 0.80),
    "green": (0.30, 0.50, 0.22),
    "pig": (0.88, 0.62, 0.60),
    "donkey": (0.50, 0.46, 0.42),
    "turf": (0.36, 0.44, 0.22),
    "clay": (0.78, 0.56, 0.34),
}

def cell_rand(a, b):
    """A random value 0..1 per integer cell (a, b): one brick, tile or plank its own shade."""
    return hash3(np.stack([a, b, np.zeros_like(a)], -1))

def texture(mat, pos, nrm):
    base = np.array(MAT[mat])[None, :] * np.ones((len(pos), 1))
    n1 = fbm(pos * 1.7)[:, None]
    fine = fbm(pos * 9.0, 2)[:, None]
    k = 0.85 + 0.3 * n1 + 0.12 * (fine - 0.5)
    # along a wall or a roof's ridge: walls and ridges run along x or y
    u_wall = pos[:, 0] + pos[:, 1]
    if mat in ("logs", "timber"):
        # horizontal planks / logs, each its own shade, rounded logs dark at their seams
        freq = 9.0 if mat == "logs" else 14.0
        rows = np.abs(np.sin(pos[:, 2] * freq))[:, None]
        plank = cell_rand(np.floor(pos[:, 2] * freq / math.pi), np.floor(u_wall * 1.3))[:, None]
        k = k * (0.66 + 0.34 * np.sqrt(rows)) * (0.88 + 0.24 * plank)
    elif mat in ("stone", "sandstone"):
        # blocks in courses, each block its own shade, dark mortar between
        zr = pos[:, 2] * 4.0
        row = np.floor(zr)
        u = u_wall * 2.4 + (row % 2) * 0.5
        mortar = (np.minimum(zr - row, 1 - (zr - row)) < 0.07) | (np.minimum(u - np.floor(u), 1 - (u - np.floor(u))) < 0.05)
        block = cell_rand(row, np.floor(u))[:, None]
        # a block's lower edge a little darker: light from above
        under = (0.94 + 0.06 * np.clip((zr - row) * 3, 0, 1))[:, None]
        k = k * (0.8 + 0.36 * block) * under * np.where(mortar, 0.62, 1.0)[:, None]
    elif mat in ("redtile", "slate", "clay"):
        # tiles in staggered courses: each course lit at its lower edge and in the shadow of the
        # course above at its top, each tile its own shade
        per = 9.0 if mat == "redtile" else 11.0
        zr = pos[:, 2] * per
        row = np.floor(zr)
        f = zr - row
        u = u_wall * (4.4 if mat == "redtile" else 5.2) + (row % 2) * 0.5
        fu = u - np.floor(u)
        gap = np.minimum(fu, 1 - fu) < 0.05
        tile = cell_rand(row, np.floor(u))[:, None]
        course = 0.62 + 0.42 * np.clip(1 - f, 0, 1) ** 0.7
        k = k * (course * np.where(gap, 0.75, 1.0))[:, None] * (0.86 + 0.28 * tile)
        if mat == "redtile":
            # sun-bleached and darker tiles mixed, as on old roofs
            base = base * (1 + (tile - 0.5) * np.array([[0.10, 0.25, 0.30]]))
    elif mat == "thatch":
        streak = fbm(np.stack([pos[:, 0] * 20, pos[:, 1] * 2, pos[:, 2] * 20], -1), 2)[:, None]
        # laid in courses, the lower edge of each a shade darker
        course = (0.86 + 0.14 * ((pos[:, 2] * 5.0) % 1.0))[:, None]
        k = k * (0.75 + 0.45 * streak) * course
    elif mat == "plaster":
        k = 0.92 + 0.12 * n1
        # half-timbering: dark posts, a rail at mid height and braces between them
        u = u_wall * 1.6
        fu = u - np.floor(u)
        post = np.minimum(fu, 1 - fu) < 0.05
        rail = np.abs((pos[:, 2] % 0.62) - 0.31) < 0.035
        brace = np.abs(((fu * 0.62) - (pos[:, 2] % 0.62)) % 0.62) < 0.035
        beam = (post | rail | (brace & (pos[:, 2] > 0.25)))[:, None]
        k = np.where(beam, 0.42, k)
    elif mat == "rock":
        # strata and cracks: bands across the slope, broken by noise
        band = np.abs(np.sin(pos[:, 2] * 9.0 + fbm(pos * 2.5)[:] * 5.0))[:, None]
        k = k * (0.74 + 0.3 * band) * (0.9 + 0.2 * fine)
    col = base * k
    # weathering: walls darker and a little greener towards the ground
    if mat in ("stone", "sandstone", "plaster", "logs", "timber"):
        low = np.clip(1 - pos[:, 2] / 0.45, 0, 1)[:, None] * (0.6 + 0.4 * n1)
        col = col * (1 - 0.22 * low) + np.array([[0.02, 0.035, 0.0]]) * low
    return col

# ---------------------------------------------------------------- primitives (SDF)

def sd_box(p, c, half):
    q = np.abs(p - c) - half
    return np.linalg.norm(np.maximum(q, 0), axis=-1) + np.minimum(np.max(q, axis=-1), 0)

def sd_gable(p, c, half, h, along_x=True):
    """Gable roof: base box footprint `half` (x,y) at height c.z, ridge `h` above, ridge along x."""
    q = p - c
    if not along_x:
        q = q[..., [1, 0, 2]]
        half = (half[1], half[0])
    hx, hy = half
    # triangle cross-section in (y, z): |y|/hy + z/h <= 1, z >= 0
    d_slope = (np.abs(q[..., 1]) * h + q[..., 2] * hy - h * hy) / math.hypot(h, hy)
    d = np.maximum.reduce([d_slope, -q[..., 2], np.abs(q[..., 0]) - hx])
    return d

def sd_cyl(p, c, r, h):
    q = p - c
    d_r = np.hypot(q[..., 0], q[..., 1]) - r
    d_h = np.abs(q[..., 2] - h / 2) - h / 2
    return np.minimum(np.maximum(d_r, d_h), 0) + np.hypot(np.maximum(d_r, 0), np.maximum(d_h, 0))

def sd_cone(p, c, r, h):
    q = p - c
    rr = np.hypot(q[..., 0], q[..., 1])
    d = (rr * h + q[..., 2] * r - h * r) / math.hypot(h, r)
    return np.maximum(d, -q[..., 2])

class Scene:
    def __init__(self):
        self.prims = []

    def box(self, c, half, mat):
        self.prims.append((lambda p, c=np.array(c), h=np.array(half): sd_box(p, c, h), mat))
        return self

    def gable(self, c, half, h, mat, along_x=True):
        self.prims.append((lambda p, c=np.array(c): sd_gable(p, c, half, h, along_x), mat))
        return self

    def cyl(self, c, r, h, mat):
        self.prims.append((lambda p, c=np.array(c): sd_cyl(p, c, r, h), mat))
        return self

    def cone(self, c, r, h, mat):
        self.prims.append((lambda p, c=np.array(c): sd_cone(p, c, r, h), mat))
        return self

    def rock(self, c, r, h, mat="rock", amp=0.22, seed=0.0):
        """A cone of rock with a lumpy, broken surface (a mine's shoulder, boulders)."""
        c = np.array(c)
        def f(p, c=c):
            bump = (fbm((p + seed) * 1.3) - 0.5) * 2 * amp + (fbm((p + seed) * 4.0, 2) - 0.5) * amp * 0.25
            return (sd_cone(p, c, r, h) - bump) * 0.8
        self.prims.append((f, mat))
        return self

    def dome(self, c, r, mat, squash=0.85):
        """Half a sphere standing on c."""
        c = np.array(c)
        self.prims.append((lambda p, c=c: np.maximum(np.linalg.norm((p - c) * np.array([1, 1, 1 / squash]), axis=-1) - r, c[2] - p[..., 2]) * min(1, squash), mat))
        return self

    def sdf(self, p):
        ds = np.stack([f(p) for f, _ in self.prims], -1)
        return ds.min(-1), ds.argmin(-1)

# ---------------------------------------------------------------- camera and render

YAW = math.radians(30)     # camera from the front-left: the front and the right side are visible
PITCH = math.radians(27)   # looking down
VIEW = np.array([-math.sin(YAW) * math.cos(PITCH), math.cos(YAW) * math.cos(PITCH), -math.sin(PITCH)])
RIGHT = np.cross(VIEW, [0, 0, 1]); RIGHT /= np.linalg.norm(RIGHT)
UP = np.cross(RIGHT, VIEW)
LIGHT = np.array([-0.45, -0.55, 0.70]); LIGHT /= np.linalg.norm(LIGHT)

# set by --snow: the winter pictures (`*_w.png`)
SNOW = False

def render(scene, extent=(6.0, 5.2), res=(495, 429), center=(0, 0, 1.6)):
    w, h = res
    xs = (np.arange(w) + 0.5) / w - 0.5
    ys = 0.5 - (np.arange(h) + 0.5) / h
    gx, gy = np.meshgrid(xs * extent[0], ys * extent[1])
    origin = np.array(center) - VIEW * 20 + gx[..., None] * RIGHT + gy[..., None] * UP
    o = origin.reshape(-1, 3)
    t = np.zeros(len(o))
    hit = np.zeros(len(o), bool)
    alive = np.ones(len(o), bool)
    for _ in range(140):
        p = o[alive] + VIEW * t[alive, None]
        d, _ = scene.sdf(p)
        idx = np.where(alive)[0]
        t[idx] += d * 0.9
        h_now = d < 1e-3
        hit[idx[h_now]] = True
        alive[idx[h_now | (t[idx] > 45)]] = False
        if not alive.any():
            break
    rgba = np.zeros((len(o), 4))
    hi = np.where(hit)[0]
    p = o[hi] + VIEW * t[hi, None]
    _, mid = scene.sdf(p)
    e = 2e-3
    n = np.stack([scene.sdf(p + np.array(v))[0] - scene.sdf(p - np.array(v))[0] for v in ([e, 0, 0], [0, e, 0], [0, 0, e])], -1)
    n /= np.linalg.norm(n, axis=-1, keepdims=True) + 1e-9
    col = np.zeros((len(p), 3))
    for k, (_, mat) in enumerate(scene.prims):
        sel = mid == k
        if sel.any():
            col[sel] = texture(mat, p[sel], n[sel])
    if SNOW:
        # winter: snow lies on roofs, ledges and crowns (surfaces facing up); on roofs it holds on
        # steep slopes too, in patches (art director round 3, #4: steep Viking turf and the
        # towers' cones stayed summer green)
        roof_ids = [k for k, (_, m) in enumerate(scene.prims) if m in ("thatch", "turf", "slate", "redtile")]
        roof = np.isin(mid, roof_ids)
        lo = np.where(roof, 0.12, 0.55)
        patches = np.where(roof, (fbm(p * 7.0) - 0.5) * 0.6, 0.0)
        t = np.clip((n[:, 2] - lo) / 0.25 + patches, 0, 1)
        t = t * t * (3 - 2 * t)
        col = col * (1 - t[:, None]) + np.array([0.93, 0.95, 0.98])[None, :] * t[:, None]
    lam = np.clip(n @ LIGHT, 0, 1)
    # soft shadow toward the light
    sh = np.ones(len(p))
    tt = np.full(len(p), 0.05)
    for _ in range(24):
        d, _ = scene.sdf(p + n * 0.01 + LIGHT * tt[:, None])
        sh = np.minimum(sh, np.clip(8 * d / tt, 0, 1))
        tt += np.clip(d, 0.05, 0.4)
    # ambient occlusion along the normal
    ao = np.ones(len(p))
    for s in (0.06, 0.15, 0.3):
        d, _ = scene.sdf(p + n * s)
        ao -= np.clip(s - d, 0, s) * 1.4
    ao = np.clip(ao, 0.35, 1)
    # warm sun, a cool sky filling the shade, a little warm light bounced up from the ground
    sky = (0.42 + 0.25 * n[:, 2])[:, None]
    bounce = np.clip(-n[:, 2], 0, 1)[:, None] * 0.18 + 0.05
    sun_c = np.array([[1.04, 0.97, 0.86]])
    sky_c = np.array([[0.80, 0.88, 1.04]])
    ground_c = np.array([[0.62, 0.66, 0.40]])
    light = (0.98 * lam[:, None] * sh[:, None] * sun_c + sky * 0.55 * sky_c + bounce * ground_c) * ao[:, None]
    glow = np.isin(mid, [k for k, (_, m) in enumerate(scene.prims) if m == "fire"])
    out = col * light
    out[glow] = col[glow]
    rgba[hi, :3] = np.clip(out, 0, 1)
    rgba[hi, 3] = 1
    img = (rgba.reshape(h, w, 4) * 255).astype(np.uint8)
    return Image.fromarray(img, "RGBA")

# ---------------------------------------------------------------- the buildings

# the people building (--nation): 0 Romans, 1 Vikings, 2 Nubians, 3 Japanese; each its own
# silhouette (art director round 2, #9)
NATION = 0

def roof_shape(s, x, y, z, hx, hy, roof_h, mat):
    """The roof over walls of half size (hx, hy) topping out at z, in the nation's style."""
    if NATION == 1:
        # Vikings: steep turf roofs, crossed boards at the gable ends
        h = roof_h * 1.6
        s.gable((x, y, z), (hx + 0.15, hy + 0.2), h, "turf")
        for gx in (-hx - 0.15, hx + 0.15):
            for sgn in (-1, 1):
                s.box((x + gx, y + sgn * 0.12, z + h + 0.12), (0.03, 0.03, 0.22), "timber")
        return h
    if NATION == 2:
        # Nubians: a flat roof behind a parapet, a dome on top
        s.box((x, y, z + 0.05), (hx + 0.05, hy + 0.05, 0.05), "plaster")
        for gx, gy, ex, ey in ((0, -hy - 0.05, hx + 0.05, 0.04), (0, hy + 0.05, hx + 0.05, 0.04),
                               (-hx - 0.05, 0, 0.04, hy + 0.05), (hx + 0.05, 0, 0.04, hy + 0.05)):
            s.box((x + gx, y + gy, z + 0.18), (ex, ey, 0.08), "plaster")
        s.dome((x, y, z + 0.1), min(hx, hy) * 0.75, mat)
        return 0.1 + min(hx, hy) * 0.65
    if NATION == 3:
        # Japanese: wide eaves, a low roof with its ridge ends turned up
        h = roof_h * 0.8
        s.gable((x, y, z), (hx + 0.35, hy + 0.4), h, mat)
        for gx in (-hx - 0.3, hx + 0.3):
            s.box((x + gx, y, z + h + 0.05), (0.08, 0.06, 0.12), mat)
        return h
    # Romans: low tile roofs
    h = roof_h * 0.6
    s.gable((x, y, z), (hx + 0.15, hy + 0.2), h, mat)
    return h

def walls(s, hx, hy, hz, mat, x=0.0, y=0.0):
    return s.box((x, y, hz), (hx, hy, hz), mat)

def door(s, x, y_front, w=0.32, h=0.7):
    # a frame around a plank door
    s.box((x, y_front - 0.015, h / 2 + 0.03), (w / 2 + 0.05, 0.03, h / 2 + 0.04), "timber")
    return s.box((x, y_front - 0.04, h / 2), (w / 2, 0.05, h / 2), "door")

def window(s, x, y_front, z, w=0.26):
    # frame, dark pane and a sill
    s.box((x, y_front - 0.015, z), (w / 2 + 0.05, 0.03, w / 2 + 0.05), "timber")
    s.box((x, y_front - 0.05, z - w / 2 - 0.04), (w / 2 + 0.08, 0.06, 0.025), "stone")
    return s.box((x, y_front - 0.035, z), (w / 2, 0.04, w / 2), "window")

def house(s, hx, hy, hz, wall, roof, roof_h=1.0, chimney=False, x=0.0, y=0.0):
    walls(s, hx, hy, hz, wall, x, y)
    # a stone plinth under timber and plaster
    if wall != "stone":
        s.box((x, y, 0.07), (hx + 0.05, hy + 0.05, 0.07), "stone")
    # an eave beam under the roof's front edge, the roof, a ridge beam
    s.box((x, y - hy - 0.02, 2 * hz - 0.03), (hx + 0.1, 0.05, 0.04), "timber")
    roof_h = roof_shape(s, x, y, 2 * hz, hx, hy, roof_h, roof_mat(roof))
    if NATION in (0, 1):
        s.box((x, y, 2 * hz + roof_h - 0.02), (hx + 0.17, 0.05, 0.05), "timber")
    if NATION == 0 and hx > 0.9:
        # a porch on two columns
        for cx in (-hx * 0.55, -hx * 0.05):
            s.cyl((x + cx, y - hy - 0.35, 0), 0.07, 2 * hz - 0.05, "plaster")
        s.box((x - hx * 0.3, y - hy - 0.3, 2 * hz - 0.03), (hx * 0.4, 0.3, 0.04), "redtile")
    if NATION == 3:
        # dark posts at the corners
        for cx in (-hx, hx):
            s.box((x + cx, y - hy - 0.02, hz), (0.05, 0.05, hz), "timber")
    door(s, x - hx * 0.3, y - hy)
    if hx > 0.9:
        window(s, x + hx * 0.45, y - hy, hz * 1.2)
        window(s, x - hx * 0.75, y - hy, hz * 1.2)
    if chimney:
        s.box((x + hx * 0.5, y + hy * 0.2, 2 * hz + roof_h * 0.6), (0.16, 0.16, roof_h * 0.6), "stone")
    return s

def roof_mat(mat):
    """Roofing per nation: Viking turf, Nubian clay, Japanese dark tiles."""
    if NATION == 1 and mat in ("redtile", "slate", "thatch"):
        return "turf"
    if NATION == 2 and mat in ("redtile", "slate"):
        return "clay"
    if NATION == 3 and mat in ("redtile", "thatch"):
        return "slate"
    return mat

def cap(s, x, y, z, r, mat="slate"):
    """A tower's top in the nation's style."""
    if NATION == 1:
        s.cone((x, y, z), r + 0.1, r * 2.4, "turf")
    elif NATION == 2:
        s.dome((x, y, z), r * 0.95, "clay")
    elif NATION == 3:
        # two tiers of wide, low roofs
        s.cone((x, y, z), r + 0.45, r * 0.55, "slate")
        s.cyl((x, y, z + r * 0.3), r * 0.5, r * 0.5, "plaster")
        s.cone((x, y, z + r * 0.75), r + 0.15, r * 0.6, "slate")
    else:
        s.cone((x, y, z), r + 0.15, r * 1.6, mat)
    return s

def tower(s, r, h, wall="stone", roof="slate", x=0.0, y=0.0):
    s.cyl((x, y, 0), r, h, wall)
    return cap(s, x, y, h, r, roof)

def crenels(s, hx, hy, z, n=4):
    for i in range(n):
        f = -hx + (i + 0.5) * 2 * hx / n
        s.box((f, -hy, z + 0.12), (0.12, 0.12, 0.12), "stone")
    return s

def lookout():
    """A watch platform on four tall legs (art director round 3, #8: a roof on four sticks):
    stone footings, cross braces, a railing round the deck and a ladder up the front."""
    s = sc()
    for x in (-0.45, 0.45):
        for y in (-0.45, 0.45):
            s.box((x, y, 0.12), (0.13, 0.13, 0.12), "stone")
            s.box((x, y, 1.35), (0.06, 0.06, 1.35), "timber")
    for z in (0.8, 1.7):
        s.box((0, -0.45, z), (0.45, 0.035, 0.035), "timber").box((0, 0.45, z), (0.45, 0.035, 0.035), "timber")
        s.box((-0.45, 0, z), (0.035, 0.45, 0.035), "timber").box((0.45, 0, z), (0.035, 0.45, 0.035), "timber")
    # the deck and its railing
    s.box((0, 0, 2.7), (0.62, 0.62, 0.05), "timber")
    for side in (-0.6, 0.6):
        s.box((0, side, 2.95), (0.6, 0.03, 0.03), "timber").box((side, 0, 2.95), (0.03, 0.6, 0.03), "timber")
    for x in (-0.6, 0.0, 0.6):
        for y in (-0.6, 0.6):
            s.box((x, y, 2.85), (0.03, 0.03, 0.15), "timber")
    # a ladder up the front
    for x in (-0.14, 0.14):
        s.box((x, 0.62, 1.35), (0.025, 0.025, 1.35), "timber")
    for k in range(9):
        s.box((0, 0.62, 0.25 + k * 0.28), (0.14, 0.02, 0.02), "timber")
    # a small roof on posts over the watchman
    for x in (-0.5, 0.5):
        for y in (-0.5, 0.5):
            s.box((x, y, 3.2), (0.03, 0.03, 0.45), "timber")
    s.gable((0, 0, 3.65), (0.62, 0.62), 0.5, "thatch")
    return s


def sc():
    return Scene()

def mine(heap):
    """A broken rock shoulder with boulders, a timber-framed portal, its dark mouth, a plank
    path and a spoil heap."""
    s = sc().rock((0, 0.55, 0), 1.95, 1.75)
    for (bx, by, br) in ((-1.25, -0.55, 0.32), (1.4, 0.2, 0.26), (-0.75, -1.05, 0.18)):
        s.rock((bx, by, 0), br * 1.3, br * 1.4, amp=0.08, seed=bx)
    # a little turf clinging to the top
    s.dome((0.25, 0.75, 1.2), 0.35, "turf", squash=0.4)
    s.box((0, -0.62, 0.4), (0.42, 0.55, 0.4), "rock")
    s.box((0, -1.13, 0.36), (0.26, 0.05, 0.32), "window")
    for x in (-0.32, 0.32):
        s.box((x, -1.15, 0.38), (0.06, 0.06, 0.38), "timber")
    s.box((0, -1.15, 0.8), (0.42, 0.07, 0.07), "timber")
    s.box((0, -1.45, 0.02), (0.18, 0.3, 0.02), "timber")
    s.cone((0.95, -1.15, 0), 0.42, 0.36, heap)
    return s

def crenels_all(s, hx, hy, z, n=4, mat="sandstone"):
    """Battlements round the whole top."""
    for i in range(n):
        f = -hx + (i + 0.5) * 2 * hx / n
        g = -hy + (i + 0.5) * 2 * hy / n
        s.box((f, -hy, z + 0.12), (0.11, 0.11, 0.12), mat)
        s.box((f, hy, z + 0.12), (0.11, 0.11, 0.12), mat)
        s.box((-hx, g, z + 0.12), (0.11, 0.11, 0.12), mat)
        s.box((hx, g, z + 0.12), (0.11, 0.11, 0.12), mat)
    return s

def slits(s, hx, hy, z, x=0.0, y=0.0):
    """Arrow slits on the two visible faces (front and right)."""
    for f in (-0.4, 0.4):
        s.box((x + f * hx, y - hy - 0.01, z), (0.03, 0.03, 0.13), "window")
        s.box((x + hx + 0.01, y + f * hy, z), (0.03, 0.03, 0.13), "window")
    return s

def keep(s, hx, hy, hz, x=0.0, y=0.0, roofed=True):
    """A sandstone tower block: plinth, slits, a timber hoarding, battlements or a slate cap."""
    s.box((x, y, hz), (hx, hy, hz), "sandstone")
    s.box((x, y, 0.08), (hx + 0.06, hy + 0.06, 0.08), "stone")
    slits(s, hx, hy, hz * 1.2, x, y)
    s.box((x, y, 2 * hz + 0.06), (hx + 0.1, hy + 0.1, 0.06), "timber")
    if roofed:
        if NATION == 0:
            s.cone((x, y, 2 * hz + 0.12), max(hx, hy) * 1.25, max(hx, hy) * 1.3, "slate")
        else:
            cap(s, x, y, 2 * hz + 0.12, max(hx, hy))
    elif NATION == 2:
        # a Nubian fort: smooth walls and a dome even on a flat top
        s.dome((x, y, 2 * hz + 0.12), min(hx, hy) * 0.6, "clay")
    else:
        crenels_all(s, hx, hy, 2 * hz + 0.12)
    return s

def sandtower(s, r, h, x=0.0, y=0.0):
    s.cyl((x, y, 0), r, h, "sandstone")
    if NATION != 0:
        cap(s, x, y, h, r)
    else:
        for a in range(8):
            ang = a * math.pi / 4
            s.box((x + math.cos(ang) * r, y + math.sin(ang) * r, h + 0.12), (0.1, 0.1, 0.12), "sandstone")
    s.box((x - r * 0.3, y - r - 0.01, h * 0.6), (0.03, 0.03, 0.13), "window")
    s.box((x + r * 0.5, y - r * 0.85, h * 0.75), (0.03, 0.03, 0.13), "window")
    return s

BUILDINGS = {
    # sandstone like the headquarters: slits, hoardings, battlements or slate caps
    "guardhouse": lambda: door(keep(sc(), 0.75, 0.75, 0.9), -0.2, -0.75),
    "watchtower": lambda: door(keep(keep(sc(), 1.05, 0.95, 0.4, roofed=False), 0.6, 0.6, 1.25, roofed=True), 0.0, -0.95),
    "fortress": lambda: door(sandtower(sandtower(sandtower(keep(sc(), 1.5, 1.3, 0.75, roofed=False), 0.45, 2.3, x=-1.5, y=-1.3), 0.45, 2.3, x=1.5, y=-1.3), 0.7, 2.8, y=0.4), 0.0, -1.3, w=0.5, h=0.8),
    # a long hall for the soldiers, with a yard fence and a weapons rack
    "barracks": lambda: door(house(sc(), 1.2, 0.75, 0.55, "sandstone", "slate", 0.8), -0.4, -0.75).box((1.45, -0.9, 0.35), (0.04, 0.04, 0.35), "timber").box((1.45, -0.9, 0.62), (0.03, 0.25, 0.03), "timber").box((1.45, -1.05, 0.45), (0.02, 0.02, 0.3), "metal").box((1.45, -0.75, 0.45), (0.02, 0.02, 0.3), "metal"),
    "lookouttower": lambda: lookout(),
    "catapult": lambda: house(sc(), 1.0, 0.8, 0.5, "timber", "slate", 0.6).box((0.2, -1.3, 0.25), (0.6, 0.25, 0.1), "timber").box((0.2, -1.3, 0.75), (0.06, 0.06, 0.6), "timber"),
    "granitemine": lambda: mine("rock"),
    "coalmine": lambda: mine("coal"),
    "ironmine": lambda: mine("ore"),
    "goldmine": lambda: mine("gold"),
    # a drying rack with fish and a canoe
    "fishery": lambda: house(sc(), 0.75, 0.65, 0.5, "logs", "thatch", 0.8, x=-0.25).box((0.75, -0.75, 0.45), (0.04, 0.04, 0.45), "timber").box((1.35, -0.75, 0.45), (0.04, 0.04, 0.45), "timber").box((1.05, -0.75, 0.88), (0.34, 0.03, 0.03), "timber").box((0.92, -0.75, 0.72), (0.05, 0.03, 0.13), "water").box((1.05, -0.75, 0.72), (0.05, 0.03, 0.13), "water").box((1.18, -0.75, 0.72), (0.05, 0.03, 0.13), "water").box((0.4, -1.35, 0.07), (0.6, 0.14, 0.07), "timber"),
    # a heap of cut blocks
    "quarry": lambda: house(sc(), 0.7, 0.65, 0.5, "stone", "slate", 0.7, x=-0.3).box((0.85, -0.7, 0.14), (0.22, 0.2, 0.14), "rock").box((1.3, -0.6, 0.14), (0.2, 0.22, 0.14), "rock").box((1.05, -0.65, 0.42), (0.2, 0.18, 0.14), "rock").box((1.1, -1.15, 0.1), (0.18, 0.16, 0.1), "rock"),
    # saplings in a little nursery
    "forester": lambda: house(sc(), 0.75, 0.65, 0.5, "logs", "green", 0.8, x=-0.3).cone((0.85, -0.7, 0.1), 0.22, 0.6, "green").cone((1.3, -0.5, 0.1), 0.26, 0.8, "green").cone((1.15, -1.1, 0.1), 0.18, 0.45, "green").cyl((0.85, -0.7, 0), 0.04, 0.12, "timber").cyl((1.3, -0.5, 0), 0.04, 0.12, "timber"),
    # a hide stretched on a frame
    "hunter": lambda: house(sc(), 0.75, 0.65, 0.5, "logs", "timber", 0.8, x=-0.3).box((0.8, -0.8, 0.5), (0.04, 0.04, 0.5), "timber").box((1.4, -0.8, 0.5), (0.04, 0.04, 0.5), "timber").box((1.1, -0.8, 0.98), (0.34, 0.04, 0.03), "timber").box((1.1, -0.8, 0.6), (0.24, 0.02, 0.3), "ore"),
    "well": lambda: sc().cyl((0, 0, 0), 0.5, 0.5, "stone").box((-0.4, 0, 0.8), (0.05, 0.05, 0.4), "timber").box((0.4, 0, 0.8), (0.05, 0.05, 0.4), "timber").gable((0, 0, 1.2), (0.55, 0.45), 0.4, "redtile").cyl((0, 0, 0.45), 0.42, 0.04, "water"),
    # a ham hanging under a beam, a chopping block
    "slaughterhouse": lambda: house(sc(), 1.0, 0.8, 0.6, "plaster", "redtile", 0.9, x=-0.2).box((1.25, -0.6, 0.55), (0.04, 0.04, 0.55), "timber").box((1.05, -0.6, 1.08), (0.25, 0.04, 0.04), "timber").cyl((0.95, -0.6, 0.62), 0.12, 0.34, "pig").cyl((1.25, -1.15, 0), 0.2, 0.3, "logs"),
    "brewery": lambda: house(sc(), 1.1, 0.8, 0.6, "plaster", "redtile", 0.9, chimney=True).cyl((1.35, -0.6, 0), 0.22, 0.45, "timber"),
    "armory": lambda: house(sc(), 1.1, 0.8, 0.6, "stone", "redtile", 0.9, chimney=True).box((1.35, -0.6, 0.2), (0.15, 0.1, 0.2), "metal"),
    # an anvil on a block outside
    "metalworks": lambda: house(sc(), 1.0, 0.8, 0.6, "stone", "slate", 0.9, chimney=True, x=-0.2).cyl((1.25, -0.9, 0), 0.16, 0.3, "logs").box((1.25, -0.9, 0.36), (0.22, 0.08, 0.06), "metal").box((1.25, -0.9, 0.31), (0.08, 0.06, 0.04), "metal"),
    "ironsmelter": lambda: house(sc(), 1.1, 0.8, 0.6, "stone", "slate", 0.8).box((0.7, 0.2, 1.6), (0.22, 0.22, 0.9), "stone").box((0.5, -0.82, 0.3), (0.18, 0.03, 0.15), "fire"),
    "pigfarm": lambda: house(sc(), 1.5, 0.8, 0.6, "plaster", "thatch", 1.0, x=-0.3, y=0.3).box((1.3, -0.9, 0.18), (0.6, 0.5, 0.02), "timber").cyl((1.2, -0.9, 0.1), 0.15, 0.18, "pig").cyl((1.5, -1.1, 0.1), 0.15, 0.18, "pig"),
    "storehouse": lambda: house(sc(), 1.2, 0.9, 0.7, "stone", "redtile", 0.9).box((-0.4, -0.92, 0.45), (0.3, 0.03, 0.45), "door"),
    "mill": lambda: tower(sc(), 0.75, 1.7, "stone", "timber").box((0, -0.85, 1.75), (1.3, 0.04, 0.1), "timber").box((0, -0.85, 1.75), (0.1, 0.04, 1.3), "timber").box((-0.15, -0.76, 0.35), (0.16, 0.03, 0.35), "door"),
    "bakery": lambda: house(sc(), 1.1, 0.8, 0.6, "plaster", "redtile", 0.9, chimney=True).cyl((1.35, -0.4, 0), 0.3, 0.35, "stone"),
    # a big saw blade at the gable, logs and boards outside
    "sawmill": lambda: house(sc(), 1.1, 0.8, 0.6, "timber", "redtile", 0.9).box((1.45, -0.5, 0.12), (0.35, 0.3, 0.12), "logs").box((1.45, -1.0, 0.05), (0.35, 0.12, 0.05), "plaster").box((0.55, -0.86, 0.75), (0.2, 0.02, 0.2), "metal"),
    "mint": lambda: house(sc(), 1.0, 0.8, 0.65, "stone", "redtile", 0.9).cyl((0.0, -0.84, 1.05), 0.14, 0.02, "gold"),
    "shipyard": lambda: house(sc(), 1.0, 0.8, 0.6, "timber", "redtile", 0.8).box((1.5, -0.4, 0.25), (0.25, 0.7, 0.2), "logs"),
    "farm": lambda: house(sc(), 1.5, 0.9, 0.6, "plaster", "thatch", 1.1, x=-0.2, y=0.2).box((1.4, -0.6, 0.02), (0.6, 0.6, 0.02), "grain").cone((1.6, 0.5, 0), 0.35, 0.6, "grain"),
    "donkeybreeder": lambda: house(sc(), 1.5, 0.8, 0.55, "logs", "thatch", 1.0, x=-0.2, y=0.3).box((1.3, -0.9, 0.2), (0.6, 0.5, 0.02), "timber").box((1.3, -0.9, 0.25), (0.25, 0.1, 0.12), "donkey"),
    # the Romans' headquarters and woodcutter are hand-made pictures; these are the other
    # peoples' and the snowy ones (render them with --nation=1..3 or --snow only)
    "headquarters": lambda: door(sandtower(sandtower(keep(keep(sc(), 1.6, 1.4, 0.7, roofed=False), 0.75, 0.7, 1.45, roofed=True), 0.5, 2.0, x=-1.6, y=-1.4), 0.5, 2.0, x=1.6, y=-1.4), 0.0, -1.4, w=0.55, h=0.85),
    "woodcutter": lambda: house(sc(), 0.75, 0.65, 0.5, "logs", "thatch", 0.8, x=-0.3).cyl((1.0, -0.8, 0), 0.2, 0.2, "logs").box((1.0, -0.8, 0.28), (0.02, 0.15, 0.08), "metal").box((0.95, -0.3, 0.1), (0.35, 0.12, 0.1), "logs").box((0.95, -0.3, 0.28), (0.3, 0.1, 0.08), "logs"),
    "harbor": lambda: house(sc(), 1.4, 1.0, 0.7, "stone", "redtile", 1.0).box((1.6, -1.1, 0.08), (0.3, 0.9, 0.06), "timber").box((1.5, 0.3, 1.3), (0.06, 0.06, 1.3), "timber"),
}

WIDTH = {"hut": 84, "house": 112, "castle": 156, "mine": 84}
HIRES = 1.5
SIZE = {
    "fortress": "castle", "headquarters": "castle", "woodcutter": "hut", "farm": "castle", "pigfarm": "castle", "donkeybreeder": "castle", "harbor": "castle",
    "granitemine": "mine", "coalmine": "mine", "ironmine": "mine", "goldmine": "mine",
    "guardhouse": "hut", "lookouttower": "hut", "fishery": "hut", "quarry": "hut", "forester": "hut", "hunter": "hut", "well": "hut",
}

def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    args = sys.argv[2:]
    nation = 0
    global SNOW, NATION
    while args and args[0].startswith("--"):
        if args[0].startswith("--nation="):
            nation = int(args[0].split("=")[1])
        elif args[0] == "--snow":
            SNOW = True
        args = args[1:]
    # the nations build in their own materials (0 Romans, 1 Vikings, 2 Nubians, 3 Japanese)
    if nation == 1:
        MAT["redtile"] = (0.40, 0.28, 0.18); MAT["plaster"] = (0.55, 0.38, 0.22)
    elif nation == 2:
        MAT["redtile"] = (0.72, 0.50, 0.30); MAT["plaster"] = (0.80, 0.62, 0.42); MAT["thatch"] = (0.70, 0.55, 0.30)
    elif nation == 3:
        MAT["redtile"] = (0.26, 0.30, 0.40); MAT["plaster"] = (0.93, 0.91, 0.86); MAT["thatch"] = (0.30, 0.34, 0.42)
    NATION = nation
    suffix = (f"_n{nation}" if nation else "") + ("_w" if SNOW else "")
    keys = args or list(BUILDINGS)
    mf = out / "manifest.json"
    manifest = json.loads(mf.read_text()) if mf.exists() else {}
    for key in keys:
        img = render(BUILDINGS[key]())
        bbox = img.getbbox()
        img = img.crop(bbox)
        tw = WIDTH[SIZE.get(key, "house")]
        th = round(img.height * tw / img.width)
        # 1.5x the pixels of the size on the map: crisp up to zoom 3 (the manifest keeps the size)
        img = img.resize((round(tw * HIRES), round(th * HIRES)), Image.LANCZOS)
        img.save(out / f"{key}{suffix}.png")
        manifest[key + suffix] = {"w": tw, "h": th, "ax": tw / 2, "ay": th - th * 0.14, "source": "prerender"}
        print(key, tw, th)
    mf.write_text(json.dumps(manifest, indent=1, sort_keys=True))

if __name__ == "__main__":
    main()
