// Renders the video page frame by frame with headless Chrome (CDP) and pipes the frames to ffmpeg.
//   node render.mjs stills 2.3,5.6,9.4        → stills/<t>.png (to check individual moments)
//   node render.mjs video [out.mp4]           → silent 1080p60 video (mux audio afterwards)
// Needs the Vite dev server running (bunx vite --port 1431) and Node 22+.
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const T = JSON.parse(fs.readFileSync(path.join(here, "timeline.json"), "utf8"));
const [mode, arg] = [process.argv[2], process.argv[3]];
const URL_ = process.env.VIDEO_URL || "http://localhost:1431/video/index.html";
const CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const PORT = 9300 + Math.floor(Math.random() * 300);
const profile = fs.mkdtempSync(path.join(os.tmpdir(), "dilo-video-"));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const chrome = spawn(
  CHROME,
  ["--headless=new", "--hide-scrollbars", "--force-device-scale-factor=1", `--window-size=${T.width},${T.height}`, `--remote-debugging-port=${PORT}`, "--no-first-run", "--no-default-browser-check", `--user-data-dir=${profile}`, "--mute-audio", "about:blank"],
  { stdio: "ignore" },
);
const cleanup = () => {
  try { chrome.kill("SIGKILL"); } catch {}
  try { fs.rmSync(profile, { recursive: true, force: true }); } catch {}
};
process.on("exit", cleanup);

let targets;
for (let i = 0; i < 100; i++) {
  try { targets = await (await fetch(`http://localhost:${PORT}/json`)).json(); if (targets.length) break; } catch {}
  await sleep(200);
}
const ws = new WebSocket(targets.find((t) => t.type === "page").webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0;
const pending = new Map();
ws.onmessage = (ev) => {
  const m = JSON.parse(ev.data);
  if (m.id && pending.has(m.id)) {
    const { res, rej } = pending.get(m.id);
    pending.delete(m.id);
    m.error ? rej(new Error(JSON.stringify(m.error))) : res(m.result);
  }
  if (m.method === "Runtime.exceptionThrown") console.error("PAGE ERROR", m.params.exceptionDetails.exception?.description);
};
const send = (method, params = {}) => new Promise((res, rej) => { const i = ++id; pending.set(i, { res, rej }); ws.send(JSON.stringify({ id: i, method, params })); });

await send("Page.enable");
await send("Emulation.setDeviceMetricsOverride", { width: T.width, height: T.height, deviceScaleFactor: 1, mobile: false });
await send("Page.navigate", { url: URL_ });
for (let i = 0; i < 100; i++) {
  const r = await send("Runtime.evaluate", { expression: "typeof window.__seek", returnByValue: true });
  if (r.result.value === "function") break;
  await sleep(200);
}
await send("Runtime.evaluate", { expression: "document.fonts.ready.then(()=>1)", awaitPromise: true });

const seek = (t) => send("Runtime.evaluate", { expression: `window.__seek(${t})`, awaitPromise: true });
const shot = async (fmt = "jpeg") => Buffer.from((await send("Page.captureScreenshot", { format: fmt, quality: 96, captureBeyondViewport: false })).data, "base64");

if (mode === "stills") {
  const dir = path.join(here, "stills");
  fs.mkdirSync(dir, { recursive: true });
  for (const t of arg.split(",").map(Number)) {
    await seek(t);
    fs.writeFileSync(path.join(dir, `${t.toFixed(2)}.png`), await shot("png"));
    console.log("still", t);
  }
} else if (mode === "video") {
  const out = arg || path.join(here, "silent.mp4");
  const total = Math.round(T.duration * T.fps);
  const ff = spawn("ffmpeg", ["-y", "-loglevel", "error", "-f", "image2pipe", "-framerate", String(T.fps), "-c:v", "mjpeg", "-i", "-", "-c:v", "libx264", "-preset", "medium", "-crf", "14", "-pix_fmt", "yuv420p", "-r", String(T.fps), "-movflags", "+faststart", out], { stdio: ["pipe", "inherit", "inherit"] });
  const t0 = Date.now();
  for (let f = 0; f < total; f++) {
    await seek(f / T.fps);
    const buf = await shot("jpeg");
    if (!ff.stdin.write(buf)) await new Promise((r) => ff.stdin.once("drain", r));
    if (f % 120 === 0) console.log(`frame ${f}/${total}  ${((Date.now() - t0) / 1000).toFixed(0)}s`);
  }
  ff.stdin.end();
  await new Promise((r) => ff.on("close", r));
  console.log("done", out);
}
cleanup();
process.exit(0);
