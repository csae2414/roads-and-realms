#!/usr/bin/env -S uv run -q --no-project --with numpy python
"""Level the audio assets: music and news jingles to a loudness target, ambience beds quieter,
short effects by their loudest moment (momentary loudness, 400 ms) to a target per class, mono,
never above -3 dBFS peak (sound designer #1: levelling effects by peak left a 28 dB spread).
Files already on target are left alone, so running it again does not re-encode them.

usage: uv run --no-project --with numpy python tools/loudness.py [--check]
   or: tools/loudness.py [--check]   (--check only reports and exits 1 when off target)
(numpy is needed: plain python3 failed on the first animal call, sound designer 2 #11)
"""
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent / "assets" / "audio"
JINGLES = {"alarm", "built", "lost", "fanfare", "victory", "defeat", "horn"}
# (integrated LUFS, true peak dBTP)
MUSIC = (-18.0, -1.5)
# (the beds 8 LU under the music at default sliders: sound designer #2; rr-audio AMBIENT_LUFS)
AMBIENT = (-24.0, -3.0)
JINGLE = (-16.0, -1.5)
# short effects: max momentary loudness (LUFS M-max) per class, and a peak ceiling
EFFECT_PEAK = -3.0
EFFECT_CLASS = {
    "combat": {"sword0", "sword1", "sword2", "death", "bow", "thump", "catapult"},
    "animal": {"sheep", "duck", "stag", "fox", "chirp", "deer", "bray", "bear", "pig"},
    "ui": {"click", "refuse", "news", "news_good", "news_bad"},
}
EFFECT_TARGET = {"work": -20.0, "combat": -18.0, "animal": -24.0, "ui": -22.0}
EFFECT_TOL = 1.5
# sounds whose energy is one sharp crash: the limiter can't lift them further without
# chattering (a falling tree pushed 9 dB came out quieter than pushed 6), so they may sit this
# far under target
CRASH_SLACK = {"treefall": 3.0}


def effect_class(stem):
    # (variants: duck0, duck1, ... are ducks)
    base = stem.rstrip("0123456789") or stem
    return next((c for c, s in EFFECT_CLASS.items() if stem in s or base in s), "work")


def momentary_max(f):
    # padded so a sound shorter than the 400 ms window is measured whole
    out = ff(["-i", str(f), "-af", "apad=pad_dur=0.5,ebur128", "-f", "null", "-"])
    vals = [float(v) for v in re.findall(r"M:\s*(-?[\d.]+)", out)]
    return max(vals) if vals else -120.0


# cues that carry meaning must survive a small speaker: their loudest moment through a 250 Hz
# high-pass (a laptop or TV) within LAPTOP_DB of the full one (sound designer round 3, #6: a
# death dropped 26 dB, a docking ship 15)
LAPTOP_DB = 10.0
LAPTOP_CUES = ("sword", "death", "thump", "catapult", "dock", "launch", "click", "refuse", "news", "alarm", "setdown", "promote")


def laptop_drop(f):
    full = momentary_max(f)
    out = ff(["-i", str(f), "-af", "highpass=f=250:poles=2,highpass=f=250:poles=2,apad=pad_dur=0.5,ebur128", "-f", "null", "-"])
    vals = [float(v) for v in re.findall(r"M:\s*(-?[\d.]+)", out)]
    return full - (max(vals) if vals else -120.0)


def ff(args):
    return subprocess.run(["ffmpeg", "-hide_banner", "-nostats", *args], capture_output=True, text=True).stderr


def measure(f, target):
    out = ff(["-i", str(f), "-af", f"loudnorm=I={target[0]}:TP={target[1]}:print_format=json", "-f", "null", "-"])
    return json.loads(out[out.rindex("{") : out.rindex("}") + 1])


def channels(f):
    out = subprocess.run(
        ["ffprobe", "-v", "error", "-select_streams", "a:0", "-show_entries", "stream=channels", "-of", "csv=p=0", str(f)],
        capture_output=True,
        text=True,
    ).stdout
    return int(out.strip() or 1)


def peak(f):
    return float(re.search(r"max_volume: (-?[\d.]+) dB", ff(["-i", str(f), "-af", "volumedetect", "-f", "null", "-"]))[1])


def encode(f, af, mono=False):
    tmp = f.with_suffix(".tmp.ogg")
    channels = ["-ac", "1"] if mono else []
    subprocess.run(
        ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-i", str(f), "-af", af, *channels, "-c:a", "libvorbis", "-q:a", "6", str(tmp)],
        check=True,
    )
    tmp.replace(f)


def decode(f):
    """The file as mono float samples at 44.1 kHz (via ffmpeg)."""
    import numpy as np

    raw = subprocess.run(
        ["ffmpeg", "-hide_banner", "-loglevel", "error", "-i", str(f), "-ac", "1", "-ar", "44100", "-f", "f32le", "-"],
        capture_output=True,
        check=True,
    ).stdout
    return np.frombuffer(raw, dtype=np.float32)


