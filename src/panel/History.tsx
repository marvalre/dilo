import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, events, type Dictation } from "./api";
import { useSettings, useTauriEvent, useTodayKey } from "./hooks";
import { dayKey, errMsg, fmtDayHeader, fmtDuration, fmtNum, fmtTime, isToday, langName, startOfWeek } from "./format";
import { hotkeyLabel } from "./hotkey";
import { CheckIcon, CopyIcon, PencilIcon, SearchIcon, TrashIcon } from "./icons";
import { AutoTextarea, ConfirmDialog, PageHeader, Toast } from "./ui";
import { MascotFace } from "../shared/MascotFace";

const PAGE = 50;
type Filter = "all" | "today" | "week";
const FILTERS: { id: Filter; label: string }[] = [
  { id: "all", label: "Todo" },
  { id: "today", label: "Hoy" },
  { id: "week", label: "Esta semana" },
];

export default function History() {
  const [input, setInput] = useState("");
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [items, setItems] = useState<Dictation[] | null>(null);
  const [hasMore, setHasMore] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [menu, setMenu] = useState(false);
  const [confirmAll, setConfirmAll] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const [hidden, setHidden] = useState<ReadonlySet<number>>(() => new Set());
  const [undoId, setUndoId] = useState<number | null>(null);
  const seq = useRef(0);
  const want = useRef(PAGE);
  const reloading = useRef(false);
  const pendingDelete = useRef<{ id: number; timer: number } | null>(null);
  const today = useTodayKey();

  useEffect(() => {
    if (!menu) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setMenu(false);
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [menu]);

  useEffect(() => {
    const t = window.setTimeout(() => setQuery(input.trim()), 200);
    return () => window.clearTimeout(t);
  }, [input]);

  /** (Re)load from the start, keeping at least `want.current` rows. */
  const reload = useCallback(async (q: string) => {
    const my = ++seq.current;
    reloading.current = true;
    try {
      let all: Dictation[] = [];
      let more = true;
      while (more && all.length < Math.max(want.current, PAGE)) {
        const page = await api.history(q, all.length);
        all = all.concat(page);
        more = page.length >= PAGE;
      }
      if (my !== seq.current) return;
      setItems(all);
      setHasMore(more);
      setError(null);
    } catch (e) {
      if (my === seq.current) setError(errMsg(e));
    } finally {
      if (my === seq.current) reloading.current = false;
    }
  }, []);

  useEffect(() => {
    want.current = PAGE;
    reload(query);
  }, [query, reload]);
  useTauriEvent(events.onHistoryChanged, () => reload(query));

  const loadMore = async () => {
    if (!items || loadingMore) return;
    want.current = items.length + PAGE;
    setLoadingMore(true);
    if (reloading.current) {
      await reload(query);
      setLoadingMore(false);
      return;
    }
    const my = ++seq.current;
    try {
      const page = await api.history(query, items.length);
      if (my !== seq.current) return;
      setItems((prev) => [...(prev ?? []), ...page.filter((p) => !prev?.some((x) => x.id === p.id))]);
      setHasMore(page.length >= PAGE);
    } catch (e) {
      if (my === seq.current) setError(errMsg(e));
    } finally {
      setLoadingMore(false);
    }
  };

  const visible = useMemo(() => {
    if (!items) return [];
    const shown = hidden.size === 0 ? items : items.filter((d) => !hidden.has(d.id));
    const now = Date.now();
    if (filter === "today") return shown.filter((d) => isToday(d.created_at, now));
    if (filter === "week") {
      const from = startOfWeek(now);
      return shown.filter((d) => d.created_at >= from);
    }
    return shown;
  }, [items, filter, hidden, today]);

  const groups = useMemo(() => {
    const out: { key: string; label: string; items: Dictation[] }[] = [];
    for (const d of visible) {
      const k = dayKey(d.created_at);
      const last = out[out.length - 1];
      if (last && last.key === k) last.items.push(d);
      else out.push({ key: k, label: fmtDayHeader(d.created_at), items: [d] });
    }
    return out;
  }, [visible, today]);

  const unhide = useCallback((id: number) => {
    setHidden((prev) => {
      if (!prev.has(id)) return prev;
      const next = new Set(prev);
      next.delete(id);
      return next;
    });
  }, []);

  const commitDelete = useCallback(() => {
    const p = pendingDelete.current;
    if (!p) return;
    pendingDelete.current = null;
    window.clearTimeout(p.timer);
    setUndoId((u) => (u === p.id ? null : u));
    api.deleteDictation(p.id).then(
      () => {
        setItems((prev) => prev?.filter((x) => x.id !== p.id) ?? null);
        unhide(p.id);
      },
      (e) => {
        setToast(errMsg(e));
        unhide(p.id);
      },
    );
  }, [unhide]);
  useEffect(() => commitDelete, [commitDelete]);

  const clear = async (days: number | null) => {
    setMenu(false);
    setConfirmAll(false);
    commitDelete();
    try {
      const n = await api.clearHistory(days);
      setToast(n === 0 ? "No había dictados que eliminar" : `Se eliminaron ${fmtNum(n)} ${n === 1 ? "dictado" : "dictados"}`);
      want.current = PAGE;
      reload(query);
    } catch (e) {
      setToast(errMsg(e));
    }
  };

  const replace = (d: Dictation) => setItems((prev) => prev?.map((x) => (x.id === d.id ? d : x)) ?? null);
  const remove = (id: number) => {
    commitDelete();
    setToast(null);
    setHidden((prev) => new Set(prev).add(id));
    setUndoId(id);
    pendingDelete.current = { id, timer: window.setTimeout(commitDelete, 4000) };
  };
  const undo = () => {
    const p = pendingDelete.current;
    if (!p) return;
    window.clearTimeout(p.timer);
    pendingDelete.current = null;
    setUndoId(null);
    unhide(p.id);
  };

  const isEmptyDb = items !== null && items.length === 0 && query === "";
  const showTools = items !== null && !isEmptyDb;

  return (
    <>
      <PageHeader title="Historial">
        {showTools && (
          <div className="menu-anchor">
            <button className="btn" aria-haspopup="menu" aria-expanded={menu} onClick={() => setMenu((v) => !v)}>
              Limpiar…
            </button>
            {menu && (
              <>
                <div className="menu-backdrop" onMouseDown={() => setMenu(false)} />
                <div className="menu" role="menu">
                  <button role="menuitem" className="menu-item" onClick={() => clear(30)}>
                    Más antiguos de 30 días
                  </button>
                  <div className="menu-sep" />
                  <button
                    role="menuitem"
                    className="menu-item danger"
                    onClick={() => {
                      setMenu(false);
                      setConfirmAll(true);
                    }}
                  >
                    Todo el historial…
                  </button>
                </div>
              </>
            )}
          </div>
        )}
      </PageHeader>

      {showTools && (
        <div className="toolbar">
          <label className="search">
            <SearchIcon />
            <input
              type="search"
              placeholder="Buscar en el historial"
              aria-label="Buscar en el historial"
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => e.key === "Escape" && !e.nativeEvent.isComposing && e.keyCode !== 229 && setInput("")}
              spellCheck={false}
            />
          </label>
          <div className="chips" role="radiogroup" aria-label="Filtrar por fecha">
            {FILTERS.map((f) => (
              <button
                key={f.id}
                role="radio"
                aria-checked={filter === f.id}
                className={filter === f.id ? "chip active" : "chip"}
                onClick={() => setFilter(f.id)}
              >
                {f.label}
              </button>
            ))}
          </div>
        </div>
      )}

      {error && <p className="error-text">{error}</p>}

      {isEmptyDb ? (
        <EmptyHistory />
      ) : items && visible.length === 0 ? (
        <div className="empty small-empty">
          <p className="empty-title">{query ? "Sin resultados" : "Nada por aquí"}</p>
          <p className="empty-text">
            {query
              ? filter === "all"
                ? `No hay dictados que contengan “${query}”.`
                : `No hay dictados de ${filter === "today" ? "hoy" : "esta semana"} que contengan “${query}”.`
              : filter === "today"
                ? "Hoy aún no has dictado nada."
                : filter === "week"
                  ? "Esta semana aún no has dictado nada."
                  : "No hay dictados que mostrar."}
          </p>
        </div>
      ) : (
        groups.map((g) => (
          <section key={g.key} className="day">
            <h2 className="day-header">{g.label}</h2>
            <div className="list">
              {g.items.map((d) => (
                <HistoryRow key={d.id} d={d} onReplace={replace} onDelete={() => remove(d.id)} onError={setToast} />
              ))}
            </div>
          </section>
        ))
      )}

      {hasMore && visible.length > 0 && (
        <div className="load-more">
          <button className="btn" onClick={loadMore} disabled={loadingMore}>
            {loadingMore ? "Cargando…" : "Cargar más"}
          </button>
        </div>
      )}

      {confirmAll && (
        <ConfirmDialog
          title="¿Borrar todo el historial?"
          message="Se eliminarán todos tus dictados de este Mac. Tus estadísticas volverán a cero. Esta acción no se puede deshacer."
          confirmLabel="Borrar todo"
          destructive
          onConfirm={() => clear(null)}
          onCancel={() => setConfirmAll(false)}
        />
      )}
      {toast ? (
        <Toast message={toast} onDone={() => setToast(null)} />
      ) : (
        undoId != null && (
          <Toast
            key={undoId}
            message="Dictado eliminado"
            duration={4000}
            action={{ label: "Deshacer", onClick: undo }}
            onDone={() => setUndoId((u) => (u === undoId ? null : u))}
          />
        )
      )}
    </>
  );
}

