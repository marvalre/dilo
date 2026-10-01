import { useEffect, useState, type ReactNode } from "react";
import { api, type Permissions, type Settings } from "./api";
import { useModelStatus, useSettings, useUpdate } from "./hooks";
import { getVersion } from "@tauri-apps/api/app";
import { errMsg, fmtNum, langName } from "./format";
import { hotkeyCaps } from "./hotkey";
import { CheckIcon } from "./icons";
import { Keycaps, PageHeader, Select, Switch } from "./ui";

const LANGS = ["es", "en"];
const DEFAULT_MIC = "__default__";

function Row({ label, sub, children }: { label: ReactNode; sub?: ReactNode; children?: ReactNode }) {
  return (
    <div className="grow">
      <div className="grow-label">
        <span>{label}</span>
        {sub && <span className="grow-sub">{sub}</span>}
      </div>
      {children != null && <div className="grow-control">{children}</div>}
    </div>
  );
}

function Group({ title, children, footer }: { title?: string; children: ReactNode; footer?: ReactNode }) {
  return (
    <section className="group-wrap">
      {title && <h2 className="section-label">{title}</h2>}
      <div className="group">{children}</div>
      {footer && <p className="group-footer">{footer}</p>}
    </section>
  );
}

function useAppVersion(): string {
  const [v, setV] = useState("");
  useEffect(() => {
    getVersion().then(setV).catch(() => {});
  }, []);
  return v;
}

function UpdatesGroup({ autoUpdate, onAutoUpdate }: { autoUpdate: boolean; onAutoUpdate: (v: boolean) => void }) {
  const { phase, info, progress, error, check, install } = useUpdate();
  const version = useAppVersion();
  const known = !!progress && progress.total > 0;
  const pct = known ? Math.min(100, (progress!.done / progress!.total) * 100) : 0;

  let sub: ReactNode = `Versión ${version}`;
  if (phase === "checking") sub = "Buscando…";
  else if (phase === "uptodate") sub = `Tienes la última versión (${version}).`;
  else if (phase === "available" && info) sub = `Hay una versión nueva: ${info.version}. Tu versión es ${info.current}.`;
  else if (phase === "downloading") sub = known ? `Descargando… ${Math.round(pct)} %` : "Descargando…";
  else if (phase === "error") sub = <span className="error-text">{error}</span>;

  return (
    <Group title="Actualizaciones" footer="Al actualizar, Dilo se reinicia sola. No tienes que volver a dar permisos.">
      <Row label="Dilo" sub={sub}>
        {phase === "available" || phase === "downloading" ? (
          <button className="btn btn-primary" onClick={install} disabled={phase === "downloading"}>
            {phase === "downloading" ? "Actualizando…" : `Actualizar a ${info?.version}`}
          </button>
        ) : (
          <button className="btn" onClick={check} disabled={phase === "checking"}>
            Buscar actualizaciones
          </button>
        )}
      </Row>
      <Row label="Buscar automáticamente" sub="Revisa cada pocas horas y te avisa en Inicio y en el menú.">
        <Switch label="Buscar actualizaciones automáticamente" checked={autoUpdate} onChange={onAutoUpdate} />
      </Row>
    </Group>
  );
}

export default function SettingsPage() {
  const { settings, save } = useSettings();
  const [error, setError] = useState<string | null>(null);
  const [mics, setMics] = useState<string[]>([]);
  const version = useAppVersion();

  useEffect(() => {
    api.microphones().then(setMics).catch(() => {});
  }, []);

  if (!settings) return <PageHeader title="Ajustes" />;

  const set = (patch: Partial<Settings>) => {
    setError(null);
    save(patch).catch((e) => setError(errMsg(e)));
  };

  const micList = settings.mic && !mics.includes(settings.mic) ? [settings.mic, ...mics] : mics;

  return (
    <>
      <PageHeader title="Ajustes" />
      {error && <p className="error-text banner-error">{error}</p>}

      <Group
        title="Dictado"
        footer={
          <>
            Consejo: si usas fn, en Ajustes del Sistema → Teclado pon «Al pulsar 🌐» en «No hacer nada».
          </>
        }
      >
        <HotkeyRow hotkey={settings.hotkey} onSave={(hotkey) => save({ hotkey })} />
        <Row label="Idioma principal" sub="Etiqueta tus dictados y estadísticas. Dilo entiende español e inglés por sí solo.">
          <Select label="Idioma" value={settings.language} onChange={(v) => set({ language: v })}>
            {LANGS.map((c) => (
              <option key={c} value={c}>
                {langName(c)}
              </option>
            ))}
          </Select>
        </Row>
        <Row label="Micrófono">
          <Select label="Micrófono" value={settings.mic ?? DEFAULT_MIC} onChange={(v) => set({ mic: v === DEFAULT_MIC ? null : v })}>
            <option value={DEFAULT_MIC}>Predeterminado del sistema</option>
            {micList.map((m) => (
              <option key={m} value={m}>
                {m}
              </option>
            ))}
          </Select>
        </Row>
      </Group>

      <Group title="Modelo">
        <ModelRow />
        <Row label="Liberar el modelo tras" sub="Libera memoria cuando no dictas. Volver a cargarlo tarda un segundo.">
          <Select label="Liberar el modelo tras" value={String(settings.idle_unload_min)} onChange={(v) => set({ idle_unload_min: Number(v) })}>
            {![0, 2, 5, 10, 30].includes(settings.idle_unload_min) && (
              <option value={settings.idle_unload_min}>{fmtNum(settings.idle_unload_min)} min</option>
            )}
            {[2, 5, 10, 30].map((n) => (
              <option key={n} value={n}>
                {n} min
              </option>
            ))}
            <option value="0">Nunca</option>
          </Select>
        </Row>
      </Group>

      <Group title="General">
        <Row label="Abrir al iniciar sesión">
          <Switch label="Abrir al iniciar sesión" checked={settings.launch_at_login} onChange={(v) => set({ launch_at_login: v })} />
        </Row>
        <Row label="Aviso de almacenamiento" sub="Te avisamos en Inicio si Dilo ocupa más de esto.">
          <Select label="Aviso de almacenamiento" value={String(settings.storage_warn_mb)} onChange={(v) => set({ storage_warn_mb: Number(v) })}>
            {![500, 1000, 2000, 5000].includes(settings.storage_warn_mb) && (
              <option value={settings.storage_warn_mb}>{fmtNum(settings.storage_warn_mb)} MB</option>
            )}
            <option value="500">500 MB</option>
            <option value="1000">1 GB</option>
            <option value="2000">2 GB</option>
            <option value="5000">5 GB</option>
          </Select>
        </Row>
      </Group>

      <UpdatesGroup autoUpdate={settings.auto_update} onAutoUpdate={(v) => set({ auto_update: v })} />

      <PermissionsGroup />

      <footer className="about">Dilo {version} · open source (MIT)</footer>
    </>
  );
}

