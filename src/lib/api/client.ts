import { invoke } from '@tauri-apps/api/core';
import type {
  AccountDto,
  AccountInput,
  CommandError,
  ConnectionTestDto,
  DashboardDto,
  LmsPlatform,
  LmsUniversityDto,
  LmsUniversityInput,
  Page,
  ProbeDto,
  ResultDto,
  ResultQuery,
  ResultSort,
  RunDto,
  SettingsDto,
  SettingsInput,
  UniversityDto,
} from './types';

/**
 * Normalizes a rejected `invoke()` call into a `CommandError`.
 *
 * Tauri command errors serialize as `{ kind, message }` (see spec §10). Anything
 * else (a network hiccup outside Tauri, a thrown string, etc.) becomes a generic
 * internal error so callers never need to guess the shape of `e`.
 */
export function toCommandError(e: unknown): CommandError {
  if (
    typeof e === 'object' &&
    e !== null &&
    'kind' in e &&
    'message' in e &&
    typeof (e as { kind: unknown }).kind === 'string' &&
    typeof (e as { message: unknown }).message === 'string'
  ) {
    return e as CommandError;
  }
  return { kind: 'internal', message: 'Something went wrong' };
}

export const api = {
  checkNow: () => invoke<number>('check_now', {}),

  getDashboard: (sort: ResultSort) => invoke<DashboardDto>('get_dashboard', { sort }),

  getResults: (query: ResultQuery) => invoke<Page<ResultDto>>('get_results', { query }),

  setResultSeen: (id: number, seen: boolean) =>
    invoke<ResultDto>('set_result_seen', { id, seen }),

  markAllSeen: () => invoke<number>('mark_all_seen', {}),

  getRunHistory: (limit: number) => invoke<RunDto[]>('get_run_history', { limit }),

  getAccounts: () => invoke<AccountDto[]>('get_accounts', {}),

  addAccount: (input: AccountInput) => invoke<AccountDto>('add_account', { input }),

  updateAccount: (id: number, input: AccountInput) =>
    invoke<AccountDto>('update_account', { id, input }),

  removeAccount: (id: number) => invoke<void>('remove_account', { id }),

  testAccount: (id: number) => invoke<ConnectionTestDto>('test_account', { id }),

  connectMicrosoftAccount: (id: number) =>
    invoke<AccountDto>('connect_microsoft_account', { id }),

  getCatalog: () => invoke<UniversityDto[]>('get_catalog', {}),

  getUniversities: () => invoke<LmsUniversityDto[]>('get_universities', {}),

  lmsProbe: (url: string, platform: LmsPlatform) =>
    invoke<ProbeDto>('lms_probe', { url, platform }),

  addUniversity: (input: LmsUniversityInput) =>
    invoke<LmsUniversityDto>('add_university', { input }),

  renameUniversity: (id: number, name: string) =>
    invoke<LmsUniversityDto>('rename_university', { id, name }),

  removeUniversity: (id: number) => invoke<void>('remove_university', { id }),

  lmsSignIn: (id: number) => invoke<LmsUniversityDto>('lms_sign_in', { id }),

  lmsSignOut: (id: number) => invoke<LmsUniversityDto>('lms_sign_out', { id }),

  lmsCheckSession: (id: number) => invoke<LmsUniversityDto>('lms_check_session', { id }),

  getSettings: () => invoke<SettingsDto>('get_settings', {}),

  updateSettings: (input: SettingsInput) => invoke<SettingsDto>('update_settings', { input }),

  setJevApiKey: (key: string) => invoke<void>('set_jev_api_key', { key }),

  clearJevApiKey: () => invoke<void>('clear_jev_api_key', {}),

  testAiProvider: () => invoke<ConnectionTestDto>('test_ai_provider', {}),
};
