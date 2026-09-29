// The mascot face, shared by the floating mascot window and the app's skin picker.
// Pure SVG; animation state (level, blink, mode) is driven by the parent.

export type Skin = "wave" | "glass" | "jolly" | "dot" | "aura";
export type MascotSize = "s" | "m" | "l";
export type Mode = "listening" | "thinking" | "done" | "sad" | "confused";

export const SKINS: { id: Skin; name: string; description: string }[] = [
  { id: "wave", name: "Wave", description: "Solo la onda de tu voz, sin carita" },
  { id: "glass", name: "Glass", description: "Cristal oscuro, el más Apple" },
  { id: "jolly", name: "Jolly", description: "Carita crema con mejillas" },
  { id: "dot", name: "Dot", description: "Mínimo y discreto" },
  { id: "aura", name: "Aura", description: "Halo que respira con tu voz" },
];

export const SIZE_SCALE: Record<MascotSize, number> = { s: 1.4, m: 1.8, l: 2.3 };

interface Palette {
  w: number; // viewBox width
  h: number; // viewBox height
  body: (id: string) => JSX.Element;
  ink: string;
  eyeRx: number;
  eyeRy: number;
  eyeY: number;
  eyeGap: number; // half distance between eye centers
  barsY: number;
  barW: number;
  barGap: number;
  barMax: number;
  cheeks?: boolean;
  bars?: number; // bar count (default 5)
  noFace?: boolean; // waveform only, no eyes
  halo?: boolean;
}

const PALETTES: Record<Skin, Palette> = {
  wave: {
    w: 46, h: 24, ink: "#fff", eyeRx: 0, eyeRy: 0, eyeY: 0, eyeGap: 0, barsY: 12, barW: 2, barGap: 1.3, barMax: 17, bars: 9, noFace: true,
    body: (id) => (
      <>
        <defs>
          <linearGradient id={id} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor="#2c2c2e" stopOpacity={0.96} />
            <stop offset="1" stopColor="#1c1c1e" stopOpacity={0.96} />
          </linearGradient>
        </defs>
        <rect x={1} y={1} width={44} height={22} rx={11} fill={`url(#${id})`} stroke="rgba(255,255,255,.22)" strokeWidth={0.6} />
      </>
    ),
  },
  glass: {
    w: 30, h: 24, ink: "#fff", eyeRx: 1.3, eyeRy: 1.7, eyeY: 9.4, eyeGap: 4, barsY: 15.6, barW: 1.2, barGap: 0.9, barMax: 5,
    body: (id) => (
      <>
        <defs>
          <linearGradient id={id} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor="#3a3a3c" stopOpacity={0.94} />
            <stop offset="1" stopColor="#1c1c1e" stopOpacity={0.94} />
          </linearGradient>
        </defs>
        <rect x={1} y={1} width={28} height={22} rx={11} fill={`url(#${id})`} stroke="rgba(255,255,255,.2)" strokeWidth={0.6} />
      </>
    ),
  },
  jolly: {
    w: 26, h: 26, ink: "#1d1d1f", eyeRx: 1.25, eyeRy: 1.6, eyeY: 11.2, eyeGap: 4, barsY: 16.4, barW: 1.1, barGap: 0.8, barMax: 5, cheeks: true,
    body: () => <circle cx={13} cy={13} r={12} fill="#F4EBDD" stroke="rgba(0,0,0,.1)" strokeWidth={0.5} />,
  },
  dot: {
    w: 20, h: 20, ink: "#fff", eyeRx: 0.95, eyeRy: 1.25, eyeY: 7.8, eyeGap: 3, barsY: 12.8, barW: 0.9, barGap: 0.6, barMax: 4,
    body: () => <circle cx={10} cy={10} r={9.3} fill="#1d1d1f" stroke="rgba(255,255,255,.28)" strokeWidth={0.6} />,
  },
  aura: {
    w: 26, h: 26, ink: "#1d1d1f", eyeRx: 1.05, eyeRy: 1.4, eyeY: 11, eyeGap: 3, barsY: 15.3, barW: 1, barGap: 0.7, barMax: 4.2, halo: true,
    body: () => <circle cx={13} cy={13} r={9.5} fill="#fff" />,
  },
};

const BAR_ENV = [0.55, 0.85, 1, 0.8, 0.5];
const envFor = (i: number, n: number) => (n === 5 ? BAR_ENV[i] : 0.3 + 0.7 * Math.sin((Math.PI * (i + 0.5)) / n));

