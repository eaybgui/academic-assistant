import { useEffect } from 'react';
import { onBackButtonPress } from '@tauri-apps/api/app';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { IS_ANDROID } from '../../config/platform';

/** Closes the topmost open dialog by sending it the Escape key, which `Modal` already handles. */
function closeTopmostDialog(): boolean {
  const dialogs = document.querySelectorAll<HTMLDialogElement>('dialog[open]');
  const topmost = dialogs[dialogs.length - 1];
  if (!topmost) return false;
  topmost.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
  return true;
}

function handleBack({ canGoBack }: { canGoBack: boolean }) {
  if (closeTopmostDialog()) return;
  if (canGoBack) {
    window.history.back();
  } else {
    void getCurrentWindow().close();
  }
}

/**
 * Android's Back button: close the open dialog, else go to the previous screen, else leave the app.
 * Registering the listener replaces the system default, so all three cases are handled here.
 * Does nothing outside the Android build.
 */
export function useAndroidBack(): void {
  useEffect(() => {
    if (!IS_ANDROID) return;
    let cancelled = false;
    let unregister: (() => Promise<void>) | undefined;
    void onBackButtonPress(handleBack).then((listener) => {
      if (cancelled) void listener.unregister();
      else unregister = () => listener.unregister();
    });
    return () => {
      cancelled = true;
      void unregister?.();
    };
  }, []);
}
