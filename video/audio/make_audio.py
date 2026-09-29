#!/usr/bin/env python3
"""Dilo launch video audio: music bed + sound design. Usage: python3 make_audio.py  (reads ../timeline.json)"""
import json, os, re, subprocess, tempfile, wave
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
TL = json.load(open(os.path.join(HERE, '..', 'timeline.json')))
SR = 44100
DUR = TL['duration']
N = int(round(DUR * SR))
BEAT = 60.0 / TL['bpm']
M = TL['marks']
rng = np.random.default_rng(7)
TT = np.arange(N) / SR

# ------------------------------------------------------------ helpers
def mf(m): return 440.0 * 2 ** ((m - 69) / 12)
def note_midi(s):
    m = re.match(r'([A-G])(#|b)?(\d)', s); base = {'C': 0, 'D': 2, 'E': 4, 'F': 5, 'G': 7, 'A': 9, 'B': 11}[m.group(1)]
    base += {'#': 1, 'b': -1, None: 0}[m.group(2)]
    return 12 * (int(m.group(3)) + 1) + base

def filt(x, lo=None, hi=None, order=4):
    """zero-phase FFT lowpass(hi)/highpass(lo), works on last... 1-D or (n,2)"""
    x = np.asarray(x, float)
    n = x.shape[0]; L = n + n // 8
    f = np.fft.rfftfreq(L, 1 / SR); H = np.ones_like(f)
    if hi: H = H / np.sqrt(1 + (f / hi) ** order)
    if lo: H = H / np.sqrt(1 + (lo / np.maximum(f, 1e-3)) ** order)
    if x.ndim == 1: return np.fft.irfft(np.fft.rfft(x, L) * H, L)[:n]
    return np.stack([filt(x[:, c], lo, hi, order) for c in range(x.shape[1])], 1)

def tt(n): return np.arange(n) / SR
def fade_in(x, s=0.002):
    k = min(len(x), int(s * SR)); x = x.copy(); x[:k] *= np.linspace(0, 1, k) ** 2 if x.ndim == 1 else (np.linspace(0, 1, k) ** 2)[:, None]; return x
def fade_out(x, s=0.01):
    k = min(len(x), int(s * SR)); x = x.copy(); r = np.linspace(1, 0, k) ** 2
    x[-k:] *= r if x.ndim == 1 else r[:, None]; return x
def peak(x): return float(np.max(np.abs(x))) if len(x) else 0.0
def norm(x, p): return x * (p / (peak(x) + 1e-12))
def st(x, pan=0.5):
    """mono -> stereo with constant-power pan (0=L,1=R)"""
    return np.stack([x * np.cos(pan * np.pi / 2), x * np.sin(pan * np.pi / 2)], 1)
def place(bus, sig, t, g=1.0):
    i = int(round(t * SR))
    if i >= N or i + len(sig) <= 0: return
    j = min(N, i + len(sig)); bus[i:j] += sig[:j - i] * g

def make_ir(rt60, seed, lo=180, hi=5000, pre=0.012):
    r = np.random.default_rng(seed); n = int(SR * rt60 * 1.1)
    ir = np.zeros((n + int(pre * SR), 2))
    t = tt(n)
    for c in range(2):
        d = r.standard_normal(n) * np.exp(-6.9 * t / rt60)
        d *= np.minimum(1, t / 0.02)
        ir[int(pre * SR):, c] = filt(d, lo=lo, hi=hi, order=2)
    return ir / np.sqrt(np.sum(ir ** 2) / 2)

def convolve(x, ir):
    L = len(x) + len(ir); out = np.zeros((len(x), 2))
    for c in range(2):
        xc = x[:, c] if x.ndim == 2 else x
        # both IR channels mixed by input channel -> keep decorrelated: use ir[:,c]
        out[:, c] = np.fft.irfft(np.fft.rfft(xc, L) * np.fft.rfft(ir[:, c], L), L)[:len(x)]
    return out

