import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { flushSync } from "react-dom";
import T from "./timeline.json";
import { prog } from "./anim";
import { Demo, Features, Hook, Outro, Reveal } from "./scenes";
import { fill } from "./ui";

let setTime: (t: number) => void = () => {};

function Video() {
  const [t, setT] = useState(0);
  useEffect(() => {
    setTime = setT;
  }, []);
  const fadeOut = prog(t, T.duration - 0.3, T.duration);
  return (
    <div style={{ ...fill, background: "#000", overflow: "hidden" }}>
      <Hook t={t} />
      <Reveal t={t} />
      <Demo t={t} />
      <Features t={t} />
      <Outro t={t} />
      <div style={{ ...fill, background: "#000", opacity: fadeOut, pointerEvents: "none" }} />
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<Video />);

// Used by render.mjs: draw the frame for time t and resolve after it has painted.
(window as unknown as { __seek: (t: number) => Promise<void> }).__seek = (t: number) => {
  flushSync(() => setTime(t));
  return new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() => r())));
};
