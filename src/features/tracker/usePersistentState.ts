import { useCallback, useState } from 'react';

type Allowed<T extends string> = readonly T[] | ((value: string) => value is T);

function readStored<T extends string>(key: string, fallback: T, allowed: Allowed<T>): T {
  try {
    const stored = localStorage.getItem(key);
    if (stored === null) return fallback;
    const valid = typeof allowed === 'function' ? allowed(stored) : allowed.includes(stored as T);
    return valid ? (stored as T) : fallback;
  } catch {
    return fallback;
  }
}

/**
 * State mirrored to `localStorage`. A stored value that is not allowed (a list of values or a
 * validator) is ignored, and storage failures only mean the preference does not persist.
 */
export function usePersistentState<T extends string>(
  key: string,
  fallback: T,
  allowed: Allowed<T>,
): [T, (value: T) => void] {
  const [value, setValue] = useState<T>(() => readStored(key, fallback, allowed));

  const update = useCallback(
    (next: T) => {
      setValue(next);
      try {
        localStorage.setItem(key, next);
      } catch {
        // Storage unavailable: the preference just won't persist.
      }
    },
    [key],
  );

  return [value, update];
}
