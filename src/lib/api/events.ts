import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  LmsSyncFinishedEvent,
  LmsSyncProgressEvent,
  RunFinishedEvent,
  RunProgressEvent,
} from './types';

/** Subscribes to `run:progress`. Call the returned unlisten function on cleanup. */
export function onRunProgress(cb: (event: RunProgressEvent) => void): Promise<UnlistenFn> {
  return listen<RunProgressEvent>('run:progress', (e) => cb(e.payload));
}

/** Subscribes to `run:finished`. Call the returned unlisten function on cleanup. */
export function onRunFinished(cb: (event: RunFinishedEvent) => void): Promise<UnlistenFn> {
  return listen<RunFinishedEvent>('run:finished', (e) => cb(e.payload));
}

/** Subscribes to `lms-sync:progress`. Call the returned unlisten function on cleanup. */
export function onLmsSyncProgress(cb: (event: LmsSyncProgressEvent) => void): Promise<UnlistenFn> {
  return listen<LmsSyncProgressEvent>('lms-sync:progress', (e) => cb(e.payload));
}

/** Subscribes to `lms-sync:finished`. Call the returned unlisten function on cleanup. */
export function onLmsSyncFinished(cb: (event: LmsSyncFinishedEvent) => void): Promise<UnlistenFn> {
  return listen<LmsSyncFinishedEvent>('lms-sync:finished', (e) => cb(e.payload));
}
