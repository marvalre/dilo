import type { CSSProperties, ReactNode } from "react";
import T from "./timeline.json";
import type { Skin } from "../src/shared/MascotFace";
import { clamp, easeInCubic, easeInOutCubic, easeOutBack, easeOutCubic, easeOutExpo, lerp, prog, window4 } from "./anim";
import { BLUE, FONT, INK, MUTED, VIOLET, WHITE, Letters, Mascot, Words, fill, gradientText, idleLevel, sceneStyle, speechLevel } from "./ui";

const M = T.marks;

/* ────────────────────────────── 1 · Hook ────────────────────────────── */

export function Hook({ t }: { t: number }) {
  const h = M.hook;
  const dim = easeInOutCubic(prog(t, h.l2a - 0.35, h.l2a + 0.35));
  const punch = 1 + 0.1 * Math.exp(-Math.pow((t - h.l2b - 0.1) / 0.16, 2)) * (t >= h.l2b ? 1 : 0);
  const big: CSSProperties = { fontFamily: FONT, fontWeight: 700, fontSize: 176, letterSpacing: "-0.045em", color: WHITE, lineHeight: 1 };
  return (
    <div style={sceneStyle(t, 0, 0.2, h.out, T.scenes.hook[1] + 0.05)}>
      <div style={{ ...fill, background: "radial-gradient(900px 600px at 50% 55%, rgba(60,70,140,0.22), transparent 70%)" }} />
      <div style={{ position: "absolute", left: 0, width: "100%", top: 300 - 110 * dim, opacity: lerp(1, 0.28, dim) }}>
        <Words
          t={t}
          style={big}
          words={[
            { w: "Piensas", at: h.w1 },
            { w: "rápido.", at: h.w2, style: gradientText },
          ]}
        />
      </div>
      <div style={{ position: "absolute", left: 0, width: "100%", top: 560, transform: `scale(${punch})` }}>
        <Words
          t={t}
          style={{ ...big, fontSize: 210 }}
          words={[
            { w: "Escribir,", at: h.l2a },
            { w: "no.", at: h.l2b, style: { color: "#ff5c8a" } },
          ]}
        />
      </div>
    </div>
  );
}

/* ────────────────────────────── 2 · Reveal ────────────────────────────── */

export function Reveal({ t }: { t: number }) {
  const r = M.reveal;
  const pop = easeOutBack(prog(t, r.mascot, r.mascot + 0.8), 1.9);
  const ring = prog(t, r.mascot, r.mascot + 1.1);
  const glow = easeOutExpo(prog(t, r.mascot - 0.05, r.mascot + 1.2));
  const word: CSSProperties = { fontFamily: FONT, fontWeight: 700, color: WHITE, letterSpacing: "-0.055em", lineHeight: 1 };
  return (
    <div style={sceneStyle(t, T.scenes.reveal[0] - 0.02, T.scenes.reveal[0] + 0.05, r.out, T.scenes.reveal[1] + 0.05)}>
      <div
        style={{
          position: "absolute",
          left: 960 - 650,
          top: 400 - 650,
          width: 1300,
          height: 1300,
          borderRadius: "50%",
          background: "radial-gradient(circle, rgba(94,155,255,0.42) 0%, rgba(179,107,255,0.2) 32%, transparent 62%)",
          transform: `scale(${lerp(0.2, 1, glow)})`,
          opacity: glow,
        }}
      />
      <div
        style={{
          position: "absolute",
          left: 960 - 130,
          top: 400 - 130,
          width: 260,
          height: 260,
          borderRadius: "50%",
          border: "3px solid rgba(255,255,255,0.55)",
          transform: `scale(${lerp(0.6, 5.2, easeOutCubic(ring))})`,
          opacity: (1 - ring) * (t >= r.mascot ? 1 : 0) * 0.9,
        }}
      />
      <div style={{ position: "absolute", left: 960, top: 400, transform: `translate(-50%,-50%) scale(${pop})`, opacity: clamp(pop * 3) }}>
        <Mascot t={t} level={idleLevel(t, [r.t1, r.t2, r.t3])} size={9.6} uid="reveal" />
      </div>
      <Letters t={t} text="Dilo" at={r.letters} style={{ ...word, position: "absolute", left: 0, width: "100%", top: 560, fontSize: 250 }} />
      <Words
        t={t}
        style={{ ...word, position: "absolute", left: 0, width: "100%", top: 850, fontSize: 64, fontWeight: 500 }}
        gap="0.5em"
        words={[
          { w: "Mantén.", at: r.t1, style: { color: "rgba(245,245,247,0.78)" } },
          { w: "Habla.", at: r.t2, style: { color: "rgba(245,245,247,0.78)" } },
          { w: "Suelta.", at: r.t3, style: gradientText },
        ]}
      />
    </div>
  );
}

