import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { RunFinishedEvent, RunProgressEvent } from './types';

/** Subscribes to `run:progress`. Call the returned unlisten function on cleanup. */
export function onRunProgress(cb: (event: RunProgressEvent) => void): Promise<UnlistenFn> {
  return listen<RunProgressEvent>('run:progress', (e) => cb(e.payload));
}

/** Subscribes to `run:finished`. Call the returned unlisten function on cleanup. */
export function onRunFinished(cb: (event: RunFinishedEvent) => void): Promise<UnlistenFn> {
  return listen<RunFinishedEvent>('run:finished', (e) => cb(e.payload));
}