def bed_problem(f):
    """Ambience beds loop: at least 30 s long, and no dip or jump at the seam (sound designer #3).
    The levels of 100 ms windows round the seam stay within the bed's own range (its 10th to
    90th percentile, 3 dB either side): natural ups and downs pass, a hole (the old ones were
    15-26 dB) or a jump does not."""
    import numpy as np

    x = decode(f)
    if len(x) < 30 * 44100:
        return f"{len(x) / 44100:.1f} s long (at least 30)"
    win = 4410
    levels = 20 * np.log10(np.sqrt(np.mean(x[: len(x) // win * win].reshape(-1, win) ** 2, axis=1)) + 1e-9)
    lo, hi = np.percentile(levels, 10) - 3.0, np.percentile(levels, 90) + 3.0
    seam = np.concatenate([levels[-5:], levels[:5]])
    worst = float(max(lo - seam.min(), seam.max() - hi, 0.0))
    return None if worst == 0.0 else f"seam {worst:.1f} dB outside the bed's range"


def dull(f):
    """Animal calls need air: energy above 8 kHz at least -45 dB of the whole (sound designer #6:
    the synthesised ones were band-limited buzz, -57 to -73 dB)."""
    import numpy as np

    x = decode(f)
    spec = np.abs(np.fft.rfft(x)) ** 2
    freqs = np.fft.rfftfreq(len(x), 1 / 44100)
    hf = 10 * np.log10(spec[freqs > 8000].sum() / spec.sum() + 1e-12)
    return None if hf >= -45 else f"{hf:.0f} dB above 8 kHz (at least -45)"


def main():
    check = "--check" in sys.argv
    off = []
    for f in sorted(ROOT.glob("sfx/*.ogg")):
        # (the synthesised calls; the bird chirp and the sheep are recordings)
        if f.stem.rstrip("0123456789") in ("duck", "fox", "stag", "deer", "bear"):
            why = dull(f)
            print(f"{f.relative_to(ROOT)}: air {'ok' if why is None else why}")
            if why is not None:
                # (made by tools/synth_sfx.py)
                off.append(f)
    for f in sorted(ROOT.glob("ambient/*.ogg")):
        why = bed_problem(f)
        print(f"{f.relative_to(ROOT)}: loop {'ok' if why is None else why}")
        if why is not None:
            # (made by tools/ambience.py)
            off.append(f)
    for f in sorted(ROOT.glob("*/*.ogg")):
        kind = f.parent.name
        if kind == "sfx" and f.stem not in JINGLES:
            p, m = peak(f), momentary_max(f)
            target = EFFECT_TARGET[effect_class(f.stem)]
            # a sharp sound at the peak ceiling (after the limiter) may stay up to 2 LU under
            # target (4.5 hid the swords at -22 against -18: sound designer 2 #11); further down
            # it gets its gain through a limiter
            gain = target - m
            at_ceiling = p >= EFFECT_PEAK - 1.0 and 0 < target - m <= CRASH_SLACK.get(f.stem, 2.0)
            ok = (abs(m - target) <= EFFECT_TOL or at_ceiling) and p <= EFFECT_PEAK + 0.5
            print(f"{f.relative_to(ROOT)}: {m:+.1f} LUFS M-max ({effect_class(f.stem)} {target:+.0f}), peak {p:+.1f} dB {'ok' if ok else f'-> {gain:+.1f} dB'}")
            if f.stem.rstrip("0123456789") in LAPTOP_CUES:
                drop = laptop_drop(f)
                if drop > LAPTOP_DB:
                    print(f"{f.relative_to(ROOT)}: on a small speaker {drop:.1f} dB quieter (at most {LAPTOP_DB:.0f})")
                    off.append(f)
            if not ok:
                off.append(f)
                if not check:
                    # mono first (as the game plays it), then the gain, a limiter half a dB under
                    # the ceiling (the encoder overshoots a little)
                    limit = 10 ** ((EFFECT_PEAK - 0.5) / 20)
                    down = "pan=mono|c0=0.5*c0+0.5*c1," if channels(f) == 2 else ""
                    encode(
                        f,
                        f"{down}volume={gain:.2f}dB,alimiter=limit={limit:.3f}:level=false:attack=1:release=20",
                        mono=True,
                    )
            continue
        target = MUSIC if kind == "music" else AMBIENT if kind == "ambient" else JINGLE
        m = measure(f, target)
        i, tp = float(m["input_i"]), float(m["input_tp"])
        ok = abs(i - target[0]) <= 0.5 and tp <= target[1] + 0.5
        print(f"{f.relative_to(ROOT)}: {i:+.1f} LUFS, {tp:+.1f} dBTP {'ok' if ok else '-> ' + str(target[0])}")
        if not ok:
            off.append(f)
            if not check:
                af = (
                    f"loudnorm=I={target[0]}:TP={target[1]}:LRA=11:measured_I={m['input_i']}:"
                    f"measured_TP={m['input_tp']}:measured_LRA={m['input_lra']}:measured_thresh={m['input_thresh']}:"
                    f"offset={m['target_offset']}:linear=true,aresample=44100"
                )
                encode(f, af)
    if check and off:
        sys.exit(1)


if __name__ == "__main__":
    main()
