import { useEffect, useState } from "react";

const PRESETS: { value: string; label: string }[] = [
  { value: "Fn", label: "Fn / 🌐" },
  { value: "OptRight", label: "Option derecha" },
  { value: "CmdRight", label: "Command derecha" },
  { value: "CtrlRight", label: "Control derecha" },
  { value: "Ctrl+Space", label: "Ctrl+Space" },
];
const CUSTOM = "__custom__";

interface Props {
  value: string;
  error: string | null;
  onChange: (hotkey: string) => void;
}

export default function HotkeyField({ value, error, onChange }: Props) {
  const isPreset = PRESETS.some((p) => p.value === value);
  const [custom, setCustom] = useState(!isPreset);
  const [draft, setDraft] = useState(isPreset ? "" : value);

  useEffect(() => {
    if (!PRESETS.some((p) => p.value === value)) {
      setCustom(true);
      setDraft(value);
    }
  }, [value]);

  const commit = () => {
    const v = draft.trim();
    if (v && v !== value) onChange(v);
  };

  return (
    <section className="card">
      <h3 className="card-title">Tecla para dictar</h3>
      <select
        className="field"
        value={custom ? CUSTOM : value}
        onChange={(e) => {
          if (e.target.value === CUSTOM) {
            setCustom(true);
            setDraft(value);
          } else {
            setCustom(false);
            onChange(e.target.value);
          }
        }}
      >
        {PRESETS.map((p) => (
          <option key={p.value} value={p.value}>
            {p.label}
          </option>
        ))}
        <option value={CUSTOM}>Personalizada…</option>
      </select>
      {custom && (
        <input
          className="field"
          placeholder="p. ej. Cmd+Shift+D"
          value={draft}
          spellCheck={false}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => e.key === "Enter" && commit()}
        />
      )}
      <p className="hint">Mantén la tecla presionada mientras hablas y suéltala para pegar el texto.</p>
      {error && <p className="error-text">{error}</p>}
    </section>
  );
}