/* ────────────────────────────── 3 · Demo ────────────────────────────── */

const WIN = { x: 300, y: 170, w: 1320, h: 660 };
const CLICK = { x: 760, y: 560 };
const BODY = { x: WIN.x + 64, y: WIN.y + 64 + 62 + 62 + 40 };
const SENTENCE = "Hola Ana, te confirmo la reunión de mañana a las 5 de la tarde.";

function Cursor({ x, y, s = 1.35, pressed = 0 }: { x: number; y: number; s?: number; pressed?: number }) {
  const k = s * (1 - 0.1 * pressed);
  return (
    <svg
      width={30 * k}
      height={42 * k}
      viewBox="0 0 30 42"
      style={{ position: "absolute", left: x - 2 * k, top: y - 2 * k, overflow: "visible", filter: "drop-shadow(0 3px 5px rgba(0,0,0,.28))" }}
    >
      <path d="M2 2 L2 31 L9.2 24.4 L14.2 36.4 L19.4 34.2 L14.5 22.6 L24.2 22.4 Z" fill="#111" stroke="#fff" strokeWidth={2.4} strokeLinejoin="round" />
    </svg>
  );
}

function Caption({ t, a, b, words }: { t: number; a: number; b: number; words: { w: ReactNode; style?: CSSProperties }[] }) {
  const out = prog(t, b - 0.22, b);
  return (
    <div style={{ position: "absolute", left: 0, top: 68, width: "100%", opacity: 1 - out, transform: `translateY(${-14 * out}px)`, visibility: t < a || out >= 1 ? "hidden" : "visible" }}>
      <Words
        t={t}
        style={{ fontFamily: FONT, fontWeight: 650, fontSize: 64, letterSpacing: "-0.03em", color: INK }}
        gap="0.27em"
        dur={0.6}
        rise={26}
        blur={10}
        words={words.map((x, i) => ({ ...x, at: a + i * 0.075 }))}
      />
    </div>
  );
}

function Keycap({ t }: { t: number }) {
  const d = M.demo;
  const appear = easeOutExpo(prog(t, d.keydown - 0.35, d.keydown - 0.05));
  const press = easeOutCubic(prog(t, d.keydown, d.keydown + 0.08)) * (1 - easeOutCubic(prog(t, d.keyup, d.keyup + 0.12)));
  const fade = 1 - prog(t, d.keyup + 0.5, d.keyup + 0.9);
  const pulse = prog((t - d.keydown) % 0.9, 0, 0.9) * (t > d.keydown && t < d.keyup ? 1 : 0);
  return (
    <div style={{ position: "absolute", left: 960 - 75, top: 888, width: 150, height: 150, opacity: appear * fade, transform: `translateY(${(1 - appear) * 60}px)` }}>
      <div style={{ position: "absolute", inset: -14, borderRadius: 48, border: `3px solid ${BLUE}`, opacity: (1 - pulse) * 0.5 * press, transform: `scale(${1 + pulse * 0.5})` }} />
      <div
        style={{
          position: "absolute",
          inset: 0,
          borderRadius: 34,
          background: "linear-gradient(#ffffff, #eceef3)",
          boxShadow: `0 ${lerp(12, 3, press)}px 0 #c3c6d0, 0 ${lerp(30, 12, press)}px 40px rgba(20,30,70,${lerp(0.22, 0.14, press)}), inset 0 0 0 1.5px rgba(255,255,255,.9)`,
          transform: `translateY(${press * 9}px)`,
          fontFamily: FONT,
          color: INK,
        }}
      >
        <div style={{ position: "absolute", left: 18, top: 12, fontSize: 46, fontWeight: 500, color: press > 0.5 ? BLUE : INK }}>⌥</div>
        <div style={{ position: "absolute", left: 18, bottom: 14, fontSize: 24, fontWeight: 500, color: MUTED }}>option</div>
      </div>
    </div>
  );
}

