import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";

/** States sent by Rust on `mascot://state`. */
export type MascotState = "hidden" | "listening" | "thinking" | "done" | "swallow" | "sad" | "confused";

const MOUTH_POINTS = 9;
const MOUTH_LEFT = 28.5;
const MOUTH_RIGHT = 43.5;
const MOUTH_Y = 41.5;
// Smoothing so the mouth opens quickly and closes softly.
const ATTACK = 0.55;
const RELEASE = 0.12;

/** Bell curve: the middle of the mouth moves most, the corners barely. */
const envelope = (i: number) => Math.sin((Math.PI * i) / (MOUTH_POINTS - 1));

function mouthPath(level: number, t: number, state: MascotState): string {
  const pts: string[] = [];
  for (let i = 0; i < MOUTH_POINTS; i++) {
    const x = MOUTH_LEFT + ((MOUTH_RIGHT - MOUTH_LEFT) * i) / (MOUTH_POINTS - 1);
    const env = envelope(i);
    let y = MOUTH_Y;
    if (state === "listening") {
      const smile = 0.9 * env; // resting smile
      const wave = Math.sin(t * 0.018 + i * 1.15) * 0.6 + Math.sin(t * 0.011 - i * 0.7) * 0.4;
      y += smile + wave * level * 6.5 * env;
    } else if (state === "done") {
      y += 2.2 * env; // big smile
    } else if (state === "sad") {
      y += -1.6 * env + 1.4;
    } else if (state === "confused") {
      y += Math.sin(i * 1.4) * 0.9;
    } else {
      y += 0.5 * env; // thinking: calm, slight smile
    }
    pts.push(`${x.toFixed(2)},${y.toFixed(2)}`);
  }
  return `M${pts.join(" L")}`;
}

function Eyes({ state, blink }: { state: MascotState; blink: boolean }) {
  const ink = "var(--ink)";
  if (state === "done") {
    return (
      <g fill="none" stroke={ink} strokeWidth={1.8} strokeLinecap="round">
        <path d="M28.2 33 Q30.2 29.8 32.2 33" />
        <path d="M39.8 33 Q41.8 29.8 43.8 33" />
      </g>
    );
  }
  if (state === "sad") {
    return (
      <g fill="none" stroke={ink} strokeWidth={1.8} strokeLinecap="round">
        <path d="M28.2 31 Q30.2 33.6 32.2 31" />
        <path d="M39.8 31 Q41.8 33.6 43.8 31" />
      </g>
    );
  }
  if (state === "thinking") {
    return (
      <g className="thinking-eyes" stroke={ink} strokeWidth={1.9} strokeLinecap="round">
        <line x1={29} y1={31} x2={32.4} y2={30.2} />
        <line x1={39.6} y1={31} x2={43} y2={30.2} />
      </g>
    );
  }
  if (state === "confused") {
    return (
      <g fill={ink} stroke={ink} strokeWidth={1.8} strokeLinecap="round">
        <circle cx={30.2} cy={32} r={2.3} stroke="none" />
        <line x1={39.6} y1={32} x2={43.6} y2={31.2} />
      </g>
    );
  }
  return (
    <g fill={ink}>
      <ellipse cx={30.2} cy={32} rx={2.3} ry={blink ? 0.35 : 2.5} />
      <ellipse cx={41.8} cy={32} rx={2.3} ry={blink ? 0.35 : 2.5} />
    </g>
  );
}

export function Mascot() {
  const [state, setState] = useState<MascotState>("hidden");
  const [blink, setBlink] = useState(false);
  const [scale, setScale] = useState(1);
  const mouthRef = useRef<SVGPathElement>(null);
  const target = useRef(0);
  const level = useRef(0);
  const stateRef = useRef<MascotState>("hidden");

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return runDemo(stateRef, target, setState);
    const unState = listen<MascotState>("mascot://state", (e) => {
      stateRef.current = e.payload;
      if (e.payload !== "listening") target.current = 0;
      setState(e.payload);
    });
    const unLevel = listen<number>("mascot://level", (e) => {
      target.current = e.payload;
    });
    return () => {
      unState.then((f) => f());
      unLevel.then((f) => f());
    };
  }, []);

  // Mouth animation loop. Writes the path directly to avoid re-rendering React at 60 fps.
  useEffect(() => {
    let raf = 0;
    let lastScale = 1;
    const tick = (t: number) => {
      const k = target.current > level.current ? ATTACK : RELEASE;
      level.current += (target.current - level.current) * k;
      mouthRef.current?.setAttribute("d", mouthPath(level.current, t, stateRef.current));
      const s = stateRef.current === "listening" ? 1 + level.current * 0.07 : 1;
      if (Math.abs(s - lastScale) > 0.004) {
        lastScale = s;
        setScale(s);
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, []);

  // Random blinking while awake.
  useEffect(() => {
    if (state !== "listening") return;
    let timer: number;
    const schedule = () => {
      timer = window.setTimeout(() => {
        setBlink(true);
        window.setTimeout(() => setBlink(false), 120);
        schedule();
      }, 2500 + Math.random() * 2500);
    };
    schedule();
    return () => window.clearTimeout(timer);
  }, [state]);

  if (state === "hidden") return null;

  return (
    <div className={`stage state-${state}`}>
      <svg viewBox="0 0 72 72" width={72} height={72} aria-hidden>
        <defs>
          <radialGradient id="body" cx="38%" cy="32%" r="75%">
            <stop offset="0%" stopColor="var(--body-hi)" />
            <stop offset="70%" stopColor="var(--body)" />
            <stop offset="100%" stopColor="var(--body-lo)" />
          </radialGradient>
        </defs>
        <g className="creature">
         <g className="breath" style={{ transform: `scale(${scale})` }}>
          <circle className="body" cx={36} cy={36} r={22} fill="url(#body)" stroke="var(--outline)" strokeWidth={1} />
          <g className="features">
            <Eyes state={state} blink={blink} />
            <path
              ref={mouthRef}
              className="mouth"
              fill="none"
              stroke="var(--ink)"
              strokeWidth={1.8}
              strokeLinecap="round"
              strokeLinejoin="round"
            />
            <g className="cheeks" fill="var(--cheek)">
              <ellipse cx={25.5} cy={38} rx={2.6} ry={1.5} />
              <ellipse cx={46.5} cy={38} rx={2.6} ry={1.5} />
            </g>
          </g>
         </g>
        </g>
      </svg>
    </div>
  );
}

/** Browser-only demo (no Tauri): cycles through every state with a fake voice. */
function runDemo(
  stateRef: { current: MascotState },
  target: { current: number },
  setState: (s: MascotState) => void,
) {
  const script: [MascotState, number][] = [
    ["listening", 3200],
    ["thinking", 1200],
    ["done", 500],
    ["swallow", 900],
    ["confused", 1200],
    ["sad", 1200],
  ];
  let i = 0;
  let stepTimer = 0;
  const voice = window.setInterval(() => {
    target.current = stateRef.current === "listening" ? Math.max(0, Math.random() * 1.1 - 0.15) : 0;
  }, 90);
  const step = () => {
    const [s, ms] = script[i++ % script.length];
    stateRef.current = s;
    setState(s);
    stepTimer = window.setTimeout(step, ms);
  };
  step();
  return () => {
    window.clearInterval(voice);
    window.clearTimeout(stepTimer);
  };
}
