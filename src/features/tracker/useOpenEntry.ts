import { useCallback, useState } from 'react';
import type { TrackerEntry } from './entries';

/**
 * Tracks which entry's details are open by key and returns the live entry from `entries`, so edits
 * show up while it is open. Once fresh (non-placeholder) data no longer contains the key, the key is
 * forgotten so the entry can't pop open again if it comes back.
 */
export function useOpenEntry(entries: TrackerEntry[] | undefined, isPlaceholderData: boolean) {
  const [openKey, setOpenKey] = useState<string | null>(null);
  const open = useCallback((entry: TrackerEntry) => setOpenKey(entry.key), []);
  const close = useCallback(() => setOpenKey(null), []);

  const openEntry = entries?.find((entry) => entry.key === openKey) ?? null;
  if (openKey !== null && entries !== undefined && !isPlaceholderData && openEntry === null) {
    setOpenKey(null);
  }

  return { openEntry, open, close };
}
