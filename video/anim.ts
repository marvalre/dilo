// Small deterministic animation helpers: everything is a pure function of time t (seconds).
export const clamp = (x: number, a = 0, b = 1) => Math.max(a, Math.min(b, x));
/** Progress 0..1 of t between a and b. */
export const prog = (t: number, a: number, b: number) => clamp((t - a) / (b - a));
export const easeOutCubic = (x: number) => 1 - Math.pow(1 - clamp(x), 3);
export const easeOutExpo = (x: number) => (clamp(x) >= 1 ? 1 : 1 - Math.pow(2, -10 * clamp(x)));
export const easeInCubic = (x: number) => Math.pow(clamp(x), 3);
export const easeInOutCubic = (x: number) => {
  x = clamp(x);
  return x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2;
};
/** Overshooting ease (ease-out-back). */
export const easeOutBack = (x: number, s = 1.7) => {
  x = clamp(x) - 1;
  return x * x * ((s + 1) * x + s) + 1;
};
export const lerp = (a: number, b: number, k: number) => a + (b - a) * k;
/** 0 before `a`, 1 between b and c, 0 after d — a soft pulse window. */
export const window4 = (t: number, a: number, b: number, c: number, d: number) =>
  prog(t, a, b) * (1 - prog(t, c, d));
