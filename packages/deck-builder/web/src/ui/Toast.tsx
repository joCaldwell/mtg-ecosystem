import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { Button } from "./Button.tsx";
import styles from "./Toast.module.css";

export interface ToastOptions {
  action?: { label: string; kbd?: string; onAction: () => void };
  /** Milliseconds before it goes away. Default 6000. */
  duration?: number;
}

interface ToastApi {
  toast: (message: ReactNode, opts?: ToastOptions) => void;
  dismiss: () => void;
}

interface Current {
  id: number;
  message: ReactNode;
  opts: ToastOptions;
}

const ToastContext = createContext<ToastApi | null>(null);

let nextId = 1;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [current, setCurrent] = useState<Current | null>(null);

  const toast = useCallback((message: ReactNode, opts: ToastOptions = {}) => {
    setCurrent({ id: nextId++, message, opts });
  }, []);
  const dismiss = useCallback(() => setCurrent(null), []);
  const api = useMemo(() => ({ toast, dismiss }), [toast, dismiss]);

  // One timer per toast; a replacement toast has a new id, so it restarts.
  const id = current?.id;
  const duration = current?.opts.duration ?? 6000;
  useEffect(() => {
    if (id === undefined) return;
    const t = setTimeout(() => setCurrent((c) => (c?.id === id ? null : c)), duration);
    return () => clearTimeout(t);
  }, [id, duration]);

  const action = current?.opts.action;
  return (
    <ToastContext.Provider value={api}>
      {children}
      {createPortal(
        <div className={styles.region} role="status" aria-live="polite">
          {current ? (
            <div key={current.id} className={styles.toast}>
              <span>{current.message}</span>
              {action ? (
                <Button
                  size="sm"
                  kbd={action.kbd}
                  onClick={() => {
                    action.onAction();
                    dismiss();
                  }}
                >
                  {action.label}
                </Button>
              ) : null}
            </div>
          ) : null}
        </div>,
        document.body,
      )}
    </ToastContext.Provider>
  );
}

export function useToast(): ToastApi {
  const api = useContext(ToastContext);
  if (!api) throw new Error("useToast needs a <ToastProvider> above it");
  return api;
}
