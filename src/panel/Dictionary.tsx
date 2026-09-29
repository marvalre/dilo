import { useEffect, useState, type KeyboardEvent } from "react";
import { api, type Rule, type RuleKind } from "./api";
import { errMsg } from "./format";
import { ArrowIcon, PencilIcon, TrashIcon } from "./icons";
import { AutoTextarea, PageHeader, Segmented, Switch } from "./ui";

const COPY: Record<RuleKind, { explain: [string, string, string]; from: string; to: string; emptyTitle: string; emptyText: string }> = {
  correction: {
    explain: ["Cuando el modelo escriba algo mal, Dilo lo corrige solo. Ej:", "cloud", "Claude"],
    from: "Escuchado",
    to: "Escribir",
    emptyTitle: "Sin correcciones",
    emptyText: "Añade palabras que el modelo suele escribir mal: nombres, marcas o términos técnicos.",
  },
  shortcut: {
    explain: ["Di una frase y Dilo pega otra cosa. Ej:", "mi correo", "tu@email.com"],
    from: "Cuando digo",
    to: "Pegar",
    emptyTitle: "Sin atajos",
    emptyText: "Crea atajos para textos que repites: tu correo, una dirección o una firma.",
  },
};

export default function Dictionary() {
  const [kind, setKind] = useState<RuleKind>("correction");
  const [rules, setRules] = useState<Rule[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.rules().then(setRules, (e) => setError(errMsg(e)));
  }, []);

  const upsert = (r: Rule) =>
    setRules((prev) => {
      const list = prev ?? [];
      return list.some((x) => x.id === r.id) ? list.map((x) => (x.id === r.id ? r : x)) : [r, ...list];
    });

  const save = async (r: Rule | (Omit<Rule, "id"> & { id: null })) => {
    setError(null);
    try {
      upsert(await api.saveRule(r));
      return true;
    } catch (e) {
      setError(errMsg(e));
      return false;
    }
  };

  const remove = async (id: number) => {
    setError(null);
    const prev = rules;
    setRules((l) => l?.filter((x) => x.id !== id) ?? null);
    try {
      await api.deleteRule(id);
    } catch (e) {
      setRules(prev);
      setError(errMsg(e));
    }
  };

  const c = COPY[kind];
  const list = (rules ?? []).filter((r) => r.kind === kind);

  return (
    <>
      <PageHeader title="Diccionario">
        <Segmented
          label="Tipo de regla"
          value={kind}
          onChange={setKind}
          options={[
            { value: "correction", label: "Correcciones" },
            { value: "shortcut", label: "Atajos" },
          ]}
        />
      </PageHeader>
      <p className="explain">
        {c.explain[0]} <span className="token">{c.explain[1]}</span> → <span className="token">{c.explain[2]}</span>
      </p>

      <AddRule key={kind} kind={kind} onAdd={(from, to) => save({ id: null, kind, from, to, enabled: true })} />

      {error && <p className="error-text">{error}</p>}

      {rules && list.length === 0 ? (
        <div className="empty small-empty">
          <p className="empty-title">{c.emptyTitle}</p>
          <p className="empty-text">{c.emptyText}</p>
        </div>
      ) : (
        list.length > 0 && (
          <div className="group">
            {list.map((r) => (
              <RuleRow key={r.id} r={r} onSave={save} onDelete={() => remove(r.id)} />
            ))}
          </div>
        )
      )}
    </>
  );
}