export function Demo({ t }: { t: number }) {
  const d = M.demo;
  const [s0, s1] = T.scenes.demo;
  const cam = 1 + 0.05 * easeInOutCubic(prog(t, s0, s1));
  const camX = lerp(0, -30, prog(t, d.keydown, d.paste));

  // cursor path: quadratic bezier with a soft ease
  const cp = easeInOutCubic(prog(t, d.cursorFrom, d.cursorTo));
  const P0 = { x: 1700, y: 960 };
  const P1 = { x: 1180, y: 800 };
  const P2 = CLICK;
  const cx = (1 - cp) * (1 - cp) * P0.x + 2 * (1 - cp) * cp * P1.x + cp * cp * P2.x;
  const cy = (1 - cp) * (1 - cp) * P0.y + 2 * (1 - cp) * cp * P1.y + cp * cp * P2.y;
  const cursorIn = easeOutExpo(prog(t, d.cursorFrom - 0.25, d.cursorFrom + 0.2));
  const press = window4(t, d.click, d.click + 0.05, d.click + 0.08, d.click + 0.2);
  const ripple = prog(t, d.click, d.click + 0.55);

  // mascot next to the cursor (same offset idea as the real app)
  const mIn = easeOutBack(prog(t, d.keydown + 0.05, d.keydown + 0.65), 2.0);
  const sw = prog(t, d.keyup + 0.05, d.keyup + 0.37);
  const mScale = t < d.keyup + 0.05 ? mIn : lerp(1, 0.02, easeInCubic(sw)) * (1 + 0.12 * Math.sin(Math.PI * clamp(sw * 2.4)));
  const mVisible = t >= d.keydown + 0.05 && t < d.keyup + 0.38;
  const mode = t < d.keyup + 0.05 ? "listening" : "done";

  // pasted text
  const paste = prog(t, d.paste, d.paste + 0.5);
  const hl = 1 - prog(t, d.paste + 0.15, d.paste + 1.1);
  const caret = t >= d.click && t < d.paste && Math.floor((t - d.click) * 2) % 2 === 0;

  return (
    <div style={sceneStyle(t, d.in, d.in + 0.45, d.out, d.out + 0.4)}>
      <div style={{ ...fill, background: "linear-gradient(160deg,#e6eeff 0%,#f6ecff 55%,#ffeee8 100%)" }} />
      <div style={{ ...fill, background: "radial-gradient(900px 700px at 12% 8%, rgba(120,170,255,.55), transparent 62%), radial-gradient(900px 800px at 92% 92%, rgba(210,140,255,.42), transparent 60%)" }} />
      {/* menu bar */}
        <div style={{ position: "absolute", left: 0, top: 0, width: "100%", height: 38, background: "rgba(255,255,255,.55)", backdropFilter: "blur(20px)", fontFamily: FONT, fontSize: 17, color: INK, display: "flex", alignItems: "center", padding: "0 26px", gap: 30 }}>
          <b style={{ fontWeight: 700 }}>Correo</b>
          <span>Archivo</span>
          <span>Edición</span>
          <span>Visualización</span>
          <span>Mensaje</span>
          <span style={{ marginLeft: "auto", opacity: 0.85 }}>mar 29 sep&nbsp;&nbsp;10:31</span>
        </div>
      <div style={{ ...fill, transform: `translateX(${camX}px) scale(${cam})`, transformOrigin: "50% 55%" }}>
        {/* window */}
        <div style={{ position: "absolute", left: WIN.x, top: WIN.y, width: WIN.w, height: WIN.h, background: "#fff", borderRadius: 22, boxShadow: "0 50px 130px rgba(40,60,130,.28), 0 0 0 1px rgba(0,0,0,.06)", overflow: "hidden", fontFamily: FONT }}>
          <div style={{ height: 64, borderBottom: "1px solid #ececf0", position: "relative", background: "linear-gradient(#fafafc,#f4f4f7)" }}>
            {["#ff5f57", "#febc2e", "#28c840"].map((c, i) => (
              <span key={c} style={{ position: "absolute", left: 28 + i * 28, top: 23, width: 18, height: 18, borderRadius: 9, background: c }} />
            ))}
            <div style={{ textAlign: "center", lineHeight: "64px", fontSize: 21, fontWeight: 600, color: "#3a3a3c" }}>Nuevo mensaje</div>
          </div>
          <div style={{ height: 62, borderBottom: "1px solid #ececf0", display: "flex", alignItems: "center", padding: "0 40px", gap: 18, fontSize: 24, color: MUTED }}>
            Para:
            <span style={{ background: "#e8f0fe", color: "#1a56db", borderRadius: 24, padding: "5px 18px", fontWeight: 500 }}>Ana Torres</span>
          </div>
          <div style={{ height: 62, borderBottom: "1px solid #ececf0", display: "flex", alignItems: "center", padding: "0 40px", gap: 18, fontSize: 24, color: MUTED }}>
            Asunto:
            <span style={{ color: INK, fontWeight: 600 }}>Reunión de mañana</span>
          </div>
        </div>
        {/* body text */}
        <div style={{ position: "absolute", left: BODY.x, top: BODY.y, width: 1190, fontFamily: FONT, fontSize: 46, lineHeight: 1.36, letterSpacing: "-0.012em", color: INK }}>
          <span
            style={{
              opacity: easeOutExpo(paste),
              filter: paste < 1 ? `blur(${(1 - paste) * 6}px)` : undefined,
              background: `rgba(10,132,255,${0.2 * hl})`,
              borderRadius: 8,
              boxDecorationBreak: "clone",
              WebkitBoxDecorationBreak: "clone",
              padding: "2px 4px",
              margin: "0 -4px",
            }}
          >
            {SENTENCE}
          </span>
        </div>
        <div style={{ position: "absolute", left: BODY.x - 2, top: BODY.y + 4, width: 4, height: 58, borderRadius: 2, background: BLUE, opacity: caret && t < d.paste ? 1 : 0 }} />
        {/* click ripple + cursor */}
        <div style={{ position: "absolute", left: CLICK.x - 40, top: CLICK.y - 40, width: 80, height: 80, borderRadius: "50%", border: `3px solid ${BLUE}`, opacity: (1 - ripple) * 0.7 * (t >= d.click ? 1 : 0), transform: `scale(${lerp(0.15, 1.5, easeOutCubic(ripple))})` }} />
        <div style={{ opacity: cursorIn }}>
          <Cursor x={cx} y={cy} pressed={press} />
        </div>
        {mVisible && (
          <div style={{ position: "absolute", left: cx + 96, top: cy + 60, transform: `translate(-50%,-50%) scale(${mScale}) rotate(${sw * 30}deg)`, opacity: clamp(mIn * 3) * (1 - 0.6 * sw) }}>
            <Mascot t={t} level={speechLevel(t)} size={5.4} mode={mode} uid="demo" />
          </div>
        )}
      </div>
      <Caption t={t} a={d.cursorFrom} b={d.keydown - 0.05} words={[{ w: "Haz" }, { w: "clic" }, { w: "donde" }, { w: "quieras" }, { w: "escribir." }]} />
      <Caption t={t} a={d.keydown} b={d.keyup + 0.25} words={[{ w: "Mantén" }, { w: "una" }, { w: "tecla" }, { w: "y" }, { w: "habla.", style: gradientText }]} />
      <Caption t={t} a={d.paste} b={d.out} words={[{ w: "Suelta." }, { w: "Ya" }, { w: "está" }, { w: "escrito.", style: gradientText }]} />
      <Keycap t={t} />
    </div>
  );
}

