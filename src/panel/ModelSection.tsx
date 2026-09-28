import { useEffect, useState } from "react";
import { api, events, type ModelStatus } from "./api";
import { useTauriEvent } from "./hooks";
import { errMsg, fmtNum } from "./format";

const mb = (bytes: number) => fmtNum(bytes / 1_000_000);

export default function ModelSection() {
  const [m, setM] = useState<ModelStatus | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    api.modelStatus().then(setM, (e) => setErr(errMsg(e)));
  }, []);
  useTauriEvent(events.onModelStatus, setM);

  const download = () => {
    setErr(null);
    api.downloadModel().catch((e) => setErr(errMsg(e)));
  };

  const pct = m && m.total > 0 ? Math.min(100, (m.done / m.total) * 100) : 0;
  const error = err ?? m?.error ?? null;

  return (
    <section className="card">
      <h3 className="card-title">Modelo de voz</h3>
      {!m ? (
        <p className="muted small">{error ?? "Comprobando…"}</p>
      ) : m.ready ? (
        <div className="row">
          <span>Parakeet v3 · listo</span>
          <span className={m.loaded ? "pill pill-ok" : "pill"}>{m.loaded ? "En memoria" : "Liberado"}</span>
        </div>
      ) : m.downloading ? (
        <div className="progress-wrap">
          <div className="progress" role="progressbar" aria-valuenow={Math.round(pct)} aria-valuemin={0} aria-valuemax={100}>
            <div className="progress-fill" style={{ width: `${pct}%` }} />
          </div>
          <div className="row small muted">
            <span>Descargando…</span>
            <span>
              {mb(m.done)} / {m.total > 0 ? mb(m.total) : "…"} MB
            </span>
          </div>
        </div>
      ) : (
        <div className="row">
          <span className="small muted">Necesario para transcribir. Se descarga una sola vez.</span>
          <button className="btn btn-accent" onClick={download}>
            Descargar (670 MB)
          </button>
        </div>
      )}
      {error && <p className="error-text">{error}</p>}
    </section>
  );
}
