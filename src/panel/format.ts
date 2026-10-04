const nf = new Intl.NumberFormat("es");
const nf1 = new Intl.NumberFormat("es", { minimumFractionDigits: 1, maximumFractionDigits: 1 });
const timeFmt = new Intl.DateTimeFormat("es", { hour: "2-digit", minute: "2-digit" });
const dayMonthFmt = new Intl.DateTimeFormat("es", { day: "numeric", month: "short" });
const fullDateFmt = new Intl.DateTimeFormat("es", { day: "numeric", month: "short", year: "numeric" });
const langNames = new Intl.DisplayNames(["es"], { type: "language" });

const finite = (n: number) => (Number.isFinite(n) ? n : 0);

const axisFmt = new Intl.NumberFormat("es", { maximumFractionDigits: 1 });
/** Axis tick label: keeps one decimal so 0.5 doesn't round up to 1. */
export const fmtAxis = (n: number) => axisFmt.format(finite(n));

export const fmtNum = (n: number) => nf.format(Math.round(finite(n)) || 0);

export function fmtDuration(ms: number): string {
  const v = Math.max(0, finite(ms));
  const tenths = Math.round(v / 100);
  if (tenths < 600) return `${nf1.format(tenths / 10)} s`;
  const secs = Math.round(v / 1000);
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return s ? `${fmtNum(m)} min ${s} s` : `${fmtNum(m)} min`;
}

export function fmtMinutes(min: number): string {
  const total = Math.max(0, Math.round(finite(min)));
  if (total < 60) return `${total} min`;
  const h = Math.floor(total / 60);
  const m = total % 60;
  return m ? `${fmtNum(h)} h ${m} min` : `${fmtNum(h)} h`;
}

const validDate = (d: Date) => Number.isFinite(d.getTime());

export const fmtDayMonth = (d: Date) => (validDate(d) ? dayMonthFmt.format(d).replace(/\.$/, "").replace("sept", "sep") : "");

const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();

export function fmtRelative(ts: number, now = Date.now()): string {
  if (!validDate(new Date(ts))) return "";
  const diffMin = Math.floor((now - ts) / 60000);
  if (diffMin < 1) return "ahora";
  if (diffMin < 60) return `hace ${diffMin} min`;
  const date = new Date(ts);
  const n = new Date(now);
  const day = startOfDay(date);
  const time = timeFmt.format(date);
  if (day === startOfDay(n)) return `hoy ${time}`;
  if (day === new Date(n.getFullYear(), n.getMonth(), n.getDate() - 1).getTime()) return `ayer ${time}`;
  if (date.getFullYear() === new Date(now).getFullYear()) return `${fmtDayMonth(date)} ${time}`;
  return fullDateFmt.format(date);
}

/** Parse "YYYY-MM-DD" as a local date. */
export function parseDay(key: string): Date {
  const [y, m, d] = key.split("-").map(Number);
  return new Date(y, m - 1, d);
}

export function todayKey(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

export function langName(code: string | null): string {
  if (!code || code === "?") return "Desconocido";
  try {
    const name = langNames.of(code) ?? code;
    return name.charAt(0).toUpperCase() + name.slice(1);
  } catch {
    return code.toUpperCase();
  }
}

export const errMsg = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : String(e));

const weekdayFmt = new Intl.DateTimeFormat("es", { weekday: "long", day: "numeric", month: "short" });
const weekdayYearFmt = new Intl.DateTimeFormat("es", { weekday: "long", day: "numeric", month: "short", year: "numeric" });

export const fmtTime = (ts: number) => (validDate(new Date(ts)) ? timeFmt.format(new Date(ts)) : "");

/** Local-day key "YYYY-MM-DD" for a timestamp. */
export function dayKey(ts: number): string {
  const d = new Date(ts);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

/** Day header for history: "Hoy", "Ayer", "lunes 26 sep". */
export function fmtDayHeader(ts: number, now = Date.now()): string {
  const d = new Date(ts);
  const n = new Date(now);
  if (!validDate(d)) return "";
  const day = startOfDay(d);
  if (day === startOfDay(n)) return "Hoy";
  if (day === new Date(n.getFullYear(), n.getMonth(), n.getDate() - 1).getTime()) return "Ayer";
  const f = d.getFullYear() === n.getFullYear() ? weekdayFmt : weekdayYearFmt;
  return f.format(d).replace(",", "").replace(/\./g, "").replace("sept", "sep");
}

export const isToday = (ts: number, now = Date.now()) => startOfDay(new Date(ts)) === startOfDay(new Date(now));

/** Monday-based start of the current week. */
export function startOfWeek(now = Date.now()): number {
  const n = new Date(now);
  const dow = (n.getDay() + 6) % 7;
  return new Date(n.getFullYear(), n.getMonth(), n.getDate() - dow).getTime();
}

const mbFmt = new Intl.NumberFormat("es", { maximumFractionDigits: 1 });

/** "670 MB", "1,2 MB", "340 KB", "1,4 GB". */
export function fmtBytes(bytes: number): string {
  const b = Math.max(0, Math.round(finite(bytes)));
  if (b < 1_000) return `${b} B`;
  const kb = Math.round(b / 1_000);
  if (kb < 1_000) return `${kb} KB`;
  const mb = b / 1_000_000;
  const mbR = mb < 10 ? Math.round(mb * 10) / 10 : Math.round(mb);
  if (mbR < 1_000) return `${mbR < 10 ? mbFmt.format(mbR) : fmtNum(mbR)} MB`;
  return `${mbFmt.format(b / 1_000_000_000)} GB`;
}

export function greeting(now = new Date()): string {
  const h = now.getHours();
  if (h >= 6 && h < 13) return "Buenos días";
  if (h >= 13 && h < 20) return "Buenas tardes";
  return "Buenas noches";
}