/* ────────────────────────────── 4 · Features ────────────────────────────── */

function Card({ t, start, end, children }: { t: number; start: number; end: number; children: ReactNode }) {
  const pin = easeOutExpo(prog(t, start, start + 0.5));
  const pout = easeInCubic(prog(t, end - 0.3, end));
  const op = pin * (1 - pout);
  return (
    <div style={{ ...fill, opacity: op, visibility: op <= 0.001 ? "hidden" : "visible", transform: `translateX(${(1 - pin) * 70 - pout * 90}px)`, filter: pout > 0 || pin < 1 ? `blur(${(1 - pin) * 12 + pout * 8}px)` : undefined }}>
      {children}
    </div>
  );
}

function Feature({ t, start, title, sub }: { t: number; start: number; title: { w: string; style?: CSSProperties }[]; sub: string }) {
  return (
    <div style={{ position: "absolute", left: 150, top: 320, width: 790 }}>
      <Words
        t={t}
        gap="0.24em"
        style={{ justifyContent: "flex-start", fontFamily: FONT, fontWeight: 700, fontSize: 116, letterSpacing: "-0.045em", lineHeight: 1.04, color: INK, textAlign: "left" }}
        words={title.map((x, i) => ({ w: x.w, style: x.style, at: start + 0.12 + i * 0.09 }))}
      />
      <div
        style={{
          marginTop: 34,
          fontFamily: FONT,
          fontSize: 42,
          lineHeight: 1.3,
          color: MUTED,
          letterSpacing: "-0.01em",
          opacity: easeOutExpo(prog(t, start + 0.6, start + 1.2)),
          transform: `translateY(${(1 - easeOutExpo(prog(t, start + 0.6, start + 1.2))) * 24}px)`,
        }}
      >
        {sub}
      </div>
    </div>
  );
}