def band_noise(n, fc_fn, bw=0.9, r=None):
    """time-varying band-pass noise via STFT; fc_fn(x in 0..1) -> Hz"""
    r = r or rng; fr, hop = 1024, 256
    x = r.standard_normal(n + 2 * fr); y = np.zeros_like(x); w = np.hanning(fr)
    f = np.maximum(np.fft.rfftfreq(fr, 1 / SR), 1.0)
    for s in range(0, n + fr, hop):
        fc = fc_fn(min(1, max(0, (s + fr / 2 - fr / 2) / max(n, 1))))
        H = np.exp(-0.5 * (np.log2(f / fc) / bw) ** 2)
        y[s:s + fr] += np.fft.irfft(np.fft.rfft(x[s:s + fr] * w) * H, fr)
    return y[fr // 2:fr // 2 + n] / 2

def beta_env(n, a, b):
    x = np.linspace(0, 1, n); e = x ** a * (1 - x) ** b; return e / e.max()

# ------------------------------------------------------------ SFX synth (mono/stereo unit signals, normalised later)
PENT = [0, 2, 4, 7, 9]          # C major pentatonic steps
def pent_freq(step, base=1046.5): return base * 2 ** (PENT[int(step) % 5] / 12) * (2 if step >= 5 else 1)

def s_tick(pitch=0, **k):
    f = pent_freq(pitch); n = int(0.09 * SR); t = tt(n)
    x = np.sin(2 * np.pi * f * t) * np.exp(-t / 0.018) + 0.35 * np.sin(2 * np.pi * f * 2.76 * t) * np.exp(-t / 0.008) \
        + 0.5 * np.sin(2 * np.pi * f * 0.5 * t) * np.exp(-t / 0.012)
    x += filt(rng.standard_normal(n), lo=2500, hi=6000) * np.exp(-t / 0.002) * 0.25
    return st(fade_in(x, 0.0006), 0.5)

def s_thud(**k):
    n = int(0.35 * SR); t = tt(n); ph = 2 * np.pi * np.cumsum(50 + 60 * np.exp(-t * 35)) / SR
    x = np.sin(ph) * np.exp(-t / 0.09) + filt(rng.standard_normal(n), hi=400) * np.exp(-t / 0.012) * 0.5
    x = np.tanh(1.4 * x) + 0.2 * np.sin(2 * ph) * np.exp(-t / 0.05)   # a bit of harmonics for small speakers
    return st(fade_in(x, 0.002))

def s_whoosh(dur=0.45, dir='up', **k):
    n = int(dur * SR) + int(0.15 * SR)
    if dir == 'up': fc = lambda x: 350 * (4200 / 350) ** (x ** 1.3); env = beta_env(n, 1.8, 0.7); pan = np.linspace(0.2, 0.8, n)
    elif dir == 'down': fc = lambda x: 4200 * (350 / 4200) ** (x ** 0.8); env = beta_env(n, 0.6, 1.6); pan = np.linspace(0.8, 0.2, n)
    else: fc = lambda x: 3200 * (450 / 3200) ** x; env = beta_env(n, 2.2, 0.8); pan = np.full(n, 0.5)
    c = band_noise(n, fc, 0.9); a = band_noise(n, fc, 0.9); b = band_noise(n, fc, 0.9)
    gl, gr = np.cos(pan * np.pi / 2), np.sin(pan * np.pi / 2)
    L = (0.75 * c * gl + 0.45 * a) * env; R = (0.75 * c * gr + 0.45 * b) * env
    x = np.stack([L, R], 1) if dir != 'in' else np.stack([c * 0.8 + 0.3 * a, c * 0.8 + 0.3 * b], 1) * env[:, None]
    return fade_out(fade_in(x, 0.004), 0.03)

def s_boom(**k):
    n = int(3.0 * SR); t = tt(n)
    f = 36 + 40 * np.exp(-t * 7); ph = 2 * np.pi * np.cumsum(f) / SR
    sub = np.sin(ph) * np.exp(-t / 0.9)
    sub = np.tanh(1.6 * sub) * 0.8 + 0.25 * np.sin(2 * ph) * np.exp(-t / 0.5)
    air = np.zeros((n, 2))
    for c in range(2):
        air[:, c] = filt(rng.standard_normal(n), lo=250, hi=3500, order=2) * np.exp(-t / 0.7) * (1 - np.exp(-t / 0.06)) * 0.18
    thump = filt(rng.standard_normal(n), hi=700) * np.exp(-t / 0.02) * 0.4
    x = st(sub + thump, 0.5) + air
    return fade_out(fade_in(x, 0.003), 0.25)

def s_pop(pitch=0, **k):
    m = 1.0 * 2 ** (PENT[int(pitch) % 5] / 12)
    n = int(0.25 * SR); t = tt(n)
    f = (420 + 520 * (1 - np.exp(-t * 60))) * m; body = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / 0.045)
    sp = sum(np.sin(2 * np.pi * fr * m * t) * np.exp(-t / d) * a for fr, d, a in [(2637, 0.09, 0.22), (3520, 0.06, 0.14), (1760, 0.12, 0.2)])
    x = fade_in(body + sp, 0.002); return st(x, 0.5)

