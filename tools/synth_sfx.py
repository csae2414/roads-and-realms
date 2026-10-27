#!/usr/bin/env python3
"""Make the synthesized effects (saw, bow, scythe) as WAV files in the given folder; encode them
to ogg with ffmpeg. usage: uv run --no-project --with numpy python tools/synth_sfx.py OUTDIR"""
import numpy as np, wave, sys
SR=44100; out=sys.argv[1]; rng=np.random.default_rng(7)
def save(name,x):
    x=x/np.max(np.abs(x))*0.7
    with wave.open(f"{out}/{name}.wav","wb") as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(SR); w.writeframes((x*32767).astype(np.int16).tobytes())
def bp(x,lo,hi):
    X=np.fft.rfft(x); f=np.fft.rfftfreq(len(x),1/SR); X[(f<lo)|(f>hi)]=0; return np.fft.irfft(X,len(x))
# bow: a plucked string (Karplus-Strong) with a falling pitch, plus a short swish of the arrow
def ks(freq,dur,decay=0.996):
    n=int(SR*dur); N=int(SR/freq); buf=rng.uniform(-1,1,N); y=np.zeros(n)
    for i in range(n):
        y[i]=buf[i%N]; buf[i%N]=decay*0.5*(buf[i%N]+buf[(i+1)%N])
    return y
t=np.arange(int(SR*0.5))/SR
bow=ks(98,0.5)*np.exp(-t*7)
sw=bp(rng.normal(size=len(t)),1500,6000)*np.exp(-((t-0.08)/0.04)**2)*0.35
save("bow",bow+sw)
# hand saw: three strokes of band-passed noise, each rising and falling
n=int(SR*0.9); t=np.arange(n)/SR; env=np.abs(np.sin(np.pi*t/0.3))**1.5
saw=bp(rng.normal(size=n),900,4500)*env*(1+0.5*np.sin(2*np.pi*55*t))
save("handsaw",saw)
# scythe: one swish, noise through a band-pass whose centre sweeps up (state-variable filter)
n=int(SR*0.45); t=np.arange(n)/SR; x=rng.normal(size=n); y=np.zeros(n); lo=bd=0.0
for i in range(n):
    fc=600+5000*(i/n)**1.5; f=2*np.sin(np.pi*fc/SR); q=0.6
    hi=x[i]-lo-q*bd; bd+=f*hi; lo+=f*bd; y[i]=bd
save("scythe",y*np.sin(np.pi*t/t[-1])**2)

# ---- animal calls (no CC0 recordings found): source-filter synthesis
def formants(x, fs, bw=120.0):
    """Resonances at the given frequencies (vocal tract), summed."""
    y = np.zeros_like(x)
    for f in fs:
        r = np.exp(-np.pi * bw / SR); th = 2 * np.pi * f / SR
        a1, a2 = -2 * r * np.cos(th), r * r
        z1 = z2 = 0.0
        for i in range(len(x)):
            v = x[i] - a1 * z1 - a2 * z2
            y[i] += v; z2, z1 = z1, v
    return y

def glottal(f0, n, buzz=0.0):
    """A pulse train at a gliding pitch f0[i], plus some noise for breath."""
    ph = np.cumsum(f0 / SR) % 1.0
    saw = 2 * ph - 1
    return saw + buzz * rng.normal(size=n)

def env(n, attack, release):
    t = np.arange(n) / SR
    e = np.minimum(1, t / attack) * np.minimum(1, (t[-1] - t) / release)
    return np.clip(e, 0, 1)

# The animals (sound designer #6): every repeat inside a call differs in pitch (8 %) and timing
# (20 ms), the voice has jitter and shimmer, and breath noise above 3 kHz gives it air; three
# variants each. The stag bellows, the deer gives a short bark.
def voice(f0, n, buzz, form, bw, attack, release):
    jitter = 1 + 0.015 * np.convolve(rng.normal(size=n), np.ones(200) / 200, "same") * 6
    v = formants(glottal(f0 * jitter, n, buzz), form, bw)
    shimmer = 1 + 0.08 * np.convolve(rng.normal(size=n), np.ones(400) / 400, "same") * 8
    breath = hp(rng.normal(size=n), 3000) * 0.08 * np.max(np.abs(v))
    return (v * shimmer + breath) * env(n, attack, release)

def hp(x, lo):
    X = np.fft.rfft(x); f = np.fft.rfftfreq(len(x), 1 / SR)
    X *= 1 / np.sqrt(1 + (lo / np.maximum(f, 1)) ** 4)   # a gentle 2nd-order slope, no brick wall
    return np.fft.irfft(X, len(x))

def call(parts):
    out = []
    for p, gap in parts:
        out += [p, np.zeros(max(0, int(SR * (gap + rng.uniform(-0.02, 0.02)))))]
    return np.concatenate(out)