function Pill({ children, p, style }: { children: ReactNode; p: number; style?: CSSProperties }) {
  const e = easeOutBack(p, 1.6);
  return (
    <div style={{ display: "inline-flex", alignItems: "center", gap: 12, background: "#fff", borderRadius: 60, padding: "14px 28px", fontFamily: FONT, fontSize: 30, fontWeight: 600, color: INK, boxShadow: "0 14px 40px rgba(30,40,90,.13), 0 0 0 1px rgba(0,0,0,.05)", opacity: clamp(p * 3), transform: `scale(${lerp(0.7, 1, e)}) translateY(${(1 - e) * 24}px)`, ...style }}>
      {children}
    </div>
  );
}

function FeatureLock({ t }: { t: number }) {
  const f = M.f1;
  const pin = easeOutBack(prog(t, f.start + 0.2, f.start + 0.95), 1.5);
  const close = easeOutBack(prog(t, f.lock - 0.08, f.lock + 0.12), 3.2);
  const ring = prog(t, f.lock, f.lock + 0.9);
  const shackleY = lerp(-30, 0, close);
  return (
    <div style={{ position: "absolute", left: 1420 - 240, top: 470 - 260, width: 480, height: 520, transform: `scale(${pin})`, opacity: clamp(pin * 2) }}>
      <div style={{ position: "absolute", left: 20, top: 20, width: 440, height: 440, borderRadius: 116, background: "linear-gradient(155deg,#2b2b30,#0d0d10)", boxShadow: "0 50px 100px rgba(20,25,60,.35), inset 0 0 0 2px rgba(255,255,255,.08)" }} />
      <div style={{ position: "absolute", left: 20 + 220 - 235, top: 20 + 220 - 235, width: 470, height: 470, borderRadius: "50%", border: `3px solid ${BLUE}`, opacity: (1 - ring) * 0.8 * (t >= f.lock ? 1 : 0), transform: `scale(${lerp(0.7, 1.6, easeOutCubic(ring))})` }} />
      <svg viewBox="0 0 120 130" width={240} height={260} style={{ position: "absolute", left: 20 + 220 - 120, top: 20 + 220 - 132 }}>
        <path d="M34 58 V40 a26 26 0 0 1 52 0 V58" fill="none" stroke="#f5f5f7" strokeWidth={11} strokeLinecap="round" style={{ transform: `translateY(${shackleY}px)` }} />
        <rect x="16" y="56" width="88" height="66" rx="18" fill="url(#lockg)" />
        <circle cx="60" cy="86" r="8" fill="#1d1d1f" />
        <rect x="56.5" y="88" width="7" height="16" rx="3.5" fill="#1d1d1f" />
        <defs>
          <linearGradient id="lockg" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor="#8ec0ff" />
            <stop offset="1" stopColor="#7d7dff" />
          </linearGradient>
        </defs>
      </svg>
    </div>
  );
}

function WifiOff({ size = 34 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={INK} strokeWidth={2.2} strokeLinecap="round">
      <path d="M2 9a15 15 0 0 1 20 0" />
      <path d="M5.5 12.6a10 10 0 0 1 13 0" />
      <path d="M9 16.2a5 5 0 0 1 6 0" />
      <circle cx="12" cy="19.6" r="1" fill={INK} />
      <path d="M3 3 L21 21" stroke="#ff453a" strokeWidth={2.6} />
    </svg>
  );
}

