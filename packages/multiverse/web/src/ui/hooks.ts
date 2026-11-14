import { useCallback, useEffect, useRef, useState } from "react";

function read<T>(key: string, decode: (raw: string | null) => T): T {
  let raw: string | null = null;
  try {
    raw = localStorage.getItem(key);
  } catch {
    // Storage can be blocked; fall back to whatever decode makes of "nothing".
  }
  return decode(raw);
}

/**
 * State mirrored to localStorage. `decode` receives the stored string (or null
 * when absent) and must always return a valid value. Other tabs' writes arrive too.
 */
export function useLocalStorage<T>(
  key: string,
  decode: (raw: string | null) => T,
  encode: (value: T) => string,
): readonly [T, (v: T) => void] {
  const [value, setValue] = useState(() => read(key, decode));
  const fns = useRef({ decode, encode });
  useEffect(() => {
    fns.current = { decode, encode };
  });

  const set = useCallback(
    (v: T) => {
      setValue(v);
      try {
        localStorage.setItem(key, fns.current.encode(v));
      } catch {
        // Quota or privacy mode: the value still holds for this session.
      }
    },
    [key],
  );

  useEffect(() => {
    const onStorage = (e: StorageEvent) => {
      if (e.key === key) setValue(fns.current.decode(e.newValue));
    };
    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, [key]);

  return [value, set] as const;
}