def s_chime(note='A5', **k):
    f = mf(note_midi(note)); n = int(3.0 * SR); t = tt(n)
    x = sum(a * np.sin(2 * np.pi * f * r * t + ph) * np.exp(-t / d) for r, a, d, ph in
            [(1, 1.0, 1.3, 0), (2.0, 0.30, 0.8, 0.5), (3.0, 0.10, 0.4, 1), (4.01, 0.05, 0.25, 2), (0.5, 0.15, 0.6, 0)])
    x = fade_in(x, 0.003)
    return fade_out(np.stack([x, np.roll(x, 40)], 1) * 0.7 + 0.3 * x[:, None], 0.3)

def s_click(**k):
    def stage(freq, g, bright, dur=0.03):
        n = int(dur * SR); t = tt(n)
        return (np.sin(2 * np.pi * freq * t) * np.exp(-t / 0.008) * 0.8 + filt(rng.standard_normal(n), lo=bright, hi=bright * 3.5) * np.exp(-t / 0.002)) * g
    out = np.zeros(int(0.12 * SR)); a = stage(210, 1.0, 1200); b = stage(300, 0.55, 2000, 0.02)
    out[:len(a)] += a; out[int(0.06 * SR):int(0.06 * SR) + len(b)] += b
    return st(fade_in(out, 0.0005), 0.5)

def s_key(down=True, **k):
    f, br, g = (150, 900, 1.0) if down else (230, 1600, 0.5)
    n = int(0.12 * SR); t = tt(n)
    x = np.sin(2 * np.pi * f * t) * np.exp(-t / 0.02) + filt(rng.standard_normal(n), lo=br, hi=br * 3.5) * np.exp(-t / 0.006) * 0.9 \
        + np.sin(2 * np.pi * (2900 if down else 3400) * t) * np.exp(-t / 0.004) * 0.15
    return st(fade_in(x, 0.0005) * g, 0.5)

def s_paste(**k):
    n = int(1.0 * SR); t = tt(n)
    thup = np.sin(2 * np.pi * np.cumsum(70 + 80 * np.exp(-t * 45)) / SR) * np.exp(-t / 0.06) + filt(rng.standard_normal(n), hi=900) * np.exp(-t / 0.015) * 0.6
    sh = np.zeros(n)
    for i, fr in enumerate([2093, 2637, 3136, 3951]):
        d = int(i * 0.028 * SR); tk = t[:n - d]
        sh[d:] += np.sin(2 * np.pi * fr * tk) * np.exp(-tk / 0.09) * 0.28
    x = thup * 1.0 + sh * 0.6
    return fade_out(fade_in(st(x, 0.5), 0.001), 0.05)

def s_lock(**k):
    n = int(0.25 * SR); t = tt(n); x = np.zeros(n)
    def m(t0, fs, d, g):
        i = int(t0 * SR); tk = t[:n - i]
        x[i:] += sum(np.sin(2 * np.pi * f * tk) * np.exp(-tk / d) * a for f, a in fs) * g
    m(0.0, [(2400, 1), (3700, 0.5)], 0.008, 0.7)
    m(0.05, [(230, 1), (460, 0.3)], 0.035, 1.0)
    x[:int(0.004 * SR)] += filt(rng.standard_normal(int(0.004 * SR)), lo=1500) * 0.2
    return st(fade_in(x, 0.0005), 0.5)

def s_swap(**k):
    n = int(0.35 * SR)
    sw = band_noise(int(0.14 * SR), lambda x: 1500 * (5500 / 1500) ** x, 0.8) * beta_env(int(0.14 * SR), 1.2, 1.2)
    x = np.zeros((n, 2)); x[:len(sw)] += st(sw * 0.7, 0.5)
    tk = s_tick(3)[:, 0]; x[int(0.11 * SR):int(0.11 * SR) + len(tk)] += st(tk * 0.9, 0.5)
    return fade_out(x, 0.02)