function RuleRow({ t, y, at, swapAt, from, to, tag }: { t: number; y: number; at: number; swapAt: number; from: string; to: string; tag: string }) {
  const pin = easeOutBack(prog(t, at, at + 0.55), 1.4);
  const strike = easeOutCubic(prog(t, swapAt - 0.3, swapAt));
  const swap = easeOutBack(prog(t, swapAt, swapAt + 0.4), 2.1);
  const flash = 1 - prog(t, swapAt, swapAt + 0.6);
  const arrow = window4(t, swapAt - 0.25, swapAt - 0.05, swapAt, swapAt + 0.25);
  return (
    <div style={{ position: "absolute", left: 40, right: 40, top: y, fontFamily: FONT, opacity: clamp(pin * 2.5), transform: `translateY(${(1 - pin) * 34}px) scale(${lerp(0.96, 1, pin)})` }}>
      <div style={{ fontSize: 22, fontWeight: 700, letterSpacing: "0.08em", color: MUTED, textTransform: "uppercase", margin: "0 0 10px 8px" }}>{tag}</div>
      <div style={{ height: 104, borderRadius: 26, background: "#f5f5f9", display: "flex", alignItems: "center", padding: "0 30px", gap: 22, whiteSpace: "nowrap" }}>
        <div style={{ fontSize: 42, fontWeight: 600, color: INK, position: "relative", minWidth: 210 }}>
          <span style={{ opacity: lerp(1, 0.4, strike) }}>{from}</span>
          <span style={{ position: "absolute", left: 0, top: "56%", height: 4, borderRadius: 2, background: "#ff453a", width: `${strike * 100}%` }} />
        </div>
        <svg width="52" height="28" viewBox="0 0 56 30" style={{ flex: "none", transform: `translateX(${arrow * 10}px)` }}>
          <path d="M4 15 H48 M36 5 L48 15 L36 25" fill="none" stroke={arrow > 0.5 ? BLUE : "#9a9aa2"} strokeWidth={4} strokeLinecap="round" strokeLinejoin="round" />
        </svg>
        <div style={{ fontSize: 42, fontWeight: 700, color: BLUE, background: `rgba(10,132,255,${0.1 + 0.25 * flash})`, borderRadius: 18, padding: "6px 22px", opacity: clamp(swap * 2.5), transform: `scale(${lerp(0.5, 1, swap)})`, transformOrigin: "left center" }}>
          {to}
        </div>
      </div>
    </div>
  );
}

function FeatureDictionary({ t }: { t: number }) {
  const f = M.f2;
  const pin = easeOutBack(prog(t, f.start + 0.2, f.start + 0.85), 1.3);
  return (
    <div style={{ position: "absolute", left: 1450 - 420, top: 520 - 250, width: 840, height: 500, borderRadius: 44, background: "#fff", boxShadow: "0 50px 110px rgba(30,40,100,.18), 0 0 0 1px rgba(0,0,0,.05)", opacity: clamp(pin * 2), transform: `scale(${lerp(0.9, 1, pin)}) translateY(${(1 - pin) * 40}px)` }}>
      <div style={{ padding: "34px 40px 0", fontFamily: FONT, fontSize: 28, fontWeight: 700, color: INK, letterSpacing: "-0.01em" }}>Mi diccionario</div>
      <RuleRow t={t} y={100} at={f.start + 0.55} swapAt={f.swap1} from="cloud" to="Claude" tag="Corrección" />
      <RuleRow t={t} y={290} at={f.row2} swapAt={f.swap2} from="mi correo" to="hola@dilo.app" tag="Atajo" />
    </div>
  );
}

const HELLOS: { w: string; bg: string; fg: string; r: number }[] = [
  { w: "Hola", bg: "#e8f0ff", fg: "#1a56db", r: -3 },
  { w: "Hello", bg: "#ffe9e4", fg: "#d6402a", r: 2 },
  { w: "Bonjour", bg: "#efe8ff", fg: "#6d3adb", r: -2 },
  { w: "Hallo", bg: "#e5f8ec", fg: "#1a8a4a", r: 3 },
  { w: "Ciao", bg: "#fff3d6", fg: "#b26b00", r: -3 },
  { w: "Olá", bg: "#e5f6ff", fg: "#0a7ab5", r: 2 },
  { w: "Cześć", bg: "#ffe6f2", fg: "#c2337a", r: -2 },
  { w: "Hej", bg: "#eaf0ff", fg: "#2a4fd6", r: 3 },
  { w: "Привіт", bg: "#e9f7e4", fg: "#3b8a1a", r: -3 },
  { w: "Ahoj", bg: "#fdeaea", fg: "#c2372f", r: 2 },
];

function FeatureLanguages({ t }: { t: number }) {
  const f = M.f3;
  return (
    <div style={{ position: "absolute", left: 1450 - 430, top: 520 - 270, width: 860, height: 540, display: "flex", flexWrap: "wrap", alignContent: "center", justifyContent: "center", gap: 26 }}>
      {HELLOS.map((h, i) => {
        const at = f.chipsFrom + i * f.chipEvery;
        const p = easeOutBack(prog(t, at, at + 0.5), 2.0);
        const bob = Math.sin(t * 2.2 + i * 1.3) * 5;
        return (
          <div key={h.w} style={{ fontFamily: FONT, fontSize: 62, fontWeight: 700, letterSpacing: "-0.03em", color: h.fg, background: h.bg, borderRadius: 70, padding: "16px 42px", boxShadow: "0 16px 40px rgba(30,40,90,.10)", opacity: clamp(p * 3), transform: `translateY(${(1 - p) * 50 + bob * p}px) scale(${lerp(0.5, 1, p)}) rotate(${h.r * p}deg)` }}>
            {h.w}
          </div>
        );
      })}
    </div>
  );
}

