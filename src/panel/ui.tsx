// Small native-looking building blocks shared by the sections.
import { Component, useEffect, useLayoutEffect, useRef, type ErrorInfo, type ReactNode, type TextareaHTMLAttributes } from "react";
import { ChevronDownIcon } from "./icons";

export function Switch({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      className={checked ? "switch on" : "switch"}
      onClick={() => onChange(!checked)}
    >
      <span className="switch-knob" />
    </button>
  );
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (v: T) => void;
  label: string;
}) {
  return (
    <div className="segmented" role="radiogroup" aria-label={label}>
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          role="radio"
          aria-checked={value === o.value}
          className={value === o.value ? "seg active" : "seg"}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function Select({
  value,
  onChange,
  children,
  label,
}: {
  value: string;
  onChange: (v: string) => void;
  children: ReactNode;
  label: string;
}) {
  return (
    <span className="select">
      <select value={value} onChange={(e) => onChange(e.target.value)} aria-label={label}>
        {children}
      </select>
      <ChevronDownIcon />
    </span>
  );
}

export function Keycaps({ caps, pulsing }: { caps: string[]; pulsing?: boolean }) {
  return (
    <span className="keycaps">
      {caps.map((c, i) => (
        <kbd key={i} className={pulsing ? "keycap pulsing" : "keycap"}>
          {c}
        </kbd>
      ))}
    </span>
  );
}

export function PageHeader({ title, subtitle, children }: { title: string; subtitle?: ReactNode; children?: ReactNode }) {
  return (
    <header className="page-header">
      <div className="page-header-text">
        <h1 className="page-title">{title}</h1>
        {subtitle && <p className="page-subtitle">{subtitle}</p>}
      </div>
      {children && <div className="page-header-actions">{children}</div>}
    </header>
  );
}

/** Textarea that grows with its content. */
export function AutoTextarea(props: TextareaHTMLAttributes<HTMLTextAreaElement> & { autoFocusEnd?: boolean }) {
  const { autoFocusEnd, ...rest } = props;
  const ref = useRef<HTMLTextAreaElement>(null);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${el.scrollHeight + el.offsetHeight - el.clientHeight}px`;
  }, [props.value]);
  useEffect(() => {
    const el = ref.current;
    if (autoFocusEnd && el) {
      el.focus();
      el.setSelectionRange(el.value.length, el.value.length);
    }
  }, [autoFocusEnd]);
  return <textarea ref={ref} rows={1} {...rest} />;
}

/** Modal confirmation sheet. */
export function ConfirmDialog({
  title,
  message,
  confirmLabel,
  destructive,
  onConfirm,
  onCancel,
}: {
  title: string;
  message: string;
  confirmLabel: string;
  destructive?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const confirmBtn = useRef<HTMLButtonElement>(null);
  const cancelBtn = useRef<HTMLButtonElement>(null);
  const cancelRef = useRef(onCancel);
  cancelRef.current = onCancel;
  const destructiveRef = useRef(destructive);
  destructiveRef.current = destructive;
  useEffect(() => {
    // Destructive actions default to the safe button.
    (destructiveRef.current ? cancelBtn : confirmBtn).current?.focus();
    const prev = document.activeElement as HTMLElement | null;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") cancelRef.current();
      else if (e.key === "Tab") {
        // Two-button trap: keep focus inside the dialog.
        const a = cancelBtn.current;
        const b = confirmBtn.current;
        if (!a || !b) return;
        const first = e.shiftKey ? a : b;
        const last = e.shiftKey ? b : a;
        if (document.activeElement === first || !(document.activeElement === a || document.activeElement === b)) {
          e.preventDefault();
          last.focus();
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      prev?.focus?.();
    };
  }, []);
  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onCancel()}>
      <div className="dialog" role="alertdialog" aria-modal="true" aria-labelledby="dlg-title">
        <h2 id="dlg-title" className="dialog-title">
          {title}
        </h2>
        <p className="dialog-message">{message}</p>
        <div className="dialog-actions">
          <button ref={cancelBtn} className="btn" onClick={onCancel}>
            Cancelar
          </button>
          <button ref={confirmBtn} className={destructive ? "btn btn-danger" : "btn btn-primary"} onClick={onConfirm}>
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}

export function Toast({
  message,
  onDone,
  action,
  duration = 2600,
}: {
  message: string;
  onDone: () => void;
  action?: { label: string; onClick: () => void };
  duration?: number;
}) {
  const doneRef = useRef(onDone);
  doneRef.current = onDone;
  const durationRef = useRef(duration);
  durationRef.current = duration;
  useEffect(() => {
    const t = window.setTimeout(() => doneRef.current(), durationRef.current);
    return () => window.clearTimeout(t);
  }, [message]);
  return (
    <div className={action ? "toast toast-has-action" : "toast"} role="status">
      {message}
      {action && (
        <button type="button" className="toast-action" onClick={action.onClick}>
          {action.label}
        </button>
      )}
    </div>
  );
}

export class ErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state: { error: Error | null } = { error: null };
  static getDerivedStateFromError(error: Error) {
    return { error };
  }
  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error(error, info.componentStack);
  }
  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div className="empty small-empty" role="alert">
        <p className="empty-title">Algo salió mal en esta sección</p>
        <button className="btn" onClick={() => this.setState({ error: null })}>
          Reintentar
        </button>
      </div>
    );
  }
}
