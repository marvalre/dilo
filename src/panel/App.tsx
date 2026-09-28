import { useEffect, useRef, useState } from "react";
import { api, events, type ModelStatus } from "./api";
import { useTauriEvent } from "./hooks";
import History from "./History";
import Stats from "./Stats";
import Settings from "./Settings";

type Tab = "history" | "stats" | "settings";

const TABS: { id: Tab; label: string }[] = [
  { id: "history", label: "Historial" },
  { id: "stats", label: "Estadísticas" },
  { id: "settings", label: "Ajustes" },
];

const isTab = (t: string): t is Tab => TABS.some((x) => x.id === t);

function Logo() {
  return (
    <svg className="logo" width="22" height="22" viewBox="0 0 22 22" aria-hidden="true">
      <circle cx="11" cy="11" r="10.5" fill="#F5EEE6" stroke="rgba(0,0,0,0.08)" />
      <circle cx="7.6" cy="9.4" r="1.5" fill="#1C1C22" />
      <circle cx="14.4" cy="9.4" r="1.5" fill="#1C1C22" />
      <path
        d="M7.4 14.2 q0.9 -0.8 1.8 0 t1.8 0 t1.8 0 t1.8 0"
        fill="none"
        stroke="#1C1C22"
        strokeWidth="1.2"
        strokeLinecap="round"
      />
    </svg>
  );
}

export default function App() {
  const [tab, setTab] = useState<Tab>(() => {
    const fromUrl = window.location.hash.slice(1);
    return isTab(fromUrl) ? fromUrl : "history";
  });
  const [model, setModel] = useState<ModelStatus | null>(null);
  const contentRef = useRef<HTMLElement>(null);

  useEffect(() => {
    contentRef.current?.scrollTo(0, 0);
  }, [tab]);

  useEffect(() => {
    api.modelStatus().then(setModel).catch(() => {});
  }, []);
  useTauriEvent(events.onShowTab, (t) => isTab(t) && setTab(t));
  useTauriEvent(events.onModelStatus, setModel);

  return (
    <div className="app">
      <header className="header">
        <div className="brand">
          <Logo />
          <span>Dicta</span>
        </div>
        <nav className="tabs" role="tablist">
          {TABS.map((t) => (
            <button
              key={t.id}
              role="tab"
              aria-selected={tab === t.id}
              className={tab === t.id ? "tab active" : "tab"}
              onClick={() => setTab(t.id)}
            >
              {t.label}
            </button>
          ))}
        </nav>
      </header>
      {model && !model.ready && tab !== "settings" && (
        <div className="banner">
          <span>
            {model.downloading
              ? "Descargando el modelo de voz…"
              : "Descarga el modelo de voz para empezar (~670 MB)"}
          </span>
          <button className="btn btn-accent btn-sm" onClick={() => setTab("settings")}>
            {model.downloading ? "Ver progreso" : "Ir a Ajustes"}
          </button>
        </div>
      )}
      <main className="content" role="tabpanel" ref={contentRef}>
        {tab === "history" && <History />}
        {tab === "stats" && <Stats />}
        {tab === "settings" && <Settings />}
      </main>
    </div>
  );
}
