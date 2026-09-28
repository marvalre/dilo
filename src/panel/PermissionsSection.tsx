import { useEffect, useState } from "react";
import { api, type Permissions } from "./api";

function PermRow({ label, ok, onOpen }: { label: string; ok: boolean | undefined; onOpen: () => void }) {
  return (
    <div className="row">
      <span>{label}</span>
      {ok === undefined ? (
        <span className="muted small">…</span>
      ) : ok ? (
        <span className="check" aria-label="Concedido">✓</span>
      ) : (
        <button className="btn btn-sm" onClick={onOpen}>
          Abrir ajustes
        </button>
      )}
    </div>
  );
}

export default function PermissionsSection() {
  const [p, setP] = useState<Permissions | null>(null);

  useEffect(() => {
    const poll = () => api.permissions().then(setP, () => {});
    poll();
    const t = window.setInterval(poll, 2000);
    return () => window.clearInterval(t);
  }, []);

  const ignore = () => {};
  return (
    <section className="card">
      <h3 className="card-title">Permisos</h3>
      <PermRow
        label="Accesibilidad"
        ok={p?.accessibility}
        onOpen={() => api.openAccessibilitySettings().catch(ignore)}
      />
      <PermRow label="Micrófono" ok={p?.microphone} onOpen={() => api.openMicrophoneSettings().catch(ignore)} />
    </section>
  );
}
