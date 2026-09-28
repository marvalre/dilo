import { useEffect, useRef } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";

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
