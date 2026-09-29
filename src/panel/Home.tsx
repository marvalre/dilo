import { useCallback, useEffect, useState } from "react";
import { api, events, type Stats, type StorageInfo } from "./api";
import { useModelStatus, useTauriEvent, useUpdate } from "./hooks";
import { errMsg, fmtBytes, fmtDayMonth, fmtMinutes, fmtNum, greeting, langName, parseDay, todayKey } from "./format";
import { DownloadIcon, WarnIcon } from "./icons";
import type { Section } from "./App";

export default function Home({ onNavigate }: { onNavigate: (s: Section) => void }) {
  const [stats, setStats] = useState<Stats | null>(null);
  const [storage, setStorage] = useState<StorageInfo | null>(null);

  const refresh = useCallback(() => {
    api.stats().then(setStats).catch(() => {});
    api.storage().then(setStorage).catch(() => {});
  }, []);
  useEffect(refresh, [refresh]);
  useTauriEvent(events.onHistoryChanged, refresh);

  const today = stats?.words_today ?? 0;
  const subtitle = !stats
    ? " "
    : today > 0
      ? `Hoy llevas ${fmtNum(today)} ${today === 1 ? "palabra dictada" : "palabras dictadas"}.`
      : stats.total_words > 0
        ? "Aún no has dictado hoy. Mantén la tecla y empieza a hablar."
        : "Mantén la tecla, habla y suelta. Tu texto aparece donde esté el cursor.";

  return (
    <>
      <header className="page-header">
        <div className="page-header-text">
          <h1 className="page-title">{greeting()}</h1>
          <p className="page-subtitle">{subtitle}</p>
        </div>
      </header>

      <UpdateCard />
      <ModelCard />

      <section className="hero-row">
        <Hero label="Palabras totales" value={stats ? fmtNum(stats.total_words) : "–"} />
        <Hero label="Hoy" value={stats ? fmtNum(stats.words_today) : "–"} />
        <Hero
          label="Racha"
          value={stats ? `${fmtNum(stats.current_streak)}` : "–"}
          unit={stats ? `${stats.current_streak === 1 ? "día" : "días"} 🔥` : undefined}
        />
      </section>

      <section className="stat-grid">
        <StatCard label="Promedio por día" value={stats ? fmtNum(stats.avg_words_per_day) : "–"} unit="palabras" />
        <StatCard label="Palabras por dictado" value={stats ? fmtNum(stats.avg_words_per_dictation) : "–"} />
        <StatCard label="Palabras por minuto" value={stats ? fmtNum(stats.avg_wpm) : "–"} />
        <StatCard label="Dictados" value={stats ? fmtNum(stats.total_dictations) : "–"} />
        <StatCard label="Tiempo ahorrado" value={stats ? fmtMinutes(stats.time_saved_min) : "–"} />
        <StatCard label="Esta semana" value={stats ? fmtNum(stats.words_week) : "–"} unit="palabras" />
        <StatCard
          label="Mejor racha"
          value={stats ? fmtNum(stats.best_streak) : "–"}
          unit={stats?.best_streak === 1 ? "día" : "días"}
        />
      </section>

      <section className="card">
        <div className="card-head">
          <h2 className="card-title">Últimos 30 días</h2>
          {stats && (
            <span className="card-meta">
              {fmtNum(stats.last_30_days.reduce((a, [, n]) => a + n, 0))} palabras
            </span>
          )}
        </div>
        <DailyChart days={stats?.last_30_days ?? []} />
      </section>

      <section className="card">
        <div className="card-head">
          <h2 className="card-title">Idiomas</h2>
        </div>
        <Languages langs={stats?.languages ?? []} />
      </section>

      {storage && <StorageBar s={storage} onClean={() => onNavigate("history")} />}
    </>
  );
}

function Hero({ label, value, unit }: { label: string; value: string; unit?: string }) {
  return (
    <div className="hero">
      <div className="hero-label">{label}</div>
      <div className="hero-value">
        {value}
        {unit && <span className="hero-unit">{unit}</span>}
      </div>
    </div>
  );
}

