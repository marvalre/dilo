import { useCallback, useEffect, useState } from "react";
import { api, events, type Stats as StatsData } from "./api";
import { useTauriEvent } from "./hooks";
import { fmtMinutes, fmtNum, langName } from "./format";
import DailyChart from "./DailyChart";

function Tile({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <div className="tile">
      <div className="tile-label">{label}</div>
      <div className="tile-value">{value}</div>
      {sub && <div className="tile-sub">{sub}</div>}
    </div>
  );
}

export default function Stats() {
  const [s, setS] = useState<StatsData | null>(null);
  const [failed, setFailed] = useState(false);

  const load = useCallback(() => {
    api.stats().then(setS, () => setFailed(true));
  }, []);
  useEffect(load, [load]);
  useTauriEvent(events.onHistoryChanged, load);

  if (!s) return <div className="empty"><p>{failed ? "No se pudieron cargar las estadísticas." : "Cargando…"}</p></div>;

  const streak = s.current_streak;
  const langTotal = s.languages.reduce((a, [, n]) => a + n, 0);

  return (
    <div className="stats">
      <div className="tiles">
        <Tile label="Palabras totales" value={fmtNum(s.total_words)} />
        <Tile label="Hoy" value={fmtNum(s.words_today)} />
        <Tile label="Esta semana" value={fmtNum(s.words_week)} />
        <Tile label="Promedio por día" value={fmtNum(s.avg_words_per_day)} />
        <Tile label="Palabras por dictado" value={fmtNum(s.avg_words_per_dictation)} />
        <Tile label="Palabras por minuto" value={fmtNum(s.avg_wpm)} />
        <Tile
          label="Racha actual"
          value={`${fmtNum(streak)} ${streak === 1 ? "día" : "días"}${streak >= 2 ? " 🔥" : ""}`}
          sub={`mejor: ${fmtNum(s.best_streak)}`}
        />
        <Tile label="Dictados" value={fmtNum(s.total_dictations)} />
        <Tile label="Tiempo ahorrado" value={fmtMinutes(s.time_saved_min)} sub="frente a escribir a 40 ppm" />
      </div>

      <section className="card">
        <h3 className="card-title">Últimos 30 días</h3>
        <DailyChart days={s.last_30_days} />
      </section>

      <section className="card">
        <h3 className="card-title">Idiomas</h3>
        {langTotal === 0 ? (
          <p className="muted small">Aún no hay datos.</p>
        ) : (
          <ul className="langs">
            {s.languages.map(([code, n]) => {
              const pct = (n / langTotal) * 100;
              return (
                <li key={code}>
                  <div className="lang-row">
                    <span>{langName(code)}</span>
                    <span className="muted">{Math.round(pct)} %</span>
                  </div>
                  <div className="lang-track">
                    <div className="lang-fill" style={{ width: `${Math.max(pct, 1.5)}%` }} />
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </section>
    </div>
  );
}