/** Height of each mouth bar for a voice level (0..1) at time t (ms). */
export function barHeights(level: number, t: number, mode: Mode, p: { barW: number; barMax: number; bars?: number; noFace?: boolean }): number[] {
  const n = p.bars ?? 5;
  return Array.from({ length: n }, (_, i) => {
    const env = envFor(i, n);
    if (mode === "done" && p.noFace) return p.barW + (p.barMax * 0.4 - p.barW) * env;
    if (mode === "thinking") {
      // Gentle travelling "loading" wave.
      const wave = (Math.sin(t / 160 - i * 0.9) + 1) / 2;
      return p.barW + (p.barMax * 0.45 - p.barW) * wave;
    }
    if (mode !== "listening") return p.barW;
    const wobble = 0.6 + 0.4 * Math.sin(t / 90 + i * 1.7);
    return Math.max(p.barW, p.barW + (p.barMax - p.barW) * level * env * wobble);
  });
}

const palette = (skin: string): Palette =>
  Object.prototype.hasOwnProperty.call(PALETTES, skin) ? PALETTES[skin as Skin] : PALETTES.wave;

export function mascotBox(skin: Skin, size: MascotSize, scale?: number): { width: number; height: number } {
  const p = palette(skin);
  const s = scale ?? SIZE_SCALE[size] ?? SIZE_SCALE.m;
  return { width: p.w * s, height: p.h * s };
}

export function MascotFace({
  skin,
  size,
  mode,
  level,
  t,
  blink,
  uid,
  scale,
}: {
  skin: Skin;
  size: MascotSize;
  mode: Mode;
  level: number;
  t: number;
  blink: boolean;
  uid: string;
  scale?: number; // overrides `size` (px multiplier)
}) {
  const p = palette(skin);
  const { width, height } = mascotBox(skin, size, scale);
  const cx = p.w / 2;
  const heights = barHeights(level, t, mode, p);
  const total = heights.length * p.barW + (heights.length - 1) * p.barGap;
  const lookUp = mode === "thinking" ? -0.6 : 0;

  const eye = (x: number) => {
    if (mode === "done") {
      const r = p.eyeRx * 1.25;
      return <path d={`M${x - r} ${p.eyeY + 0.5} Q${x} ${p.eyeY - r * 1.3} ${x + r} ${p.eyeY + 0.5}`} fill="none" stroke={p.ink} strokeWidth={p.eyeRx * 0.8} strokeLinecap="round" />;
    }
    if (mode === "sad") {
      const r = p.eyeRx * 1.2;
      return <path d={`M${x - r} ${p.eyeY - 0.4} Q${x} ${p.eyeY + r} ${x + r} ${p.eyeY - 0.4}`} fill="none" stroke={p.ink} strokeWidth={p.eyeRx * 0.8} strokeLinecap="round" />;
    }
    return <ellipse cx={x + (mode === "thinking" ? 0.5 : 0)} cy={p.eyeY + lookUp} rx={p.eyeRx} ry={blink ? 0.2 : p.eyeRy} fill={p.ink} />;
  };

  return (
    <svg width={width} height={height} viewBox={`0 0 ${p.w} ${p.h}`} style={{ overflow: "visible", display: "block" }} aria-hidden>
      {p.halo && (
        <>
          <defs>
            <radialGradient id={`${uid}-halo`} cx=".5" cy=".5" r=".5">
              <stop offset=".55" stopColor="#5E9BFF" stopOpacity={0.55} />
              <stop offset="1" stopColor="#B36BFF" stopOpacity={0} />
            </radialGradient>
          </defs>
          <circle cx={cx} cy={p.h / 2} r={10.5 + (mode === "listening" ? level * 3.5 : 1)} fill={`url(#${uid}-halo)`} />
        </>
      )}
      <g style={{ filter: "drop-shadow(0 1px 2px rgba(0,0,0,.28))" }}>{p.body(`${uid}-body`)}</g>
      {!p.noFace && eye(cx - p.eyeGap)}
      {!p.noFace && eye(cx + p.eyeGap)}
      {p.cheeks && (
        <g fill="#FFB3A7" opacity={0.7}>
          <ellipse cx={cx - 6.4} cy={14.4} rx={1.6} ry={0.9} />
          <ellipse cx={cx + 6.4} cy={14.4} rx={1.6} ry={0.9} />
        </g>
      )}
      {mode === "done" && !p.noFace ? (
        <path
          d={`M${cx - 2.2} ${p.barsY - 0.6} Q${cx} ${p.barsY + 1.4} ${cx + 2.2} ${p.barsY - 0.6}`}
          fill="none"
          stroke={p.ink}
          strokeWidth={p.barW}
          strokeLinecap="round"
        />
      ) : (
        heights.map((h, i) => (
          <rect
            key={i}
            x={cx - total / 2 + i * (p.barW + p.barGap)}
            y={p.barsY - h / 2}
            width={p.barW}
            height={h}
            rx={p.barW / 2}
            fill={p.noFace && mode === "done" ? "#30D158" : p.ink}
          />
        ))
      )}
    </svg>
  );
}