function StatCard({ label, value, unit }: { label: string; value: string; unit?: string }) {
  return (
    <div className="stat">
      <div className="stat-value">
        {value}
        {unit && <span className="stat-unit">{unit}</span>}
      </div>
      <div className="stat-label">{label}</div>
    </div>
  );
}

function UpdateCard() {
  const { phase, info, progress, error, install } = useUpdate();
  if (phase !== "available" && phase !== "downloading") return null;
  const known = !!progress && progress.total > 0;
  const pct = known ? Math.min(100, (progress!.done / progress!.total) * 100) : 0;
  return (
    <section className="model-card">
      <div className="model-card-icon">
        <DownloadIcon />
      </div>
      <div className="model-card-body">
        <h2 className="model-card-title">{phase === "downloading" ? "Actualizando Dilo…" : `Hay una versión nueva de Dilo (${info?.version})`}</h2>
        {phase === "downloading" ? (
          <div className={known ? "progress" : "progress indeterminate"} role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={known ? Math.round(pct) : undefined}>
            <div className="progress-fill" style={known ? { width: `${pct}%` } : undefined} />
          </div>
        ) : (
          <p className="model-card-text">Se instala con un clic y Dilo se reinicia sola. Tus dictados y ajustes se conservan.</p>
        )}
        {error && <p className="error-text">{error}</p>}
      </div>
      {phase === "available" && (
        <button className="btn btn-primary" onClick={install}>
          Actualizar
        </button>
      )}
    </section>
  );
}

function ModelCard() {
  const m = useModelStatus();
  const [err, setErr] = useState<string | null>(null);
  if (!m || m.ready) return null;
  const known = m.total > 0;
  const pct = known ? Math.min(100, (m.done / m.total) * 100) : 0;
  const error = err ?? m.error;
  const download = () => {
    setErr(null);
    api.downloadModel().catch((e) => setErr(errMsg(e)));
  };
  return (
    <section className="model-card">
      <div className="model-card-icon">
        <DownloadIcon />
      </div>
      <div className="model-card-body">
        <h2 className="model-card-title">{m.downloading ? "Descargando el modelo de voz…" : "Descarga el modelo de voz"}</h2>
        {m.downloading ? (
          <>
            <div
              className={known ? "progress" : "progress indeterminate"}
              role="progressbar"
              aria-valuenow={known ? Math.round(pct) : undefined}
              aria-valuemin={0}
              aria-valuemax={100}
            >
              <div className="progress-fill" style={known ? { width: `${pct}%` } : undefined} />
            </div>
            <p className="model-card-text">
              {known
                ? `${fmtNum(m.done / 1_000_000)} de ${fmtNum(m.total / 1_000_000)} MB · ${Math.round(pct)} %`
                : `${fmtNum(m.done / 1_000_000)} MB descargados`}
            </p>
          </>
        ) : (
          <p className="model-card-text">
            Dilo transcribe todo en tu Mac, sin enviar tu voz a ningún sitio. Solo necesita descargar el modelo una vez (unos 670 MB).
          </p>
        )}
        {error && <p className="error-text">{error}</p>}
      </div>
      {!m.downloading && (
        <button className="btn btn-primary" onClick={download}>
          Descargar
        </button>
      )}
    </section>
  );
}

/** Rounds up to a "nice" axis maximum: 520 → 600, 83 → 100. */
function niceCeil(n: number): number {
  const step = 10 ** Math.floor(Math.log10(n));
  const f = [1, 1.2, 1.5, 2, 2.5, 3, 4, 5, 6, 8, 10].find((x) => x * step >= n) ?? 10;
  return f * step;
}

