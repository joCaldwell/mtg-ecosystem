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

export interface ToastAction {
  label: string;
  kbd?: string;
  onAction: () => void;
}

export interface ToastOptions {
  action?: ToastAction;
  /** A second action after the first, such as showing what was just changed. */
  secondaryAction?: ToastAction;
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

const ToastContext = /* @__PURE__ */ createContext<ToastApi | null>(null);

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

  const actions = [current?.opts.action, current?.opts.secondaryAction].filter((a): a is ToastAction => !!a);
  return (
    <ToastContext.Provider value={api}>
      {children}
      {createPortal(
        <div className={styles.region} role="status" aria-live="polite">
          {current ? (
            <div key={current.id} className={styles.toast}>
              <span className={styles.message}>{current.message}</span>
              {actions.length > 0 ? (
                <span className={styles.actions}>
                  {actions.map((action) => (
                    <Button
                      key={action.label}
                      size="sm"
                      kbd={action.kbd}
                      onClick={() => {
                        action.onAction();
                        dismiss();
                      }}
                    >
                      {action.label}
                    </Button>
                  ))}
                </span>
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
