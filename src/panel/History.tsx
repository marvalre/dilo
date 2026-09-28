import { useCallback, useEffect, useRef, useState } from "react";
import { api, events, type Dictation } from "./api";
import { useTauriEvent } from "./hooks";
import HistoryItem from "./HistoryItem";

const PAGE = 50;

export default function History() {
  const [input, setInput] = useState("");
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<Dictation[]>([]);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(true);
  const reqId = useRef(0);

  useEffect(() => {
    const t = setTimeout(() => setQuery(input.trim()), 200);
    return () => clearTimeout(t);
  }, [input]);

  const load = useCallback(async (q: string, offset: number) => {
    const id = ++reqId.current;
    setLoading(true);
    try {
      const page = await api.history(q, offset);
      if (id !== reqId.current) return;
      setItems((prev) => (offset === 0 ? page : [...prev, ...page]));
      setHasMore(page.length === PAGE);
    } catch {
      if (id === reqId.current && offset === 0) setItems([]);
    } finally {
      if (id === reqId.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    load(query, 0);
  }, [query, load]);

  useTauriEvent(events.onHistoryChanged, () => load(query, 0));

  const remove = async (id: number) => {
    try {
      await api.deleteDictation(id);
      setItems((prev) => prev.filter((d) => d.id !== id));
    } catch {
      /* keep the item if deletion failed */
    }
  };

  return (
    <div className="history">
      <div className="search">
        <svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true">
          <circle cx="7" cy="7" r="5" fill="none" stroke="currentColor" strokeWidth="1.6" />
          <path d="M11 11l3.5 3.5" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
        </svg>
        <input
          type="search"
          placeholder="Buscar en tus dictados"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          aria-label="Buscar"
        />
      </div>

      {!loading && items.length === 0 ? (
        <div className="empty">
          {query ? (
            <p>No hay dictados que coincidan con “{query}”.</p>
          ) : (
            <>
              <div className="empty-key">fn</div>
              <p>Mantén presionada la tecla Fn y habla. Todo lo que dictes aparecerá aquí.</p>
            </>
          )}
        </div>
      ) : (
        <ul className="list">
          {items.map((d) => (
            <HistoryItem key={d.id} item={d} onDelete={() => remove(d.id)} />
          ))}
        </ul>
      )}

      {hasMore && (
        <button className="btn btn-ghost more" disabled={loading} onClick={() => load(query, items.length)}>
          {loading ? "Cargando…" : "Cargar más"}
        </button>
      )}
    </div>
  );
}
