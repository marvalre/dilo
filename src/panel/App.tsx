import { useEffect, useRef, useState, type ReactNode } from "react";
import { events } from "./api";
import { SettingsContext, usePageVisible, useReducedMotion, useSettings, useSettingsState, useTauriEvent } from "./hooks";
import { hotkeyLabel } from "./hotkey";
import { BookIcon, ClockIcon, FaceIcon, GearIcon, HomeIcon } from "./icons";
import { MascotFace } from "../shared/MascotFace";
import { useMascotAnim } from "../shared/useMascotAnim";
import Home from "./Home";
import History from "./History";
import Dictionary from "./Dictionary";
import MascotPage from "./MascotPage";
import SettingsPage from "./SettingsPage";
import { ErrorBoundary } from "./ui";

export type Section = "home" | "history" | "dictionary" | "mascot" | "settings";

const NAV: { id: Section; label: string; icon: ReactNode }[] = [
  { id: "home", label: "Inicio", icon: <HomeIcon /> },
  { id: "history", label: "Historial", icon: <ClockIcon /> },
  { id: "dictionary", label: "Diccionario", icon: <BookIcon /> },
  { id: "mascot", label: "Monito", icon: <FaceIcon /> },
  { id: "settings", label: "Ajustes", icon: <GearIcon /> },
];

function toSection(raw: string): Section | null {
  const t = raw.replace(/^#/, "");
  if (t === "stats") return "home";
  return NAV.some((n) => n.id === t) ? (t as Section) : null;
}

export default function App() {
  const settingsState = useSettingsState();
  const [section, setSection] = useState<Section>(() => toSection(window.location.hash) ?? "home");
  const contentRef = useRef<HTMLElement>(null);

  useEffect(() => {
    contentRef.current?.scrollTo(0, 0);
    if (window.location.hash !== `#${section}`) history.replaceState(null, "", `#${section}`);
  }, [section]);
  useTauriEvent(events.onShowTab, (t) => {
    const s = toSection(t);
    if (s) setSection(s);
  });

  return (
    <SettingsContext.Provider value={settingsState}>
      <div className="app">
        <aside className="sidebar">
          <SidebarBrand />
          <nav className="nav" aria-label="Secciones">
            {NAV.map((n) => (
              <button
                key={n.id}
                className={section === n.id ? "nav-item active" : "nav-item"}
                aria-current={section === n.id ? "page" : undefined}
                onClick={() => setSection(n.id)}
              >
                <span className="nav-icon">{n.icon}</span>
                {n.label}
              </button>
            ))}
          </nav>
          <HotkeyHint />
        </aside>
        <main className="content" ref={contentRef}>
          <div className="content-inner" key={section}>
            <ErrorBoundary>
              {section === "home" && <Home onNavigate={setSection} />}
              {section === "history" && <History />}
              {section === "dictionary" && <Dictionary />}
              {section === "mascot" && <MascotPage />}
              {section === "settings" && <SettingsPage />}
            </ErrorBoundary>
          </div>
        </main>
      </div>
    </SettingsContext.Provider>
  );
}

function SidebarBrand() {
  const { settings } = useSettings();
  const reduced = useReducedMotion();
  const visible = usePageVisible();
  const target = useRef(0.2);
  useEffect(() => {
    if (reduced) return;
    const start = performance.now();
    const id = window.setInterval(() => {
      const t = (performance.now() - start) / 1000;
      target.current = 0.22 + 0.16 * Math.sin(t * 1.3) + 0.06 * Math.sin(t * 3.1);
    }, 60);
    return () => window.clearInterval(id);
  }, [reduced]);
  const frame = useMascotAnim("listening", target, visible && !reduced);
  return (
    <div className="brand">
      <span className="brand-mascot">
        <MascotFace
          skin={settings?.mascot_skin ?? "glass"}
          size="s"
          scale={0.9}
          mode="listening"
          level={reduced ? 0.25 : frame.level}
          t={reduced ? 0 : frame.t}
          blink={frame.blink}
          uid="brand"
        />
      </span>
      <span className="brand-name">Dilo</span>
    </div>
  );
}

function HotkeyHint() {
  const { settings } = useSettings();
  if (!settings) return null;
  return (
    <div className="hint-card">
      <span className="hint-dot" aria-hidden />
      <span>
        Mantén <strong>{hotkeyLabel(settings.hotkey)}</strong> y habla
      </span>
    </div>
  );
}
