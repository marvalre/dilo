// Contract between the panel webview and the Rust core (src-tauri/src/commands.rs).
// Keep names in sync with the #[tauri::command] functions.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface Dictation {
  id: number;
  created_at: number; // unix ms
  duration_ms: number;
  text: string;
  word_count: number;
  language: string | null;
  app_name: string | null;
  error: string | null;
}

export interface Stats {
  total_words: number;
  words_today: number;
  words_week: number;
  total_dictations: number;
  avg_words_per_day: number;
  avg_words_per_dictation: number;
  avg_wpm: number;
  current_streak: number;
  best_streak: number;
  time_saved_min: number;
  last_30_days: [string, number][]; // oldest first, "YYYY-MM-DD"
  languages: [string, number][]; // largest first, ISO code or "?"
}

export interface Settings {
  hotkey: string; // handy-keys syntax: "Fn", "CtrlRight", "OptRight", "Ctrl+Space"
  language: string; // "auto" | ISO 639-1
  idle_unload_min: number; // 0 = never
  mascot_enabled: boolean;
  mic: string | null; // null = system default
}

export interface ModelStatus {
  ready: boolean;
  downloading: boolean;
  done: number; // bytes
  total: number; // bytes
  loaded: boolean; // currently in RAM
  error: string | null;
}

export interface Permissions {
  accessibility: boolean;
  microphone: boolean;
}

export const api = {
  history: (query: string, offset: number) =>
    invoke<Dictation[]>("get_history", { query, offset }),
  deleteDictation: (id: number) => invoke<void>("delete_dictation", { id }),
  stats: () => invoke<Stats>("get_stats"),
  settings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }), // rejects with a message string if the hotkey is invalid
  microphones: () => invoke<string[]>("list_microphones"),
  modelStatus: () => invoke<ModelStatus>("model_status"),
  downloadModel: () => invoke<void>("download_model"),
  copyText: (text: string) => invoke<void>("copy_text", { text }),
  permissions: () => invoke<Permissions>("permissions"),
  openAccessibilitySettings: () => invoke<void>("open_accessibility_settings"),
  openMicrophoneSettings: () => invoke<void>("open_microphone_settings"),
};

export const events = {
  onHistoryChanged: (cb: () => void): Promise<UnlistenFn> => listen("history://changed", () => cb()),
  onModelStatus: (cb: (s: ModelStatus) => void): Promise<UnlistenFn> =>
    listen<ModelStatus>("model://status", (e) => cb(e.payload)),
  /** Rust asks the panel to show a tab: "history" | "stats" | "settings". */
  onShowTab: (cb: (tab: string) => void): Promise<UnlistenFn> =>
    listen<string>("panel://tab", (e) => cb(e.payload)),
};
