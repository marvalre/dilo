Dilo launch audio. `python3 make_audio.py` (numpy + ffmpeg only) reads ../timeline.json and writes
sfx_only.wav, music_only.wav (unmastered stems, music at -20 LUFS) and dilo_audio.wav (44.1k/16-bit stereo, 29.9 s, -14 LUFS, TP <= -1 dBTP).
Music: 96 bpm, Fmaj7 - Am7 - Cmaj9 - G6, then Fmaj7/G6 in the features and a Cmaj9 that rings out from 26.0 and fades to silence at 29.9.
0-3.1 s: only a low F drone fading in. From 3.125: detuned-saw pad (low-passed) plus sine sub. From 6.25: soft kick on beats,
off-beat hats (band-limited to 6.5-11 kHz, very low), sine/pluck arp in eighths with a dotted-eighth ping-pong delay, pad ducked under the kick.
Features: denser arp, stronger kick/chord stab on the four card starts (15.0/17.5/20.0/22.5). Outro: pulse stops.
Convolution reverb (noise IR) on pad/plucks; separate short reverb send on chimes, pops, boom, paste, resolve.
SFX are all synthesized (damped sines with inharmonic partials for ticks/chimes, filtered noise sweeps for whooshes panned along
their travel direction, tanh-thickened sine for boom/thud). Each type has a target peak so ticks sit ~10 dB under boom; ticks use a C pentatonic scale.
Mastering: gain + 4 ms look-ahead limiter iterated to -14 LUFS (ffmpeg ebur128 measured).
