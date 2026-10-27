"""Rework the ambience beds into long, seamless loops (sound designer #3), from the files we have:

- stream and wind: 45 s textures granulated from the short loops (random grains, random gains,
  slow gusts for the wind), so no period can be heard;
- waves: the fades at both ends cut off;
- birds: high-passed at 250 Hz (the source carried a synth drone at 79-177 Hz);
- every bed: a 5 s equal-power crossfade baked into its seam, so repeating it is seamless.

uv run --no-project --with numpy --with scipy python tools/ambience.py [SOURCE_DIR]
(then tools/loudness.py levels them)

It reads the original CC0 files (named as below, from the sources in assets/PROVENANCE.md) from
SOURCE_DIR, by default assets/audio/src/ambient, and writes only to assets/audio/ambient: it never
reads its own output (sound designer 2 #8: run twice it granulated the granulated beds again and
cut another crossfade off each). The originals are not in the repository; fetch them first.
"""
import pathlib
import subprocess
import sys

import numpy as np
from scipy.signal import butter, sosfilt

ROOT = pathlib.Path(__file__).resolve().parent.parent / "assets" / "audio" / "ambient"
SOURCES = {"stream": "stream.ogg", "wind": "wind.ogg", "waves": "waves.ogg", "birds": "birds.ogg"}
SR = 44100
FADE = 5.0


def decode(f):
    raw = subprocess.run(
        ["ffmpeg", "-hide_banner", "-loglevel", "error", "-i", str(f), "-ac", "1", "-ar", str(SR), "-f", "f32le", "-"],
        capture_output=True,
        check=True,
    ).stdout
    return np.frombuffer(raw, dtype=np.float32).astype(np.float64)


def encode(f, x):
    x = (x / max(1e-9, np.max(np.abs(x))) * 0.7).astype(np.float32)
    tmp = f.with_suffix(".tmp.ogg")
    subprocess.run(
        ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "f32le", "-ar", str(SR), "-ac", "1", "-i", "-",
         "-c:a", "libvorbis", "-q:a", "6", str(tmp)],
        input=x.tobytes(),
        check=True,
    )
    tmp.replace(f)


def granulate(src, seconds, grain, rng, gust=None):
    """A long texture of overlapping hann-windowed grains taken at random from `src`."""
    n = int(seconds * SR)
    g = int(grain * SR)
    out = np.zeros(n + g)
    win = np.hanning(g)
    hop = g // 4
    for start in range(0, n, hop):
        o = rng.integers(0, max(1, len(src) - g))
        out[start : start + g] += src[o : o + g] * win * rng.uniform(0.7, 1.0)
    out = out[:n]
    if gust is not None:
        # slow gusts: a sum of slow random sines between 0.6 and 1.0
        t = np.arange(n) / SR
        env = sum(np.sin(2 * np.pi * rng.uniform(0.03, 0.12) * t + rng.uniform(0, 6.3)) for _ in range(3))
        out *= 0.8 + 0.2 * env / 3
    return out


def trim_quiet_ends(x, db=-6.0):
    """Cut the fade-in and fade-out: from the first to the last 100 ms window within `db` of the median."""
    win = SR // 10
    lv = 20 * np.log10(np.sqrt(np.mean(x[: len(x) // win * win].reshape(-1, win) ** 2, axis=1)) + 1e-9)
    loud = np.where(lv > np.median(lv) + db)[0]
    return x[loud[0] * win : (loud[-1] + 1) * win]


def seamless(x):
    """Bake an equal-power crossfade into the seam: the last FADE seconds fade into the first."""
    f = int(FADE * SR)
    t = np.linspace(0, np.pi / 2, f)
    head, tail = x[:f], x[-f:]
    mixed = tail * np.cos(t) + head * np.sin(t)
    return np.concatenate([mixed, x[f:-f]])


def typical_start(x):
    """A seamless loop rotated so the file starts where it sounds most typical (a second of
    windows near the median level), not inside a lull or a call."""
    win = SR // 10
    lv = 20 * np.log10(np.sqrt(np.mean(x[: len(x) // win * win].reshape(-1, win) ** 2, axis=1)) + 1e-9)
    med = np.median(lv)
    score = [np.max(np.abs(np.take(lv, range(i - 5, i + 5), mode="wrap") - med)) for i in range(len(lv))]
    return np.roll(x, -int(np.argmin(score)) * win)


def main():
    src = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT.parent / "src" / "ambient"
    if src.resolve() == ROOT.resolve():
        return "the sources can't be the output folder: that reworks the reworked beds"
    missing = [f for f in SOURCES.values() if not (src / f).is_file()]
    if missing:
        return f"missing originals in {src}: {', '.join(missing)} (see assets/PROVENANCE.md)"
    rng = np.random.default_rng(7)
    beds = {}
    beds["stream"] = granulate(decode(src / SOURCES["stream"]), 45, 0.6, rng)
    beds["wind"] = granulate(decode(src / SOURCES["wind"]), 45, 1.5, rng, gust=True)
    beds["waves"] = trim_quiet_ends(decode(src / SOURCES["waves"]))
    birds = decode(src / SOURCES["birds"])
    beds["birds"] = trim_quiet_ends(sosfilt(butter(4, 250, "highpass", fs=SR, output="sos"), birds))
    for name, x in beds.items():
        encode(ROOT / f"{name}.ogg", typical_start(seamless(x)))
        print(name, f"{len(x) / SR:.1f} s")


if __name__ == "__main__":
    sys.exit(main())