function EmptyHistory() {
  const { settings } = useSettings();
  return (
    <div className="empty">
      <div className="empty-mascot">
        <MascotFace skin={settings?.mascot_skin ?? "glass"} size="l" mode="listening" level={0.35} t={420} blink={false} uid="empty" />
      </div>
      <p className="empty-title">Tu historial está vacío</p>
      <p className="empty-text">
        Mantén <strong>{settings ? hotkeyLabel(settings.hotkey) : "fn"}</strong> y habla. Todo lo que dictes aparecerá aquí.
      </p>
    </div>
  );
}

function HistoryRow({
  d,
  onReplace,
  onDelete,
  onError,
}: {
  d: Dictation;
  onReplace: (d: Dictation) => void;
  onDelete: () => void;
  onError: (msg: string) => void;
}) {
  const [copied, setCopied] = useState(false);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(d.text);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!copied) return;
    const t = window.setTimeout(() => setCopied(false), 1200);
    return () => window.clearTimeout(t);
  }, [copied]);

  const copy = () => {
    api
      .copyText(d.text)
      .catch(() => navigator.clipboard.writeText(d.text))
      .then(() => setCopied(true))
      .catch((e) => onError(errMsg(e)));
  };

  const save = async () => {
    const text = draft.trim();
    if (!text) return;
    if (text === d.text) {
      setEditing(false);
      return;
    }
    setSaving(true);
    try {
      onReplace(await api.updateDictation(d.id, text));
      setEditing(false);
    } catch (e) {
      onError(errMsg(e));
    } finally {
      setSaving(false);
    }
  };

  const meta = [
    `${fmtNum(d.word_count)} ${d.word_count === 1 ? "palabra" : "palabras"}`,
    fmtDuration(d.duration_ms),
    d.app_name,
    d.language ? langName(d.language) : null,
  ].filter(Boolean);

  if (editing) {
    return (
      <div className="row row-editing">
        <span className="row-time">{fmtTime(d.created_at)}</span>
        <div className="row-body">
          <AutoTextarea
            className="edit-area"
            aria-label="Editar texto del dictado"
            value={draft}
            autoFocusEnd
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.nativeEvent.isComposing || e.keyCode === 229) return;
              if (e.key === "Enter" && e.metaKey) {
                e.preventDefault();
                save();
              } else if (e.key === "Escape") {
                e.preventDefault();
                setDraft(d.text);
                setEditing(false);
              }
            }}
          />
          <div className="edit-actions">
            <span className="edit-hint">{draft.trim() ? "⌘↩ para guardar · esc para cancelar" : "El texto no puede quedar vacío"}</span>
            <button
              className="btn btn-sm"
              onClick={() => {
                setDraft(d.text);
                setEditing(false);
              }}
            >
              Cancelar
            </button>
            <button className="btn btn-sm btn-primary" onClick={save} disabled={saving || !draft.trim()}>
              Guardar
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className={d.error ? "row row-error" : "row"}>
      <span className="row-time">{fmtTime(d.created_at)}</span>
      <div className="row-body">
        {d.error ? (
          <p className="row-text">{d.text ? d.text : <span className="row-error-msg">{d.error}</span>}</p>
        ) : (
          <p className="row-text">{d.text}</p>
        )}
        <p className="row-meta">
          {d.error && d.text ? <span className="row-error-msg">{d.error} · </span> : null}
          {meta.join(" · ")}
        </p>
      </div>
      <div className="row-actions">
        {!d.error && (
          <button className={copied ? "icon-btn done" : "icon-btn"} onClick={copy} title="Copiar" aria-label="Copiar">
            {copied ? <CheckIcon /> : <CopyIcon />}
            {copied && <span className="icon-btn-label">Copiado</span>}
          </button>
        )}
        {!d.error && (
          <button
            className="icon-btn"
            onClick={() => {
              setDraft(d.text);
              setEditing(true);
            }}
            title="Editar"
            aria-label="Editar"
          >
            <PencilIcon />
          </button>
        )}
        <button className="icon-btn danger" onClick={onDelete} title="Eliminar" aria-label="Eliminar">
          <TrashIcon />
        </button>
      </div>
    </div>
  );
}