def s_chip(**k):
    f = 1250 * 2 ** (PENT[rng.integers(0, 5)] / 12) * (1 + rng.uniform(-0.03, 0.03)); n = int(0.15 * SR); t = tt(n)
    x = (np.sin(2 * np.pi * f * t) + 0.25 * np.sin(2 * np.pi * f * 2.4 * t) * np.exp(-t / 0.01)) * np.exp(-t / 0.022)
    return st(fade_in(x, 0.001), rng.uniform(0.35, 0.65))

def s_swallow(**k):
    n = int(0.32 * SR); t = tt(n)
    w = band_noise(n, lambda x: 2000 * (450 / 2000) ** (x ** 0.8), 0.8) * beta_env(n, 1.5, 0.9)
    g = np.sin(2 * np.pi * np.cumsum(420 * np.exp(-t * 4.5)) / SR) * np.exp(-((t - 0.19) / 0.05) ** 2) * 0.5 
    x = w / (peak(w) + 1e-9) * 0.6 + g
    return fade_out(fade_in(st(x, 0.5), 0.004), 0.03)

def s_resolve(**k):
    n = int((DUR - 26.0) * SR) + 10; t = tt(n)
    notes = [(36, .8), (43, .5), (52, .55), (55, .5), (59, .45), (62, .4), (64, .35), (71, .18), (74, .16)]
    out = np.zeros((n, 2))
    for i, (m, a) in enumerate(notes):
        for det, pan in ((-0.004, 0.25), (0.0, 0.5), (0.004, 0.75)):
            f = mf(m) * (1 + det)
            v = np.sin(2 * np.pi * f * t) * 0.7 + (2 * ((t * f) % 1) - 1) * 0.25 * (1 if m > 40 else 0.3)
            out += st(v * a / 3, 0.15 + 0.7 * ((i * 0.37 + pan) % 1))
    out = filt(out, hi=2600, order=2)
    swell = (1 - np.exp(-t / 0.45)) * (0.55 + 0.45 * np.exp(-t / 1.2))          # bloom then settle
    tail = 0.5 * (1 + np.cos(np.pi * np.clip((t - 1.0) / (t[-1] - 1.0), 0, 1)))  # sustain, fade out by 29.9
    return out * (swell * tail)[:, None]

SYN = dict(tick=s_tick, thud=s_thud, whoosh=s_whoosh, boom=s_boom, pop=s_pop, chime=s_chime, click=s_click,
           keydown=lambda **k: s_key(True), keyup=lambda **k: s_key(False), paste=s_paste, lock=s_lock, swap=s_swap,
           chip=s_chip, swallow=s_swallow, resolve=s_resolve)
# target peak (linear, pre-master) and reverb send
PEAK = dict(tick=0.17, thud=0.34, whoosh=0.20, boom=0.45, pop=0.20, chime=0.19, click=0.17, keydown=0.16, keyup=0.14,
            paste=0.22, lock=0.20, swap=0.17, chip=0.075, swallow=0.14, resolve=0.17)
SEND = dict(tick=0.10, thud=0.10, whoosh=0.15, boom=0.35, pop=0.35, chime=0.5, click=0.0, keydown=0.0, keyup=0.0,
            paste=0.3, lock=0.05, swap=0.1, chip=0.25, swallow=0.05, resolve=0.5)

def build_sfx():
    ev = list(TL['sfx'])
    f3 = M['f3']
    for i in range(f3['chips']): ev.append(dict(t=f3['chipsFrom'] + i * f3['chipEvery'], type='chip'))
    ev.append(dict(t=M['demo']['keyup'] + 0.05, type='swallow'))
    dry = np.zeros((N, 2)); snd = np.zeros((N, 2)); stats = {}
    for e in sorted(ev, key=lambda e: e['t']):
        ty = e['type']; kw = {k: v for k, v in e.items() if k not in ('t', 'type', 'gain')}
        sig = SYN[ty](**kw)
        g = e.get('gain', 1.0)
        # per-event level: tick scales gently with pitch so higher pitches are not piercing
        if ty == 'tick': g *= 1.0 - 0.06 * kw.get('pitch', 0)
        sig = norm(sig, PEAK[ty] * g)
        if ty == 'boom' or ty == 'resolve': pass
        place(dry, sig, e['t']); place(snd, sig, e['t'], SEND[ty])
        stats.setdefault(ty, []).append((e['t'], peak(sig)))
    wet = convolve(snd, make_ir(1.3, 11, lo=250, hi=6000)) * 0.9
    out = dry + wet
    return out, stats, ev

