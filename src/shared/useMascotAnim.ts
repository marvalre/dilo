import { useEffect, useRef, useState } from "react";
import type { Mode } from "./MascotFace";

// Mouth opens fast and closes softly.
const ATTACK = 0.5;
const RELEASE = 0.15;

/** Drives the mascot at display refresh rate. `target` is read every frame (0..1 voice level). */
export function useMascotAnim(mode: Mode, target: { current: number }, running = true) {
  const [frame, setFrame] = useState({ level: 0, t: 0, blink: false });
  const level = useRef(0);
  const blinkUntil = useRef(0);

  useEffect(() => {
    if (!running) {
      level.current = 0;
      return;
    }
    let raf = 0;
    let nextBlink = performance.now() + 2500 + Math.random() * 2500;
    const tick = (t: number) => {
      const goal = mode === "listening" ? target.current : 0;
      level.current += (goal - level.current) * (goal > level.current ? ATTACK : RELEASE);
      if (t > nextBlink) {
        blinkUntil.current = t + 120;
        nextBlink = t + 2500 + Math.random() * 2500;
      }
      setFrame({ level: level.current, t, blink: t < blinkUntil.current });
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [mode, target, running]);

  return frame;
}