for k in range(3):
    # duck: two or three nasal quacks
    quacks = []
    for q in range(2 + (k == 2)):
        n = int(SR * rng.uniform(0.13, 0.18))
        p = rng.uniform(0.92, 1.08)
        quacks.append((voice(np.linspace(330, 260, n) * p, n, 0.15, [1000, 1800, 2900], 160, 0.008, 0.05), 0.09))
    save(f"duck{k}", call(quacks))

    # fox: two to four short high yelps with a sharp pitch fall
    yelps = []
    for q in range(2 + k):
        n = int(SR * rng.uniform(0.11, 0.16))
        p = rng.uniform(0.92, 1.08)
        yelps.append((voice(np.linspace(900, 520, n) * p, n, 0.2, [1600, 3200], 260, 0.005, 0.06), 0.12))
    save(f"fox{k}", call(yelps))

    # stag: a long rutting bellow, low and rough, rising then falling
    n = int(SR * rng.uniform(1.1, 1.5))
    t = np.arange(n) / SR
    f0 = (95 + 70 * np.sin(np.pi * t / t[-1]) ** 1.5) * rng.uniform(0.92, 1.08)
    save(f"stag{k}", voice(f0, n, 0.3, [400, 900, 2200], 220, 0.12, 0.35))

    # deer: one or two short gruff barks
    barks = []
    for q in range(1 + (k > 0)):
        n = int(SR * rng.uniform(0.16, 0.22))
        f0 = np.linspace(260, 190, n) * rng.uniform(0.92, 1.08)
        barks.append((voice(f0, n, 0.5, [700, 1400, 2600], 300, 0.004, 0.08), 0.25))
    save(f"deer{k}", call(barks))

# anvil: a struck steel block for the smiths, minter and smelter (sound designer #4: they all
# played the carpenter's wooden hammer); inharmonic partials of a bar, each its own decay, a
# short hard click, three slightly different blows
for k, f in enumerate([1180, 1240, 1120]):
    n = int(SR * 0.9)
    t = np.arange(n) / SR
    ring = sum(a * np.sin(2 * np.pi * f * m * t + rng.uniform(0, 6.3)) * np.exp(-t / d)
               for m, a, d in [(1.0, 1.0, 0.35), (2.76, 0.6, 0.18), (5.40, 0.35, 0.09), (8.93, 0.2, 0.05)])
    click = bp(rng.normal(size=n), 2000, 9000) * np.exp(-t / 0.004) * 0.8
    save(f"anvil{k}", (ring + click) * env(n, 0.001, 0.2))

# one cue per meaning (sound designer #5)
# refuse: a dull low "uh-uh", two short notes falling, soft square-ish tone
notes = []
for f in (196, 147):
    n = int(SR * 0.11)
    t = np.arange(n) / SR
    tone = np.tanh(3 * np.sin(2 * np.pi * f * t)) * 0.6 + 0.4 * np.sin(2 * np.pi * 2 * f * t)
    notes += [bp(tone, 80, 1500) * env(n, 0.006, 0.04), np.zeros(int(SR * 0.05))]
save("refuse", np.concatenate(notes))

# news: a soft two-note chime, rising
n = int(SR * 0.7)
t = np.arange(n) / SR
chime = np.zeros(n)
for f, at in ((660, 0.0), (880, 0.12)):
    k = int(at * SR)
    tt = t[: n - k]
    chime[k:] += (np.sin(2 * np.pi * f * tt) + 0.3 * np.sin(2 * np.pi * 2.01 * f * tt)) * np.exp(-tt / 0.18)
save("news", chime * env(n, 0.003, 0.1))

# horn: the end of the truce, a brassy call falling a fourth, with a little vibrato
n = int(SR * 1.2)
t = np.arange(n) / SR
f0 = np.where(t < 0.5, 294.0, 220.0) * (1 + 0.006 * np.sin(2 * np.pi * 5.5 * t))
save("horn", formants(glottal(f0, n, 0.03), [600, 1150, 2400], 140) * env(n, 0.05, 0.3))

# collapse: a wooden building taken down, a creak and three dull thuds of falling timber
n = int(SR * 1.0)
t = np.arange(n) / SR
x = bp(rng.normal(size=n), 60, 900) * np.exp(-t / 0.35) * 0.5
for at, f in ((0.05, 70), (0.28, 55), (0.47, 62)):
    k = int(at * SR)
    tt = t[: n - k]
    x[k:] += np.sin(2 * np.pi * f * tt * (1 - 0.3 * tt)) * np.exp(-tt / 0.09)
save("collapse", x * env(n, 0.002, 0.2))

