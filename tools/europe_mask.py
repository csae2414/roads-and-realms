"""Rasterize Natural Earth 1:50m land and lakes (public domain) into the Europe map grid.

Usage: python3 tools/europe_mask.py land.geojson lakes.geojson [W H LON0 LON1 LAT0 LAT1] > data/maps/<name>_mask.txt
Grid: W x H nodes, odd rows shifted half a node; lon/lat window (LAT0 north, LAT1 south).
"""
import json, sys

W, H = 200, 232
LON0, LON1 = -11.0, 46.0
LAT0, LAT1 = 71.5, 34.0
if len(sys.argv) >= 9:
    W, H = int(sys.argv[3]), int(sys.argv[4])
    LON0, LON1, LAT0, LAT1 = (float(x) for x in sys.argv[5:9])

def polys(path):
    out = []
    for f in json.load(open(path))["features"]:
        g = f["geometry"]
        parts = g["coordinates"] if g["type"] == "MultiPolygon" else [g["coordinates"]]
        for p in parts:
            ring = p[0]
            xs = [c[0] for c in ring]; ys = [c[1] for c in ring]
            if max(xs) < LON0 - 1 or min(xs) > LON1 + 1 or max(ys) < LAT1 - 1 or min(ys) > LAT0 + 1:
                continue
            out.append((min(xs), max(xs), min(ys), max(ys), ring, p[1:]))
    return out

def inside(ring, x, y):
    c = False
    n = len(ring)
    j = n - 1
    for i in range(n):
        xi, yi = ring[i][0], ring[i][1]
        xj, yj = ring[j][0], ring[j][1]
        if (yi > y) != (yj > y) and x < (xj - xi) * (y - yi) / (yj - yi) + xi:
            c = not c
        j = i
    return c

def hit(ps, x, y):
    for (x0, x1, y0, y1, ring, holes) in ps:
        if x0 <= x <= x1 and y0 <= y <= y1 and inside(ring, x, y) and not any(inside(h, x, y) for h in holes):
            return True
    return False

land = polys(sys.argv[1])
lakes = polys(sys.argv[2])
print(f"# Land mask {W}x{H}, lon {LON0}..{LON1}, lat {LAT0}..{LAT1}. '#' land, '.' sea, 'l' lake.")
print("# Source: Natural Earth 1:50m land and lakes, public domain (naturalearthdata.com).")
for y in range(H):
    row = []
    for x in range(W):
        px = x + (0.5 if y % 2 else 0.0)
        lon = LON0 + px / W * (LON1 - LON0)
        lat = LAT0 + y / H * (LAT1 - LAT0)
        if hit(land, lon, lat):
            row.append("l" if hit(lakes, lon, lat) else "#")
        else:
            row.append(".")
    print("".join(row))
