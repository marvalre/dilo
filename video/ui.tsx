import type { CSSProperties, ReactNode } from "react";
import { MascotFace, type Mode, type Skin } from "../src/shared/MascotFace";
import speech from "./speech.json";
import T from "./timeline.json";
import { clamp, easeOutBack, easeOutExpo, lerp, prog } from "./anim";

export const FONT = `-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif`;
export const INK = "#1d1d1f";
export const MUTED = "#6e6e73";
export const WHITE = "#f5f5f7";
export const BLUE = "#0a84ff";
export const VIOLET = "#b36bff";

export const gradientText: CSSProperties = {
  background: "linear-gradient(100deg, #5E9BFF 0%, #B36BFF 55%, #FF7AB6 100%)",
  WebkitBackgroundClip: "text",
  backgroundClip: "text",
  color: "transparent",
};

/** Voice level (0..1) of the demo sentence at time t; 0 outside the speech window. */
export function speechLevel(t: number): number {
  const { speechStart, speechEnd } = T.marks.demo;
  if (t < speechStart || t > speechEnd) return 0;
  const f = ((t - speechStart) / (speechEnd - speechStart)) * (speech.length - 1);
  const i = Math.floor(f);
  return lerp(speech[i], speech[Math.min(speech.length - 1, i + 1)], f - i);
}

/** A soft idle "breathing" voice level for logo moments. */
export function idleLevel(t: number, bumps: number[] = []): number {
  let l = 0.1 + 0.06 * Math.sin(t * 5.3) + 0.04 * Math.sin(t * 11.7 + 1);
  for (const b of bumps) l += 0.7 * Math.exp(-Math.pow((t - b - 0.08) / 0.11, 2)) * (t >= b ? 1 : 0);
  return clamp(l, 0, 1);
}

export function Mascot({
  t,
  level,
  skin = "aura",
  size,
  mode = "listening",
  uid,
}: {
  t: number;
  level: number;
  skin?: Skin;
  /** Rendered scale relative to the skin's native viewBox size (1 = native px). */
  size: number;
  mode?: Mode;
  uid: string;
}) {
  const ms = t * 1000;
  const blink = ms % 3300 < 120;
  return <MascotFace skin={skin} size="s" scale={size} mode={mode} level={level} t={ms} blink={blink} uid={uid} />;
}

/** Words that rise out of a blur one after another. */
export function Words({
  t,
  words,
  style,
  gap = "0.26em",
  dur = 0.75,
  rise = 38,
  blur = 16,
}: {
  t: number;
  words: { w: ReactNode; at: number; style?: CSSProperties }[];
  style?: CSSProperties;
  gap?: string;
  dur?: number;
  rise?: number;
  blur?: number;
}) {
  return (
    <div style={{ display: "flex", flexWrap: "wrap", justifyContent: "center", ...style }}>
      {words.map((x, i) => {
        const p = easeOutExpo(prog(t, x.at, x.at + dur));
        return (
          <span
            key={i}
            style={{
              display: "inline-block",
              marginRight: i < words.length - 1 ? gap : 0,
              opacity: p,
              transform: `translateY(${(1 - p) * rise}px)`,
              filter: p < 1 ? `blur(${(1 - p) * blur}px)` : undefined,
              ...x.style,
            }}
          >
            {x.w}
          </span>
        );
      })}
    </div>
  );
}

/** Letters that pop in one by one (wordmark). */
export function Letters({
  t,
  text,
  at,
  style,
  dur = 0.6,
}: {
  t: number;
  text: string;
  at: number[];
  style?: CSSProperties;
  dur?: number;
}) {
  return (
    <div style={{ display: "flex", justifyContent: "center", ...style }}>
      {[...text].map((c, i) => {
        const p = prog(t, at[i] ?? at[at.length - 1], (at[i] ?? at[at.length - 1]) + dur);
        const e = easeOutBack(p, 1.5);
        return (
          <span
            key={i}
            style={{
              display: "inline-block",
              opacity: easeOutExpo(p * 1.6),
              transform: `translateY(${(1 - e) * 70}px) scale(${lerp(0.86, 1, e)})`,
              filter: p < 1 ? `blur(${(1 - easeOutExpo(p)) * 14}px)` : undefined,
            }}
          >
            {c}
          </span>
        );
      })}
    </div>
  );
}

export const fill: CSSProperties = { position: "absolute", left: 0, top: 0, width: "100%", height: "100%" };

/** Visibility + transition style for a scene living in [inA, outB]. */
export function sceneStyle(t: number, inA: number, inB: number, outA: number, outB: number): CSSProperties {
  const pin = easeOutExpo(prog(t, inA, inB));
  const pout = prog(t, outA, outB);
  const op = pin * (1 - pout);
  return {
    ...fill,
    opacity: op,
    visibility: op <= 0.001 ? "hidden" : "visible",
    transform: `scale(${lerp(1.03, 1, pin) * lerp(1, 1.035, pout)})`,
    filter: pout > 0 ? `blur(${pout * 10}px)` : undefined,
  };
}