function AddRule({ kind, onAdd }: { kind: RuleKind; onAdd: (from: string, to: string) => Promise<boolean> }) {
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const c = COPY[kind];
  const ok = from.trim() !== "" && to.trim() !== "";
  const submit = async () => {
    if (!ok) return;
    if (await onAdd(from.trim(), kind === "shortcut" ? to.replace(/^\s+|\s+$/g, "") : to.trim())) {
      setFrom("");
      setTo("");
    }
  };
  return (
    <form
      className="add-rule"
      onSubmit={(e) => {
        e.preventDefault();
        submit();
      }}
    >
      <label className="field-wrap">
        <span className="field-label">{c.from}</span>
        <input
          className="field"
          value={from}
          onChange={(e) => setFrom(e.target.value)}
          placeholder={kind === "correction" ? "cloud" : "mi correo"}
          spellCheck={false}
        />
      </label>
      <span className="add-arrow" aria-hidden>
        <ArrowIcon />
      </span>
      <label className="field-wrap">
        <span className="field-label">{c.to}</span>
        {kind === "shortcut" ? (
          <AutoTextarea
            className="field"
            value={to}
            onChange={(e) => setTo(e.target.value)}
            placeholder="tu@email.com"
            spellCheck={false}
            onKeyDown={(e) => {
              if (e.nativeEvent.isComposing || e.keyCode === 229) return;
              if (e.key === "Enter" && e.metaKey) {
                e.preventDefault();
                submit();
              }
            }}
          />
        ) : (
          <input className="field" value={to} onChange={(e) => setTo(e.target.value)} placeholder="Claude" spellCheck={false} />
        )}
      </label>
      <button type="submit" className="btn btn-primary add-btn" disabled={!ok}>
        Añadir
      </button>
    </form>
  );
}

function RuleRow({
  r,
  onSave,
  onDelete,
}: {
  r: Rule;
  onSave: (r: Rule) => Promise<boolean>;
  onDelete: () => void;
}) {
  const [editing, setEditing] = useState(false);
  const [from, setFrom] = useState(r.from);
  const [to, setTo] = useState(r.to);

  const cancel = () => {
    setFrom(r.from);
    setTo(r.to);
    setEditing(false);
  };
  const commit = async () => {
    if (!from.trim() || !to.trim()) return;
    if (await onSave({ ...r, from: from.trim(), to: r.kind === "shortcut" ? to.replace(/^\s+|\s+$/g, "") : to.trim() })) setEditing(false);
  };
  const keys = (e: KeyboardEvent) => {
    if (e.nativeEvent.isComposing || e.keyCode === 229) return;
    if (e.key === "Escape") cancel();
    else if (e.key === "Enter" && (e.metaKey || !(e.target instanceof HTMLTextAreaElement))) {
      e.preventDefault();
      commit();
    }
  };

  if (editing) {
    return (
      <div className="rule rule-editing">
        <input className="field" value={from} onChange={(e) => setFrom(e.target.value)} onKeyDown={keys} autoFocus spellCheck={false} />
        <span className="add-arrow" aria-hidden>
          <ArrowIcon />
        </span>
        {r.kind === "shortcut" ? (
          <AutoTextarea className="field" value={to} onChange={(e) => setTo(e.target.value)} onKeyDown={keys} spellCheck={false} />
        ) : (
          <input className="field" value={to} onChange={(e) => setTo(e.target.value)} onKeyDown={keys} spellCheck={false} />
        )}
        <div className="rule-edit-actions">
          <button className="btn btn-sm" onClick={cancel}>
            Cancelar
          </button>
          <button className="btn btn-sm btn-primary" onClick={commit}>
            Guardar
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className={r.enabled ? "rule" : "rule disabled"}>
      <div className="rule-text">
        <span className="rule-from">{r.from}</span>
        <span className="rule-arrow" aria-hidden>
          <ArrowIcon />
        </span>
        <span className="rule-to">{r.to}</span>
      </div>
      <div className="rule-actions">
        <button className="icon-btn" onClick={() => setEditing(true)} title="Editar" aria-label="Editar">
          <PencilIcon />
        </button>
        <button className="icon-btn danger" onClick={onDelete} title="Eliminar" aria-label="Eliminar">
          <TrashIcon />
        </button>
      </div>
      <Switch label={r.enabled ? "Desactivar" : "Activar"} checked={r.enabled} onChange={(enabled) => onSave({ ...r, enabled })} />
    </div>
  );
}
