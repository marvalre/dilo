import { useEffect, useRef, useState } from "react";
import { api, type Dictation } from "./api";
import { fmtDuration, fmtNum, fmtRelative } from "./format";

interface Props {
  item: Dictation;
  onDelete: () => void;
}

export default function HistoryItem({ item, onDelete }: Props) {
  const [expanded, setExpanded] = useState(false);
  const [copied, setCopied] = useState(false);
  const timer = useRef<number>();

  useEffect(() => () => window.clearTimeout(timer.current), []);

  const copy = async () => {
    try {
      await api.copyText(item.text);
      setCopied(true);
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => setCopied(false), 1200);
    } catch {
      /* ignore */
    }
  };

  const lang = item.language && item.language !== "?" ? item.language.toUpperCase() : null;
  const words = item.word_count === 1 ? "1 palabra" : `${fmtNum(item.word_count)} palabras`;

  return (
    <li className={item.error ? "item item-error" : "item"}>
      <div
        className={expanded ? "item-text expanded" : "item-text"}
        role="button"
        tabIndex={0}
        aria-expanded={expanded}
        onClick={() => setExpanded((v) => !v)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setExpanded((v) => !v);
          }
        }}
      >
        {item.text || <span className="muted">(sin texto)</span>}
      </div>
      {item.error && <div className="item-errmsg">{item.error}</div>}
      <div className="meta">
        <span>{fmtRelative(item.created_at)}</span>
        <span className="dot">·</span>
        <span>{words}</span>
        <span className="dot">·</span>
        <span>{fmtDuration(item.duration_ms)}</span>
        {lang && <span className="badge">{lang}</span>}
        {item.app_name && <span className="app-name" title={item.app_name}>{item.app_name}</span>}
      </div>
      <div className="actions">
        <button className="icon-btn" onClick={copy} title="Copiar" aria-label="Copiar" disabled={!item.text}>
          {copied ? (
            <span className="copied">Copiado</span>
          ) : (
            <svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true">
              <rect x="5" y="5" width="8.5" height="9" rx="2" fill="none" stroke="currentColor" strokeWidth="1.4" />
              <path d="M3 10.5V4a2 2 0 0 1 2-2h5" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
            </svg>
          )}
        </button>
        <button className="icon-btn danger" onClick={onDelete} title="Eliminar" aria-label="Eliminar">
          <svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true">
            <path
              d="M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.6 8.6a1 1 0 0 0 1 .9h3.8a1 1 0 0 0 1-.9l.6-8.6"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.4"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        </button>
      </div>
    </li>
  );
}