const SKINS: { id: Skin; label: string; big: number; small: number }[] = [
  { id: "wave", label: "Wave", big: 5.2, small: 1.9 },
  { id: "glass", label: "Glass", big: 7.6, small: 2.6 },
  { id: "jolly", label: "Jolly", big: 7.6, small: 2.6 },
  { id: "dot", label: "Dot", big: 9.6, small: 3.2 },
  { id: "aura", label: "Aura", big: 7.6, small: 2.6 },
];

function FeatureSkins({ t }: { t: number }) {
  const f = M.f4;
  const pin = easeOutBack(prog(t, f.start + 0.2, f.start + 0.9), 1.3);
  const order: Skin[] = ["aura", "glass", "jolly", "dot", "wave", "aura"];
  let idx = 0;
  f.selects.forEach((s, i) => {
    if (t >= s) idx = i + 1;
  });
  const sel = order[idx];
  const lastAt = idx === 0 ? -9 : f.selects[idx - 1];
  const hop = easeOutBack(prog(t, lastAt, lastAt + 0.32), 2.4);
  const level = clamp(0.5 + 0.35 * Math.sin(t * 9.1) + 0.2 * Math.sin(t * 15.3 + 2), 0.05, 1);
  const cur = SKINS.find((s) => s.id === sel)!;
  return (
    <div style={{ position: "absolute", left: 1440 - 440, top: 520 - 300, width: 880, height: 600, borderRadius: 54, background: "linear-gradient(160deg,#17171c,#08080b)", boxShadow: "0 60px 120px rgba(20,25,60,.35), inset 0 0 0 1.5px rgba(255,255,255,.07)", opacity: clamp(pin * 2), transform: `scale(${lerp(0.9, 1, pin)}) translateY(${(1 - pin) * 40}px)`, overflow: "hidden" }}>
      <div style={{ position: "absolute", left: 440 - 330, top: 40 - 200 + 150, width: 660, height: 660, borderRadius: "50%", background: "radial-gradient(circle, rgba(94,155,255,.22), transparent 62%)" }} />
      <div style={{ position: "absolute", left: 440, top: 190, transform: `translate(-50%,-50%) scale(${lerp(0.82, 1, hop)})` }}>
        <Mascot t={t} level={level} skin={sel} size={cur.big} uid="skins-big" />
      </div>
      <div style={{ position: "absolute", left: 0, bottom: 44, width: "100%", display: "flex", justifyContent: "center", gap: 20 }}>
        {SKINS.map((s) => {
          const on = s.id === sel;
          const press = on ? 1 + 0.06 * hop : 1;
          return (
            <div key={s.id} style={{ width: 150, height: 170, borderRadius: 34, background: on ? "#26262d" : "#1a1a20", boxShadow: on ? `0 0 0 3px ${BLUE}` : "inset 0 0 0 1.5px rgba(255,255,255,.06)", transform: `scale(${press})`, position: "relative", fontFamily: FONT }}>
              <div style={{ position: "absolute", left: 75, top: 70, transform: "translate(-50%,-50%)" }}>
                <Mascot t={t} level={level} skin={s.id} size={s.small} uid={`tile-${s.id}`} />
              </div>
              <div style={{ position: "absolute", bottom: 16, width: "100%", textAlign: "center", fontSize: 26, fontWeight: 600, color: on ? "#fff" : "rgba(255,255,255,.55)" }}>{s.label}</div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

export function Features({ t }: { t: number }) {
  const [s0, s1] = T.scenes.features;
  const c = [M.f1.start, M.f2.start, M.f3.start, M.f4.start, s1];
  return (
    <div style={sceneStyle(t, s0 - 0.3, s0 + 0.15, s1 - 0.25, s1 + 0.1)}>
      <div style={{ ...fill, background: "#f5f5f7" }} />
      <div style={{ ...fill, background: "radial-gradient(900px 700px at 90% 8%, rgba(120,170,255,.28), transparent 62%), radial-gradient(800px 700px at 4% 100%, rgba(210,150,255,.22), transparent 60%)" }} />
      <Card t={t} start={c[0]} end={c[1]}>
        <Feature t={t} start={c[0]} title={[{ w: "Todo" }, { w: "pasa" }, { w: "en" }, { w: "tu", style: gradientText }, { w: "Mac.", style: gradientText }]} sub="Sin nube. Sin cuenta. Tu voz nunca sale de tu computadora." />
        <FeatureLock t={t} />
        <div style={{ position: "absolute", left: 1420 - 260, top: 800 }}>
          <Pill p={easeOutExpo(prog(t, c[0] + 0.9, c[0] + 1.4))}>
            <WifiOff /> Funciona sin internet
          </Pill>
        </div>
      </Card>
      <Card t={t} start={c[1]} end={c[2]}>
        <Feature t={t} start={c[1]} title={[{ w: "Escribe" }, { w: "a" }, { w: "tu", style: gradientText }, { w: "manera.", style: gradientText }]} sub="Correcciones y atajos en tu diccionario personal." />
        <FeatureDictionary t={t} />
      </Card>
      <Card t={t} start={c[2]} end={c[3]}>
        <Feature t={t} start={c[2]} title={[{ w: "Habla" }, { w: "en" }, { w: "25", style: gradientText }, { w: "idiomas.", style: gradientText }]} sub="Español, English, Français, Deutsch, Português… Dilo entiende." />
        <FeatureLanguages t={t} />
      </Card>
      <Card t={t} start={c[3]} end={c[4]}>
        <Feature t={t} start={c[3]} title={[{ w: "Elige" }, { w: "tu", style: gradientText }, { w: "monito.", style: gradientText }]} sub="Cinco estilos. Tres tamaños." />
        <FeatureSkins t={t} />
      </Card>
    </div>
  );
}

/* ────────────────────────────── 5 · Outro ────────────────────────────── */

export function Outro({ t }: { t: number }) {
  const o = M.outro;
  const [s0, s1] = T.scenes.outro;
  const pop = easeOutBack(prog(t, s0 + 0.15, s0 + 0.95), 1.9);
  const glow = easeOutExpo(prog(t, s0 + 0.1, s0 + 1.4));
  const word: CSSProperties = { fontFamily: FONT, fontWeight: 700, color: WHITE, letterSpacing: "-0.055em", lineHeight: 1 };
  const chip = easeOutExpo(prog(t, o.sub + 0.9, o.sub + 1.5));
  return (
    <div style={sceneStyle(t, s0 - 0.25, s0 + 0.15, s1 + 1, s1 + 2)}>
      <div style={{ ...fill, background: "#000" }} />
      <div style={{ position: "absolute", left: 960 - 700, top: 330 - 700, width: 1400, height: 1400, borderRadius: "50%", background: "radial-gradient(circle, rgba(94,155,255,0.36) 0%, rgba(179,107,255,0.18) 32%, transparent 62%)", transform: `scale(${lerp(0.3, 1, glow)})`, opacity: glow }} />
      <div style={{ position: "absolute", left: 960, top: 330, transform: `translate(-50%,-50%) scale(${pop})`, opacity: clamp(pop * 3) }}>
        <Mascot t={t} level={idleLevel(t, [o.letters[0], o.sub, o.sub + 0.09, o.sub + 0.18, o.sub + 0.27, o.shimmer])} size={8.4} uid="outro" />
      </div>
      <Letters t={t} text="Dilo" at={o.letters} style={{ ...word, position: "absolute", left: 0, width: "100%", top: 520, fontSize: 290 }} />
      <Words t={t} style={{ ...word, position: "absolute", left: 0, width: "100%", top: 840, fontSize: 70, fontWeight: 500 }} gap="0.3em" words={[{ w: "Y", at: o.sub, style: { color: "rgba(245,245,247,.8)" } }, { w: "ya", at: o.sub + 0.08, style: { color: "rgba(245,245,247,.8)" } }, { w: "está", at: o.sub + 0.16, style: { color: "rgba(245,245,247,.8)" } }, { w: "escrito.", at: o.sub + 0.24, style: gradientText }]} />
      <div style={{ position: "absolute", left: 0, width: "100%", top: 970, textAlign: "center", fontFamily: FONT, fontSize: 30, fontWeight: 500, letterSpacing: "0.02em", color: "rgba(245,245,247,.5)", opacity: chip, transform: `translateY(${(1 - chip) * 14}px)` }}>
        Para Mac
      </div>
    </div>
  );
}