# ------------------------------------------------------------ MUSIC
CH = {  # (root midi for sub, pad notes, arp notes)
    'F':  (29, [53, 57, 60, 64], [65, 69, 72, 76]),      # Fmaj7
    'Am': (33, [55, 57, 60, 64], [69, 72, 76, 79]),      # Am7
    'C9': (36, [55, 59, 62, 64], [67, 71, 74, 76]),      # Cmaj9
    'G6': (31, [59, 62, 64, 67], [67, 71, 74, 76]),      # G6
}
PROG = [(0, 5, 'F'), (5, 10, 'Am'), (10, 15, 'C9'), (15, 20, 'G6'), (20, 22.5, 'F'), (22.5, 26.0, 'G6'), (26.0, DUR + 2, 'C9')]

def kick(g=1.0):
    n = int(0.45 * SR); t = tt(n); ph = 2 * np.pi * np.cumsum(46 + 90 * np.exp(-t * 32)) / SR
    return fade_in(np.sin(ph) * np.exp(-t / 0.11) * g, 0.002)
def hat(g=1.0):
    n = int(0.08 * SR); t = tt(n); return filt(rng.standard_normal(n), lo=6500, hi=11000, order=2) * np.exp(-t / 0.018) * g
def pluck(m, g=1.0, d=0.22):
    n = int(0.9 * SR); t = tt(n); f = mf(m)
    x = np.sin(2 * np.pi * f * t) + 0.35 * np.sin(2 * np.pi * 2 * f * t) * np.exp(-t / 0.08) + 0.12 * np.sin(2 * np.pi * 3 * f * t) * np.exp(-t / 0.04)
    return fade_in(x * np.exp(-t / d) * g, 0.002)
def padnote(m, t0, t1, g):
    n = int((t1 - t0 + 1.0) * SR); t = tt(n); L = np.zeros(n); R = np.zeros(n); f = mf(m)
    for det, w in ((-0.0045, 0), (0.0, 1), (0.0045, 2)):
        v = 2 * (((t * f * (1 + det)) + rng.random()) % 1) - 1
        (L if w != 2 else R)[:] += v * (1 if w == 1 else 0.8); 
        if w == 1: R += v * 0.6
    L, R = filt(L, hi=850, order=2), filt(R, hi=850, order=2)
    s = np.sin(2 * np.pi * f * t) * 1.2; L += s; R += s
    d = t1 - t0
    e = np.minimum(1, t / 0.9) * np.where(t < d, 1, np.exp(-(t - d) / 0.35))
    return np.stack([L, R], 1) * (e * g)[:, None]

