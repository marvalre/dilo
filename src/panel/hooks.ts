import { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { api, events, type ModelStatus, type Settings, type UpdateInfo } from "./api";
import { todayKey } from "./format";

/** Subscribe to a Tauri event for the lifetime of the component. Fails silently outside Tauri. */
export function useTauriEvent<T extends unknown[]>(
  subscribe: (cb: (...args: T) => void) => Promise<UnlistenFn>,
  handler: (...args: T) => void,
) {
  const ref = useRef(handler);
  ref.current = handler;
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let disposed = false;
    subscribe((...args) => ref.current(...args))
      .then((u) => (disposed ? u() : (unlisten = u)))
      .catch(() => {});
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [subscribe]);
}

export interface SettingsCtx {
  settings: Settings | null;
  /** Merges `patch` into the latest settings and saves. Optimistic; on error reverts only the
   *  patched fields that no later save has touched, and rethrows. */
  save: (patch: Partial<Settings>) => Promise<void>;
}

export const SettingsContext = createContext<SettingsCtx>({ settings: null, save: async () => {} });
export const useSettings = () => useContext(SettingsContext);

/** Owns the settings state for the whole window (used once, in App). */
export function useSettingsState(): SettingsCtx {
  const [settings, setSettings] = useState<Settings | null>(null);
  const current = useRef<Settings | null>(null);
  const confirmed = useRef<Settings | null>(null);
  const reqId = useRef(0);
  const lastWrite = useRef<Partial<Record<keyof Settings, number>>>({});

  const apply = useCallback((s: Settings) => {
    current.current = s;
    setSettings(s);
  }, []);
  const fromBackend = useCallback(
    (s: Settings) => {
      confirmed.current = s;
      apply(s);
    },
    [apply],
  );

  useEffect(() => {
    api.settings().then(fromBackend).catch(() => {});
  }, [fromBackend]);
  useTauriEvent(events.onSettingsChanged, fromBackend);

  const save = useCallback(
    async (patch: Partial<Settings>) => {
      const base = current.current;
      if (!base) throw new Error("Los ajustes aún no se han cargado.");
      const id = ++reqId.current;
      const keys = Object.keys(patch) as (keyof Settings)[];
      for (const k of keys) lastWrite.current[k] = id;
      const next = { ...base, ...patch };
      apply(next);
      try {
        await api.saveSettings(next);
        if (confirmed.current) confirmed.current = { ...confirmed.current, ...patch };
      } catch (e) {
        const latest = current.current;
        const good = confirmed.current ?? base;
        if (latest) {
          const revert: Partial<Settings> = {};
          for (const k of keys) {
            if (lastWrite.current[k] === id) (revert as Record<string, unknown>)[k] = good[k];
          }
          if (Object.keys(revert).length) apply({ ...latest, ...revert });
        }
        throw e;
      }
    },
    [apply],
  );

  return { settings, save };
}

/** Local "YYYY-MM-DD" for today; updates on window focus and every minute. */
export function useTodayKey(): string {
  const [key, setKey] = useState(todayKey);
  useEffect(() => {
    const on = () => setKey(todayKey());
    const id = window.setInterval(on, 60_000);
    window.addEventListener("focus", on);
    document.addEventListener("visibilitychange", on);
    return () => {
      window.clearInterval(id);
      window.removeEventListener("focus", on);
      document.removeEventListener("visibilitychange", on);
    };
  }, []);
  return key;
}

export function useModelStatus(): ModelStatus | null {
  const [m, setM] = useState<ModelStatus | null>(null);
  useEffect(() => {
    api.modelStatus().then(setM).catch(() => {});
  }, []);
  useTauriEvent(events.onModelStatus, setM);
  return m;
}

export function useMediaQuery(query: string): boolean {
  const [match, setMatch] = useState(() => window.matchMedia(query).matches);
  useEffect(() => {
    const mq = window.matchMedia(query);
    const on = () => setMatch(mq.matches);
    mq.addEventListener("change", on);
    return () => mq.removeEventListener("change", on);
  }, [query]);
  return match;
}

export const useReducedMotion = () => useMediaQuery("(prefers-reduced-motion: reduce)");

/** True while the document is visible (to pause animations in the background). */
export function usePageVisible(): boolean {
  const [v, setV] = useState(!document.hidden);
  useEffect(() => {
    const on = () => setV(!document.hidden);
    document.addEventListener("visibilitychange", on);
    return () => document.removeEventListener("visibilitychange", on);
  }, []);
  return v;
}

export type UpdatePhase = "idle" | "checking" | "uptodate" | "available" | "downloading" | "error";

/** State of the in-app updater: what the background check found, plus manual check / install. */
export function useUpdate() {
  const [phase, setPhase] = useState<UpdatePhase>("idle");
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.pendingUpdate().then((u) => {
      if (u) {
        setInfo(u);
        setPhase("available");
      }
    }).catch(() => {});
  }, []);
  useTauriEvent(events.onUpdateAvailable, (u: UpdateInfo) => {
    setInfo(u);
    setPhase((p) => (p === "downloading" ? p : "available"));
  });
  useTauriEvent(events.onUpdateProgress, (p: { done: number; total: number }) => setProgress(p));

  const check = useCallback(async () => {
    setError(null);
    setPhase("checking");
    try {
      const u = await api.checkUpdate();
      setInfo(u);
      setPhase(u ? "available" : "uptodate");
    } catch (e) {
      setError(typeof e === "string" ? e : String(e));
      setPhase("error");
    }
  }, []);

  const install = useCallback(async () => {
    setError(null);
    setProgress(null);
    setPhase("downloading");
    try {
      await api.installUpdate(); // the app relaunches itself on success
    } catch (e) {
      setError(typeof e === "string" ? e : String(e));
      setPhase("error");
    }
  }, []);

  return { phase, info, progress, error, check, install };
}
