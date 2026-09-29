// Contract between the app window and the Rust core (src-tauri/src/commands.rs).
// Keep names in sync with the #[tauri::command] functions.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { MascotSize, Skin } from "../shared/MascotFace";

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
  hotkey: string; // handy-keys syntax: "Fn", "CtrlRight", "OptRight", "Cmd+Shift+D"
  language: string; // "auto" | ISO 639-1
  idle_unload_min: number; // 0 = never
  mascot_enabled: boolean;
  mascot_skin: Skin; // "wave" | "glass" | "jolly" | "dot" | "aura"
  mascot_size: MascotSize; // "s" | "m" | "l"
  mic: string | null; // null = system default
  launch_at_login: boolean;
  storage_warn_mb: number; // warn when total storage exceeds this
  auto_update: boolean; // look for new versions in the background
}

export interface ModelStatus {
  ready: boolean;
  downloading: boolean;
  done: number; // bytes
  total: number; // bytes
  loaded: boolean; // engine process running (model in RAM)
  error: string | null;
}

export interface Permissions {
  accessibility: boolean;
  microphone: boolean;
}

export type RuleKind = "correction" | "shortcut";

/** Dictionary rule. correction: model writes `from` → we write `to`.
 *  shortcut: you say `from` → `to` is pasted (e.g. "mi correo" → "yo@mail.com"). */
export interface Rule {
  id: number;
  kind: RuleKind;
  from: string;
  to: string;
  enabled: boolean;
}

export interface NewRule {
  id: number | null; // null = create
  kind: RuleKind;
  from: string;
  to: string;
  enabled: boolean;
}

export interface StorageInfo {
  model_bytes: number;
  history_bytes: number;
  total_bytes: number;
  warn_bytes: number; // storage_warn_mb in bytes
  dictations: number;
}

export interface UpdateInfo {
  version: string;
  current: string;
  notes: string | null;
}

export const api = {
  /** Looks for a newer version on GitHub. Resolves null when up to date. */
  checkUpdate: () => invoke<UpdateInfo | null>("check_update"),
  /** Downloads, verifies and installs the update found by checkUpdate, then relaunches the app. */
  installUpdate: () => invoke<void>("install_update"),
  /** Update the background check already found, if any. */
  pendingUpdate: () => invoke<UpdateInfo | null>("pending_update"),
  history: (query: string, offset: number) => invoke<Dictation[]>("get_history", { query, offset }),
  updateDictation: (id: number, text: string) => invoke<Dictation>("update_dictation", { id, text }),
  deleteDictation: (id: number) => invoke<void>("delete_dictation", { id }),
  /** Deletes all history, or only entries older than N days. Returns how many were deleted. */
  clearHistory: (olderThanDays: number | null) => invoke<number>("clear_history", { olderThanDays }),
  stats: () => invoke<Stats>("get_stats"),
  storage: () => invoke<StorageInfo>("storage_info"),

  settings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }), // rejects with a message string if invalid
  /** Waits (max 10 s) for the user to press a key/combination and returns it, e.g. "Fn", "Cmd+Shift+D".
   *  Returns null on Escape or timeout. Does not save — call saveSettings with the result. */
  captureHotkey: () => invoke<string | null>("capture_hotkey"),
  microphones: () => invoke<string[]>("list_microphones"),

  rules: () => invoke<Rule[]>("list_rules"),
  saveRule: (rule: NewRule) => invoke<Rule>("save_rule", { rule }),
  deleteRule: (id: number) => invoke<void>("delete_rule", { id }),

  modelStatus: () => invoke<ModelStatus>("model_status"),
  downloadModel: () => invoke<void>("download_model"),
  copyText: (text: string) => invoke<void>("copy_text", { text }),
  permissions: () => invoke<Permissions>("permissions"),
  openAccessibilitySettings: () => invoke<void>("open_accessibility_settings"),
  openMicrophoneSettings: () => invoke<void>("open_microphone_settings"),
};

export const events = {
  onUpdateAvailable: (cb: (u: UpdateInfo) => void): Promise<UnlistenFn> => listen<UpdateInfo>("update://available", (e) => cb(e.payload)),
  onUpdateProgress: (cb: (p: { done: number; total: number }) => void): Promise<UnlistenFn> =>
    listen<{ done: number; total: number }>("update://progress", (e) => cb(e.payload)),
  onHistoryChanged: (cb: () => void): Promise<UnlistenFn> => listen("history://changed", () => cb()),
  onModelStatus: (cb: (s: ModelStatus) => void): Promise<UnlistenFn> =>
    listen<ModelStatus>("model://status", (e) => cb(e.payload)),
  onSettingsChanged: (cb: (s: Settings) => void): Promise<UnlistenFn> =>
    listen<Settings>("settings://changed", (e) => cb(e.payload)),
  /** Rust asks the window to show a section: "home" | "history" | "dictionary" | "mascot" | "settings". */
  onShowTab: (cb: (tab: string) => void): Promise<UnlistenFn> => listen<string>("panel://tab", (e) => cb(e.payload)),
};