let captureActive = false;
const captureListeners = new Set<(v: boolean) => void>();
const setCaptureActive = (v: boolean) => {
  captureActive = v;
  captureListeners.forEach((l) => l(v));
};

function HotkeyRow({ hotkey, onSave }: { hotkey: string; onSave: (h: string) => Promise<void> }) {
  const [capturing, setCapturing] = useState(captureActive);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    captureListeners.add(setCapturing);
    setCapturing(captureActive);
    return () => {
      captureListeners.delete(setCapturing);
    };
  }, []);

  const capture = async () => {
    if (captureActive) return;
    setErr(null);
    setCaptureActive(true);
    try {
      const k = await api.captureHotkey();
      if (k) await onSave(k);
    } catch (e) {
      setErr(errMsg(e));
    } finally {
      setCaptureActive(false);
    }
  };

  return (
    <div className="grow">
      <div className="grow-label">
        <span>Tecla para dictar</span>
        <span className="grow-sub">
          {capturing ? "Presiona la tecla o combinación… (Esc para cancelar)" : "Mantenla presionada mientras hablas y suéltala para pegar."}
        </span>
        {err && <span className="grow-sub error-text">{err}</span>}
      </div>
      <div className="grow-control">
        {capturing ? <Keycaps caps={["…"]} pulsing /> : <Keycaps caps={hotkeyCaps(hotkey)} />}
        <button className="btn" onClick={capture} disabled={capturing}>
          Cambiar
        </button>
      </div>
    </div>
  );
}

function ModelRow() {
  const m = useModelStatus();
  const [err, setErr] = useState<string | null>(null);
  const download = () => {
    setErr(null);
    api.downloadModel().catch((e) => setErr(errMsg(e)));
  };
  const error = err ?? m?.error ?? null;
  if (!m) return <Row label="Modelo de voz" sub="Comprobando…" />;
  if (m.ready) {
    return (
      <Row label="Modelo de voz" sub="Parakeet v3 · listo">
        <span className={m.loaded ? "badge badge-ok" : "badge"}>{m.loaded ? "En memoria" : "Liberado"}</span>
      </Row>
    );
  }
  if (m.downloading) {
    const known = m.total > 0;
    const pct = known ? Math.min(100, (m.done / m.total) * 100) : 0;
    return (
      <Row
        label="Modelo de voz"
        sub={
          known
            ? `Descargando… ${fmtNum(m.done / 1_000_000)} de ${fmtNum(m.total / 1_000_000)} MB`
            : `Descargando… ${fmtNum(m.done / 1_000_000)} MB`
        }
      >
        <div className={known ? "progress progress-inline" : "progress progress-inline indeterminate"}>
          <div className="progress-fill" style={known ? { width: `${pct}%` } : undefined} />
        </div>
      </Row>
    );
  }
  return (
    <Row label="Modelo de voz" sub={error ? <span className="error-text">{error}</span> : "Parakeet v3 · no descargado (unos 670 MB)"}>
      <button className="btn btn-primary" onClick={download}>
        Descargar
      </button>
    </Row>
  );
}

function PermissionsGroup() {
  const [p, setP] = useState<Permissions | null>(null);
  useEffect(() => {
    const poll = () => api.permissions().then(setP).catch(() => {});
    poll();
    const id = window.setInterval(poll, 2000);
    return () => window.clearInterval(id);
  }, []);
  const item = (ok: boolean | undefined, open: () => Promise<void>) =>
    ok ? (
      <span className="perm-ok">
        <CheckIcon size={13} /> Concedido
      </span>
    ) : (
      <button className="btn" onClick={() => open().catch(() => {})} disabled={!p}>
        Abrir ajustes
      </button>
    );
  return (
    <Group title="Permisos">
      <Row label="Accesibilidad" sub="Para pegar el texto donde está el cursor.">
        {item(p?.accessibility, api.openAccessibilitySettings)}
      </Row>
      <Row label="Micrófono" sub="Para escucharte mientras mantienes la tecla.">
        {item(p?.microphone, api.openMicrophoneSettings)}
      </Row>
    </Group>
  );
}
