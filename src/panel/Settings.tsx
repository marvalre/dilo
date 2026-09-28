import { useEffect, useState } from "react";
import { api, type Settings as SettingsData } from "./api";
import { errMsg, langName } from "./format";
import ModelSection from "./ModelSection";
import PermissionsSection from "./PermissionsSection";
import HotkeyField from "./HotkeyField";

// Parakeet TDT 0.6B v3 languages.
const LANGS = ["es", "en", "fr", "de", "pt", "it", "nl", "pl", "uk", "ru", "bg", "hr", "cs", "da", "et", "fi", "el", "hu", "lv", "lt", "mt", "ro", "sk", "sl", "sv"];
const UNLOAD = [2, 5, 10, 30, 0];

export default function Settings() {
  const [s, setS] = useState<SettingsData | null>(null);
  const [mics, setMics] = useState<string[]>([]);
  const [hotkeyErr, setHotkeyErr] = useState<string | null>(null);
  const [saveErr, setSaveErr] = useState<string | null>(null);

  useEffect(() => {
    api.settings().then(setS, (e) => setSaveErr(errMsg(e)));
    api.microphones().then(setMics, () => {});
  }, []);

  const update = (patch: Partial<SettingsData>) => {
    if (!s) return;
    const next = { ...s, ...patch };
    const isHotkey = "hotkey" in patch;
    setS(next);
    api.saveSettings(next).then(
      () => (isHotkey ? setHotkeyErr(null) : setSaveErr(null)),
      (e) => (isHotkey ? setHotkeyErr(errMsg(e)) : setSaveErr(errMsg(e))),
    );
  };

  return (
    <div className="settings">
      <ModelSection />
      <PermissionsSection />
      {s && (
        <>
          <HotkeyField value={s.hotkey} error={hotkeyErr} onChange={(hotkey) => update({ hotkey })} />

          <section className="card">
            <h3 className="card-title">Idioma</h3>
            <select className="field" value={s.language} onChange={(e) => update({ language: e.target.value })}>
              <option value="auto">Automático (recomendado)</option>
              {LANGS.map((c) => (
                <option key={c} value={c}>
                  {langName(c)}
                </option>
              ))}
            </select>
            <p className="hint">Parakeet detecta el idioma solo; puedes mezclar español e inglés.</p>
          </section>

          <section className="card">
            <h3 className="card-title">Micrófono</h3>
            <select className="field" value={s.mic ?? ""} onChange={(e) => update({ mic: e.target.value || null })}>
              <option value="">Predeterminado del sistema</option>
              {s.mic && !mics.includes(s.mic) && <option value={s.mic}>{s.mic} (no conectado)</option>}
              {mics.map((m) => (
                <option key={m} value={m}>
                  {m}
                </option>
              ))}
            </select>
          </section>

          <section className="card">
            <h3 className="card-title">Memoria</h3>
            <label className="row">
              <span>Liberar el modelo de la RAM tras</span>
              <select
                className="field field-inline"
                value={s.idle_unload_min}
                onChange={(e) => update({ idle_unload_min: Number(e.target.value) })}
              >
                {UNLOAD.map((n) => (
                  <option key={n} value={n}>
                    {n === 0 ? "Nunca" : `${n} min`}
                  </option>
                ))}
              </select>
            </label>
          </section>

          <section className="card">
            <h3 className="card-title">Monito</h3>
            <label className="row toggle-row">
              <span>Mostrar el monito junto al cursor</span>
              <input
                type="checkbox"
                className="switch"
                checked={s.mascot_enabled}
                onChange={(e) => update({ mascot_enabled: e.target.checked })}
              />
            </label>
          </section>
        </>
      )}
      {saveErr && <p className="error-text">{saveErr}</p>}
    </div>
  );
}