def build_music():
    dry = np.zeros((N, 2)); snd = np.zeros((N, 2)); pad = np.zeros((N, 2)); sub = np.zeros(N)
    # drone: low F, fades in 0 -> 3.1, stays under everything
    dr = np.sin(2 * np.pi * mf(29) * TT) + 0.5 * np.sin(2 * np.pi * mf(41) * TT + 1) + 0.25 * np.sin(2 * np.pi * mf(48) * 1.002 * TT)
    dr *= np.clip(TT / 3.0, 0, 1) ** 2 * np.where(TT < 3.125, 1, 0.55)
    # (drone is F; through the piece it is folded under chords through the sub instead)
    dr *= np.clip(1 - (TT - 6.25) / 2.0, 0.0, 1.0) * (TT < 8.5) + 0.0
    dry += st(dr * 0.12, 0.5)
    t_pad0 = M['reveal']['mascot'] - 0.175  # 3.125
    for (a, b, name) in PROG:
        root, pn, an = CH[name]
        if b <= t_pad0 or a >= DUR: continue
        a2 = max(a, t_pad0)
        for m in pn:
            place(pad, padnote(m, a2 - 0.0, min(b, DUR) + 0.0, 0.05), a2 - 0.15 if a2 > t_pad0 else a2)
        # sub: one note per bar (2.5 s) from reveal on
        tt0 = a2
        while tt0 < min(b, DUR):
            l = min(2.5, min(b, DUR) - tt0) + 0.25; n = int(l * SR); t = tt(n)
            v = (np.sin(2 * np.pi * mf(root) * t) + 0.18 * np.sin(2 * np.pi * mf(root + 12) * t)) * np.minimum(1, t / 0.04) * np.exp(-t / 3.0)
            v *= np.minimum(1, (l - t) / 0.25); place(sub, v, tt0); tt0 += 2.5
    pad *= np.clip((TT - t_pad0) / 1.4, 0, 1)[:, None] ** 2
    sub *= np.clip((TT - t_pad0) / 1.0, 0, 1)
    # rhythmic section
    pulse0, pulse1 = TL['scenes']['demo'][0], TL['scenes']['outro'][0]
    feat0 = TL['scenes']['features'][0]
    kicks = []
    k = pulse0
    while k < pulse1 - 1e-6:
        kicks.append(k); k += BEAT
    duck = np.ones(N)
    cards = [15.0, 17.5, 20.0, 22.5]
    for kt in kicks:
        strong = any(abs(kt - c) < 0.02 for c in cards) or abs(kt - pulse0) < 0.02
        g = 0.4 if not strong else 0.55
        if kt >= feat0 and not strong: g = 0.45
        place(dry, st(kick(g), 0.5), kt)
        i = int(kt * SR); j = min(N, i + int(0.5 * SR))
        depth = 0.45 if kt >= feat0 else 0.3
        tk = np.arange(j - i) / SR; duck[i:j] = np.minimum(duck[i:j], 1 - depth * np.exp(-tk / 0.16) * np.minimum(1, tk / 0.004 + 0.0))
        # hats on off-beats
        place(dry, st(hat(0.045 if kt < feat0 else 0.06), 0.62), kt + BEAT / 2)
        if kt >= feat0: place(dry, st(hat(0.03), 0.4), kt + BEAT / 4 * 3 - 0.0 + 0.0)
    # card-start stab (chord hit) on the four card starts + demo start
    for c in cards + [pulse0]:
        name = [nm for a, b, nm in PROG if a <= c < b][0]
        for m in CH[name][2]:
            place(snd, st(pluck(m - 12, 0.12, 0.45), 0.5), c)
    # arp (eighths)
    pats_demo = [0, None, 2, None, 1, None, 3, 2]
    pats_feat = [0, 1, 2, 3, 2, 1, 3, 2, 0, 2, 1, 3, 2, 3, 1, 2]
    e8 = BEAT / 2
    t = pulse0 + e8 * 0; i = 0
    while t < pulse1 - 1e-6:
        name = [nm for a, b, nm in PROG if a <= t + 1e-6 < b][0]; arp = CH[name][2]
        if t < feat0: idx = pats_demo[i % 8]; g = 0.22
        else: idx = pats_feat[i % 16]; g = 0.3
        if idx is not None:
            sig = st(pluck(arp[idx], g, 0.2), 0.25 + 0.5 * ((i * 0.618) % 1))
            place(snd, sig, t)
            if t >= feat0 and i % 4 == 0: place(snd, st(pluck(arp[idx] + 12, g * 0.35, 0.15), 0.7), t + 0.02)
        t += e8; i += 1
    # ping-pong delay on plucks (dotted 8th)
    dl = int(e8 * 1.5 * SR); d = np.zeros_like(snd); d[dl:] = snd[:-dl][:, ::-1] * 0.30; d[2 * dl:] += snd[:-2 * dl] * 0.14
    snd = snd + d
    # outro bloom: warm sustained Cmaj9 from 26.0 (music side, quiet, complements the 'resolve' sfx)
    # (already present via PROG's last segment)
    padd = pad * np.where((TT >= pulse0) & (TT < pulse1), duck, 1.0)[:, None]
    subd = sub * np.where((TT >= pulse0) & (TT < pulse1), duck, 1.0)
    rev = convolve(padd * 0.7 + snd, make_ir(2.2, 5, lo=200, hi=4500)) * 0.55
    out = dry + padd + st(subd * 0.22, 0.5) + snd * 0.9 + rev
    # global envelopes: fade-in at 0, ring-out to silence exactly at DUR
    x = np.clip((TT - 27.0) / (DUR - 27.0), 0, 1); tail = 0.5 * (1 + np.cos(np.pi * x)) ** 1.0
    out *= tail[:, None]
    out = fade_in(out, 0.05); out[-int(0.1 * SR):] *= np.linspace(1, 0, int(0.1 * SR))[:, None] ** 2
    out = filt(out, lo=25, hi=12000, order=2)
    return out

