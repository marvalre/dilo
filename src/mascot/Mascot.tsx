import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { MascotFace, type MascotSize, type Mode, type Skin } from "../shared/MascotFace";
import { useMascotAnim } from "../shared/useMascotAnim";

/** States sent by Rust on `mascot://state`. */
type State = Mode | "hidden" | "swallow";

interface Look {
  mascot_skin: Skin;
  mascot_size: MascotSize;
}

const inTauri = "__TAURI_INTERNALS__" in window;

export function Mascot() {
  const [state, setState] = useState<State>(inTauri ? "hidden" : "listening");
  const [look, setLook] = useState<Look>({ mascot_skin: "glass", mascot_size: "m" });
  const target = useRef(0);
  const lastMode = useRef<Mode>("listening");

  useEffect(() => {
    if (!inTauri) return runDemo(target, setState);
    invoke<Look>("get_settings").then(setLook).catch(() => {});
    const subs = [
      listen<State>("mascot://state", (e) => {
        if (e.payload === "listening") {
          target.current = 0;
          invoke<Look>("get_settings").then(setLook).catch(() => {});
        }
        setState(e.payload);
      }),
      listen<number>("mascot://level", (e) => {
        const v = Number(e.payload);
        target.current = Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : 0;
      }),
      listen<Look>("settings://changed", (e) => setLook(e.payload)),
    ];
    return () => subs.forEach((s) => s.then((f) => f()).catch(() => {}));
  }, []);

  if (state !== "hidden" && state !== "swallow") lastMode.current = state;
  const mode = lastMode.current;
  const frame = useMascotAnim(mode, target, state !== "hidden");
  if (state === "hidden") return null;

  return (
    <div className={`stage state-${state}`}>
      <div className="creature" style={{ transform: `scale(${1 + frame.level * 0.06})` }}>
        <MascotFace skin={look.mascot_skin} size={look.mascot_size} mode={mode} level={frame.level} t={frame.t} blink={frame.blink} uid="m" />
      </div>
    </div>
  );
}

/** Browser-only demo (no Tauri): cycles through the states with a fake voice. */
function runDemo(target: { current: number }, setState: (s: State) => void) {
  const script: [State, number][] = [
    ["listening", 3000],
    ["thinking", 1400],
    ["done", 450],
    ["swallow", 700],
    ["hidden", 500],
  ];
  let i = 0;
  let timer = 0;
  const voice = window.setInterval(() => (target.current = Math.random() < 0.15 ? 0 : Math.random()), 110);
  const step = () => {
    const [s, ms] = script[i++ % script.length];
    setState(s);
    timer = window.setTimeout(step, ms);
  };
  step();
  return () => {
    window.clearInterval(voice);
    window.clearTimeout(timer);
  };
}
