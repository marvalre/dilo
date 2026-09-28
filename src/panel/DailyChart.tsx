import { useState } from "react";
import { fmtDayMonth, fmtNum, parseDay, todayKey } from "./format";

export default function DailyChart({ days }: { days: [string, number][] }) {
  const [hover, setHover] = useState<number | null>(null);
  if (days.length === 0) return <p className="muted small">Aún no hay datos.</p>;

  const max = Math.max(1, ...days.map(([, n]) => n));
  const today = todayKey();
  const mid = Math.floor((days.length - 1) / 2);
  const labelIdx = new Set([0, mid, days.length - 1]);
  const h = hover !== null ? days[hover] : null;

  return (
    <div className="chart" onMouseLeave={() => setHover(null)}>
      <div className="chart-bars">
        {days.map(([key, n], i) => (
          <div
            key={key}
            className={"bar-slot" + (hover === i ? " hover" : "")}
            onMouseEnter={() => setHover(i)}
            aria-label={`${fmtDayMonth(parseDay(key))}: ${fmtNum(n)} palabras`}
          >
            <div
              className={"bar" + (key === today ? " today" : "") + (n === 0 ? " zero" : "")}
              style={{ height: n === 0 ? undefined : `${Math.max((n / max) * 100, 3)}%` }}
            />
          </div>
        ))}
        {h && hover !== null && (
          <div
            className={"tooltip" + (hover < 5 ? " edge-l" : hover > days.length - 6 ? " edge-r" : "")}
            style={{ left: `${((hover + 0.5) / days.length) * 100}%` }}
          >
            {fmtDayMonth(parseDay(h[0]))} · {fmtNum(h[1])} {h[1] === 1 ? "palabra" : "palabras"}
          </div>
        )}
      </div>
      <div className="chart-axis">
        {[...labelIdx].map((i) => (
          <span
            key={i}
            className={"axis-label" + (i === 0 ? " first" : i === days.length - 1 ? " last" : "")}
            style={{ left: `${((i + 0.5) / days.length) * 100}%` }}
          >
            {fmtDayMonth(parseDay(days[i][0]))}
          </span>
        ))}
      </div>
    </div>
  );
}
