const nf = new Intl.NumberFormat("es");
const nf1 = new Intl.NumberFormat("es", { minimumFractionDigits: 1, maximumFractionDigits: 1 });
const timeFmt = new Intl.DateTimeFormat("es", { hour: "2-digit", minute: "2-digit" });
const dayMonthFmt = new Intl.DateTimeFormat("es", { day: "numeric", month: "short" });
const fullDateFmt = new Intl.DateTimeFormat("es", { day: "numeric", month: "short", year: "numeric" });
const langNames = new Intl.DisplayNames(["es"], { type: "language" });

export const fmtNum = (n: number) => nf.format(Math.round(n));

export function fmtDuration(ms: number): string {
  const s = ms / 1000;
  if (s < 60) return `${nf1.format(s)} s`;
  const m = Math.floor(s / 60);
  return `${m} min ${Math.round(s % 60)} s`;
}

export function fmtMinutes(min: number): string {
  const total = Math.round(min);
  if (total < 60) return `${total} min`;
  const h = Math.floor(total / 60);
  const m = total % 60;
  return m ? `${h} h ${m} min` : `${h} h`;
}

export const fmtDayMonth = (d: Date) => dayMonthFmt.format(d).replace(/\.$/, "").replace("sept", "sep");

const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();

export function fmtRelative(ts: number, now = Date.now()): string {
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