function DailyChart({ days }: { days: [string, number][] }) {
  const [hover, setHover] = useState<number | null>(null);
  const max = niceCeil(Math.max(1, ...days.map(([, n]) => n)));
  const tk = todayKey();
  if (days.length === 0) return <div className="chart-empty">Sin datos todavía</div>;
  const label = (key: string) => fmtDayMonth(parseDay(key));
  const hovered = hover != null ? days[hover] : null;
  return (
    <div className="chart" onMouseLeave={() => setHover(null)}>
      <div className="chart-plot">
        <div className="chart-grid" aria-hidden>
          <span data-v={fmtNum(max)} />
          <span data-v={fmtNum(max / 2)} />
          <span data-v="0" />
        </div>
        <div className="chart-bars">
          {days.map(([key, n], i) => (
            <div
              key={key}
              className={`chart-col${key === tk ? " today" : ""}${hover === i ? " hover" : ""}`}
              onMouseEnter={() => setHover(i)}
              aria-label={`${label(key)}: ${n} palabras`}
            >
              <div className="chart-bar" style={{ height: n > 0 ? `max(3px, ${(n / max) * 100}%)` : "2px" }} data-zero={n === 0} />
            </div>
          ))}
        </div>
        {hovered && hover != null && (
          <div
            className="chart-tip"
            style={{
              left: `${((hover + 0.5) / days.length) * 100}%`,
              bottom: `calc(${(hovered[1] / max) * 100}% + 8px)`,
              // Keep the tooltip inside the card near the edges.
              transform: `translateX(${hover < 4 ? -20 : hover > days.length - 5 ? -80 : -50}%)`,
            }}
          >
            {label(hovered[0])} · <strong>{fmtNum(hovered[1])}</strong> {hovered[1] === 1 ? "palabra" : "palabras"}
          </div>
        )}
      </div>
      <div className="chart-axis">
        <span>{label(days[0][0])}</span>
        <span>{label(days[Math.floor(days.length / 2)][0])}</span>
        <span>Hoy</span>
      </div>
    </div>
  );
}

function Languages({ langs }: { langs: [string, number][] }) {
  const total = langs.reduce((a, [, n]) => a + n, 0);
  if (total === 0) return <div className="chart-empty">Aún no hay dictados</div>;
  return (
    <ul className="langs">
      {langs.map(([code, n]) => {
        const pct = (n / total) * 100;
        return (
          <li key={code} className="lang">
            <span className="lang-name">{langName(code)}</span>
            <span className="lang-track">
              <span className="lang-fill" style={{ width: `${Math.max(pct, 1.5)}%` }} />
            </span>
            <span className="lang-pct">{pct < 1 ? "<1" : Math.round(pct)} %</span>
          </li>
        );
      })}
    </ul>
  );
}

function StorageBar({ s, onClean }: { s: StorageInfo; onClean: () => void }) {
  const total = Math.max(1, s.total_bytes);
  const over = s.warn_bytes > 0 && s.total_bytes > s.warn_bytes;
  return (
    <section className="storage">
      <div className="storage-head">
        <span className="storage-title">Almacenamiento</span>
        <span className="storage-total">{fmtBytes(s.total_bytes)}</span>
      </div>
      <div className="storage-bar" aria-hidden>
        <span className="seg-model" style={{ width: `${(s.model_bytes / total) * 100}%` }} />
        <span className="seg-history" style={{ width: `max(3px, ${(s.history_bytes / total) * 100}%)` }} />
      </div>
      <div className="storage-legend">
        <span>
          <i className="dot dot-model" /> Modelo de voz {fmtBytes(s.model_bytes)}
        </span>
        <span>
          <i className="dot dot-history" /> Historial {fmtBytes(s.history_bytes)}
        </span>
        <span className="muted">Total {fmtBytes(s.total_bytes)}</span>
      </div>
      {over && (
        <div className="storage-warn">
          <WarnIcon />
          <span>Dilo ocupa más de {fmtBytes(s.warn_bytes)}. Puedes liberar espacio borrando dictados antiguos.</span>
          <button className="btn btn-sm" onClick={onClean}>
            Limpiar historial
          </button>
        </div>
      )}
    </section>
  );
}