# ------------------------------------------------------------ loudness / IO
def write_wav(path, x):
    x = np.clip(x, -1, 1); pcm = (np.round(x * 32767)).astype('<i2')
    with wave.open(path, 'wb') as w: w.setnchannels(2); w.setsampwidth(2); w.setframerate(SR); w.writeframes(pcm.tobytes())

def ff_stats(x):
    """integrated LUFS, true peak dBTP via ffmpeg ebur128"""
    with tempfile.NamedTemporaryFile(suffix='.wav', delete=False) as f: p = f.name
    write_wav(p, x / max(1, peak(x)) if peak(x) > 1 else x)
    r = subprocess.run(['ffmpeg', '-hide_banner', '-nostats', '-i', p, '-af', 'ebur128=peak=true', '-f', 'null', '-'], capture_output=True, text=True).stderr
    os.unlink(p); s = r[r.rfind('Summary:'):]
    lufs = float(re.search(r'I:\s+(-?[\d.]+)\s+LUFS', s).group(1)); tp = float(re.search(r'Peak:\s+(-?[\d.]+|-inf)\s+dBFS', s).group(1))
    return lufs, tp

def limiter(x, ceil=0.88, look=0.004):
    a = np.max(np.abs(x), 1); g = np.minimum(1.0, ceil / np.maximum(a, 1e-9))
    w = int(look * SR) | 1
    pad = np.pad(g, (w // 2, w // 2), constant_values=1.0)
    gm = np.lib.stride_tricks.sliding_window_view(pad, w).min(axis=1)
    k = np.hanning(w + 2)[1:-1]; k /= k.sum()
    gs = np.convolve(np.pad(gm, (w // 2, w // 2), mode='edge'), k, mode='valid')[:N]
    return x * np.minimum(gs, g * 0 + 1)[:, None]

def main():
    print('music...'); music = build_music()
    lm, _ = ff_stats(music); music *= 10 ** ((-20.0 - lm) / 20)
    print('sfx...'); sfx, stats, ev = build_sfx()
    assert peak(music) < 0.98 and peak(sfx) < 0.98, (peak(music), peak(sfx))
    write_wav(os.path.join(HERE, 'music_only.wav'), music); write_wav(os.path.join(HERE, 'sfx_only.wav'), sfx)
    mix = music + sfx
    g = 1.0
    for _ in range(4):   # gain + limiter iterations to land on -14 LUFS
        y = limiter(mix * g); l, tp = ff_stats(y); g *= 10 ** ((-14.0 - l) / 20)
    y = limiter(mix * g); y[:int(0.01 * SR)] *= np.linspace(0, 1, int(0.01 * SR))[:, None]; y[-int(0.05 * SR):] *= np.linspace(1, 0, int(0.05 * SR))[:, None]
    l, tp = ff_stats(y)
    write_wav(os.path.join(HERE, 'dilo_audio.wav'), y)
    print(f'master gain {20*np.log10(g):+.2f} dB   LUFS {l:.2f}  true peak {tp:.2f} dBTP  sample peak {20*np.log10(peak(y)):.2f} dBFS')
    print('music -20 LUFS, sfx stem peak %.1f dBFS' % (20 * np.log10(peak(sfx))))
    print('\nper-type SFX peaks in final mix (dBFS), n events, min..max:')
    for ty, v in sorted(stats.items(), key=lambda kv: -max(p for _, p in kv[1])):
        ps = [20 * np.log10(p * g) for _, p in v]; print(f'  {ty:9s} n={len(v):2d}  {min(ps):6.1f} .. {max(ps):6.1f}')
    # measured windows in final mix
    print('\nfinal-mix peak in +-40ms windows at key times:')
    for t in (3.125, 8.0, 8.6, 12.05, 12.55, 15.9, 18.6, 24.45, 25.0, 26.0, 26.5):
        i = int(t * SR); print(f'  t={t:6.3f}  {20*np.log10(peak(y[i-1000:i+2000])+1e-9):6.1f} dBFS')

if __name__ == '__main__':
    main()
