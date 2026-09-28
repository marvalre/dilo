import { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { api, events, type ModelStatus, type Settings } from "./api";

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
  /** Saves the full settings object. Optimistic; reverts and rethrows on error. */
  save: (next: Settings) => Promise<void>;
}

export const SettingsContext = createContext<SettingsCtx>({ settings: null, save: async () => {} });
export const useSettings = () => useContext(SettingsContext);

/** Owns the settings state for the whole window (used once, in App). */
export function useSettingsState(): SettingsCtx {
  const [settings, setSettings] = useState<Settings | null>(null);
  const current = useRef<Settings | null>(null);
  current.current = settings;

  useEffect(() => {
    api.settings().then(setSettings).catch(() => {});
  }, []);
  useTauriEvent(events.onSettingsChanged, setSettings);

  const save = useCallback(async (next: Settings) => {
    const prev = current.current;
    setSettings(next);
    try {
      await api.saveSettings(next);
    } catch (e) {
      setSettings(prev);
      throw e;
    }
  }, []);

  return { settings, save };
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
