// Pretty-printing of handy-keys hotkey strings ("Fn", "CtrlRight", "Cmd+Shift+D").

const MOD_SYMBOL: Record<string, string> = {
  cmd: "⌘",
  command: "⌘",
  meta: "⌘",
  super: "⌘",
  shift: "⇧",
  opt: "⌥",
  option: "⌥",
  alt: "⌥",
  ctrl: "⌃",
  control: "⌃",
};

const KEY_NAME: Record<string, string> = {
  space: "espacio",
  enter: "↩",
  return: "↩",
  tab: "⇥",
  escape: "esc",
  esc: "esc",
  backspace: "⌫",
  delete: "⌦",
  up: "↑",
  down: "↓",
  left: "←",
  right: "→",
  capslock: "⇪",
};

/** One keycap label per token. */
function tokenCap(token: string): string {
  const t = token.trim();
  const lower = t.toLowerCase();
  if (lower === "fn" || lower === "function" || lower === "globe") return "fn";
  const side = /^(.*?)(right|left)$/i.exec(t);
  if (side && MOD_SYMBOL[side[1].toLowerCase()]) {
    return `${MOD_SYMBOL[side[1].toLowerCase()]} ${side[2].toLowerCase() === "right" ? "derecha" : "izquierda"}`;
  }
  if (MOD_SYMBOL[lower]) return MOD_SYMBOL[lower];
  if (KEY_NAME[lower]) return KEY_NAME[lower];
  if (/^key[a-z]$/i.test(t)) return t.slice(3).toUpperCase();
  if (/^digit\d$/i.test(t)) return t.slice(5);
  return t.length === 1 ? t.toUpperCase() : t;
}

// macOS display order: fn, ⌃, ⌥, ⇧, ⌘, then the key.
const ORDER = ["fn", "⌃", "⌥", "⇧", "⌘"];
const rank = (cap: string) => {
  const i = ORDER.indexOf(cap.split(" ")[0]);
  return i < 0 ? ORDER.length : i;
};

/** Keycaps for the settings row, order-independent: "Shift+Cmd+D" → ["⇧", "⌘", "D"]. */
export function hotkeyCaps(hotkey: string): string[] {
  return hotkey
    .split("+")
    .filter(Boolean)
    .map(tokenCap)
    .map((c, i) => ({ c, i }))
    .sort((a, b) => rank(a.c) - rank(b.c) || a.i - b.i)
    .map((x) => x.c);
}

/** Compact inline label: "Fn" → "fn 🌐", "CtrlRight" → "⌃ derecha", "Cmd+Shift+D" → "⌘⇧D". */
export function hotkeyLabel(hotkey: string): string {
  const caps = hotkeyCaps(hotkey);
  if (caps.length === 1) return caps[0] === "fn" ? "fn 🌐" : caps[0];
  const allShort = caps.every((c) => c.length === 1);
  return caps.join(allShort ? "" : " ");
}