# thump: a catapult's stone landing, a low drop and a burst of grit
n = int(SR * 0.6)
t = np.arange(n) / SR
x = np.sin(2 * np.pi * 75 * t * (1 - 0.5 * t)) * np.exp(-t / 0.12) + bp(rng.normal(size=n), 300, 4000) * np.exp(-t / 0.05) * 0.6
save("thump", x * env(n, 0.001, 0.15))

# fire: a burning building crackles, pops of grit over a low roar (sound designer #5)
n = int(SR * 1.6)
t = np.arange(n) / SR
roar = bp(rng.normal(size=n), 80, 600) * 0.35
pops = np.zeros(n)
for _ in range(28):
    k = rng.integers(0, n - 2000)
    m = int(SR * rng.uniform(0.003, 0.012))
    pops[k:k + m] += rng.normal(size=m) * np.exp(-np.arange(m) / (m / 4)) * rng.uniform(0.4, 1.0)
save("fire", (roar + bp(pops, 900, 9000)) * env(n, 0.15, 0.4))

# bray: a pack donkey's hee-haw, the voice flipping between a high squeal and a low honk
parts = []
for k in range(2):
    for f0, d in ((520, 0.32), (180, 0.38)):
        n = int(SR * d)
        f = np.full(n, f0) * rng.uniform(0.95, 1.05)
        parts.append((voice(f, n, 0.35, [800, 1500, 2700], 220, 0.03, 0.06), 0.02))
save("bray", call(parts))

# polar bear (item 335): a low rough growl with a slow tremble, three variants (at the end, so
# the effects above keep their random draws)
for k in range(3):
    n = int(SR * rng.uniform(0.9, 1.3))
    t = np.arange(n) / SR
    f0 = np.linspace(72, 52, n) * rng.uniform(0.9, 1.1)
    g = voice(f0, n, 0.85, [320, 720, 1700], 320, 0.06, 0.3)
    save(f"bear{k}", g * (1 + 0.35 * np.sin(2 * np.pi * rng.uniform(5, 8) * t)))

# launch: a ship slides off the slip, a timber groan, the hull's rush and a heavy splash
# (sound designer 2 #1: its own cue, no fanfare)
n = int(SR * 1.8)
t = np.arange(n) / SR
# (the groan's formants and the splash lifted into the mid band: a small speaker lost the
# rumble, 14.5 dB; sound designer round 3, #6)
groan = formants(glottal(110 + 40 * t, n, 0.6), [450, 1100, 2200], 120) * np.exp(-t / 0.6) * 0.6
rush = bp(rng.normal(size=n), 200, 2500) * np.clip((t - 0.4) / 0.6, 0, 1) * np.exp(-np.clip(t - 1.0, 0, None) / 0.25) * 0.5
k = int(1.0 * SR)
tt = t[: n - k]
splash = np.zeros(n)
splash[k:] = bp(rng.normal(size=n - k), 400, 6000) * np.exp(-tt / 0.3) * 1.4 + np.sin(2 * np.pi * 60 * tt) * np.exp(-tt / 0.15) * 0.4
save("launch", (groan + rush + splash) * env(n, 0.02, 0.3))

# news_good, news_bad: a find rises over three notes, a loss falls over two (sound designer 2
# #2: one chime meant a nag, a loss and a reward)
def chime_notes(notes, gap, n):
    t = np.arange(n) / SR
    x = np.zeros(n)
    for i, f in enumerate(notes):
        k = int(i * gap * SR)
        tt = t[: n - k]
        x[k:] += (np.sin(2 * np.pi * f * tt) + 0.3 * np.sin(2 * np.pi * 2.01 * f * tt)) * np.exp(-tt / 0.2)
    return x * env(n, 0.003, 0.1)
save("news_good", chime_notes((660, 880, 1100), 0.11, int(SR * 0.7)))
save("news_bad", chime_notes((587, 440), 0.16, int(SR * 0.7)))

# dock: a ship comes alongside, wood rubbing on the quay and a rope's dull slap (sound designer
# 2 #6)
n = int(SR * 1.2)
t = np.arange(n) / SR
# (a rope's creak and the hull's knock in the mid band: the rumble alone was lost on a small
# speaker; sound designer round 3, #6)
rub = formants(glottal(150 + 30 * np.sin(2 * np.pi * 1.3 * t), n, 0.8), [500, 1200, 2400], 110) * np.exp(-t / 0.5) * 0.7
k = int(0.55 * SR)
tt = t[: n - k]
slap = np.zeros(n)
slap[k:] = bp(rng.normal(size=n - k), 400, 3000) * np.exp(-tt / 0.06) + (np.sin(2 * np.pi * 380 * tt) + 0.5 * np.sin(2 * np.pi * 950 * tt)) * np.exp(-tt / 0.05)
save("dock", (rub + slap) * env(n, 0.03, 0.2))

