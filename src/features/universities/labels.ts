import type { LmsPlatform, LoginState } from '../../lib/api/types';

export const PLATFORM_LABELS: Record<LmsPlatform, string> = {
  moodle: 'Moodle',
  blackboard: 'Blackboard',
};

export const LOGIN_STATE_LABELS: Record<LoginState, string> = {
  never: 'Not signed in',
  ok: 'Connected',
  expired: 'Session expired',
  error: 'Sign-in failed',
};

/** Status line for a university: the connected account when known, otherwise the login state label. */
export function connectionStatusText(loginState: LoginState, accountLabel: string | null): string {
  if (loginState === 'ok' && accountLabel) {
    return `Connected as ${accountLabel}`;
  }
  return LOGIN_STATE_LABELS[loginState];
}