# catapult: the arm's twang and creak, then the stone's whoosh going away
n = int(SR * 0.9)
t = np.arange(n) / SR
twang = np.sin(2 * np.pi * 110 * t * (1 - 0.4 * t)) * np.exp(-t / 0.1) + bp(rng.normal(size=n), 400, 3000) * np.exp(-t / 0.03) * 0.8
whoosh = bp(rng.normal(size=n), 300, 2500) * np.clip((t - 0.08) / 0.1, 0, 1) * np.exp(-np.clip(t - 0.2, 0, None) / 0.2) * 0.5
save("catapult", (twang + whoosh) * env(n, 0.001, 0.2))

# brew: a kettle bubbling, low pops in a warm hiss
n = int(SR * 1.4)
t = np.arange(n) / SR
x = bp(rng.normal(size=n), 100, 900) * 0.15
for _ in range(14):
    k = rng.integers(0, n - 4000)
    f = rng.uniform(180, 420)
    m = int(SR * 0.05)
    tt = np.arange(m) / SR
    x[k:k + m] += np.sin(2 * np.pi * f * tt * (1 + 3 * tt)) * np.exp(-tt / 0.012)
save("brew", x * env(n, 0.1, 0.3))

# pig: two short grunts, a nasal low voice
parts = []
for f in (110, 95):
    m = int(SR * 0.22)
    tt = np.arange(m) / SR
    parts += [voice(f * (1 + 0.15 * np.sin(np.pi * tt / tt[-1])), m, 0.5, [400, 900, 2200], 150, 0.01, 0.05), np.zeros(int(SR * 0.12))]
save("pig", np.concatenate(parts))

# promote: a coin rings on a breastplate, a bright bell-like ding with a lower partial
n = int(SR * 0.9)
t = np.arange(n) / SR
x = sum(a * np.sin(2 * np.pi * f * t) * np.exp(-t / d) for f, a, d in ((1318, 1.0, 0.25), (2637, 0.4, 0.12), (3520, 0.25, 0.08), (659, 0.3, 0.35)))
save("promote", x * env(n, 0.002, 0.2))

# setdown: a carrier puts a load down, a soft wooden knock on the ground and a little grit
n = int(SR * 0.35)
t = np.arange(n) / SR
x = np.sin(2 * np.pi * 140 * t * (1 - 0.6 * t)) * np.exp(-t / 0.04) + bp(rng.normal(size=n), 500, 4000) * np.exp(-t / 0.02) * 0.3
save("setdown", x * env(n, 0.001, 0.1))

# setdown0-2: a load set down on boards, a hollow wooden knock in the mid band with a little
# grit, three of them (sound designer round 3, #5 and #6: the first was all under 200 Hz, a
# sub-bass tick nobody heard)
for v, (f1, f2) in enumerate(((420, 1250), (360, 1100), (500, 1450))):
    n = int(SR * 0.3)
    t = np.arange(n) / SR
    knock = (np.sin(2 * np.pi * f1 * t) * 0.7 + np.sin(2 * np.pi * f2 * t) * 0.4) * np.exp(-t / 0.035)
    thud = np.sin(2 * np.pi * 150 * t) * np.exp(-t / 0.05) * 0.4
    grit = bp(rng.normal(size=n), 800, 5000) * np.exp(-t / 0.015) * 0.35
    save(f"setdown{v}", (knock + thud + grit) * env(n, 0.001, 0.08))

# grunt0-2: a short voiced cry to lay over the death recording's body-fall, so a death is
# heard on a small speaker too (it dropped 23-26 dB through a 250 Hz high-pass)
for v, f in enumerate((170, 150, 195)):
    n = int(SR * 0.32)
    t = np.arange(n) / SR
    f0 = f * (1.0 - 0.35 * t / t[-1])
    save(f"grunt{v}", voice(f0, n, 0.4, [600, 1150, 2500], 130, 0.01, 0.12))

# promote2: a struck bell, inharmonic partials over a soft knock, not a test-tone sine (sound
# designer round 3, #6); news_nag: one soft note that starts no melody (the nag began like
# good news)
n = int(SR * 1.2)
t = np.arange(n) / SR
bell = sum(a * np.sin(2 * np.pi * 660 * r * t) * np.exp(-t / d) for r, a, d in ((1.0, 1.0, 0.5), (2.76, 0.5, 0.25), (5.4, 0.3, 0.12), (8.93, 0.15, 0.06)))
knock = bp(rng.normal(size=n), 1500, 6000) * np.exp(-t / 0.01) * 0.4
save("promote", (bell + knock) * env(n, 0.001, 0.3))
n = int(SR * 0.6)
t = np.arange(n) / SR
save("news", (np.sin(2 * np.pi * 740 * t) + 0.25 * np.sin(2 * np.pi * 1480 * t)) * np.exp(-t / 0.18) * env(n, 0.004, 0.1))
