import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { api, toCommandError } from './client';
import type { AccountInput, LmsUniversityInput, ResultQuery, SettingsInput } from './types';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

beforeEach(() => {
  mockedInvoke.mockReset();
  mockedInvoke.mockResolvedValue(undefined);
});

describe('api client', () => {
  it('checkNow → check_now {}', async () => {
    await api.checkNow();
    expect(mockedInvoke).toHaveBeenCalledWith('check_now', {});
  });

  it('getDashboard(sort) → get_dashboard { sort }', async () => {
    await api.getDashboard('priority');
    expect(mockedInvoke).toHaveBeenCalledWith('get_dashboard', { sort: 'priority' });
  });

  it('getResults(query) → get_results { query }', async () => {
    const query: ResultQuery = { sort: 'received', limit: 20, offset: 0 };
    await api.getResults(query);
    expect(mockedInvoke).toHaveBeenCalledWith('get_results', { query });
  });

  it('setResultSeen(id, seen) → set_result_seen { id, seen }', async () => {
    await api.setResultSeen(42, true);
    expect(mockedInvoke).toHaveBeenCalledWith('set_result_seen', { id: 42, seen: true });
  });

  it('markAllSeen() → mark_all_seen {}', async () => {
    await api.markAllSeen();
    expect(mockedInvoke).toHaveBeenCalledWith('mark_all_seen', {});
  });

  it('getRunHistory(limit) → get_run_history { limit }', async () => {
    await api.getRunHistory(10);
    expect(mockedInvoke).toHaveBeenCalledWith('get_run_history', { limit: 10 });
  });

  it('getAccounts() → get_accounts {}', async () => {
    await api.getAccounts();
    expect(mockedInvoke).toHaveBeenCalledWith('get_accounts', {});
  });

  it('addAccount(input) → add_account { input }', async () => {
    const input: AccountInput = {
      name: 'UV',
      emailAddress: 'a@uv.es',
      folders: ['INBOX'],
      defaultUniversityId: null,
      initialSyncDays: 14,
      enabled: true,
      provider: { type: 'imap', host: 'imap.uv.es', port: 993, username: 'a@uv.es', password: 'x' },
    };
    await api.addAccount(input);
    expect(mockedInvoke).toHaveBeenCalledWith('add_account', { input });
  });

  it('updateAccount(id, input) → update_account { id, input }', async () => {
    const input: AccountInput = {
      name: 'UV',
      emailAddress: 'a@uv.es',
      folders: ['INBOX'],
      defaultUniversityId: null,
      initialSyncDays: 14,
      enabled: true,
      provider: { type: 'microsoft_graph', tenant: 'organizations' },
    };
    await api.updateAccount(7, input);
    expect(mockedInvoke).toHaveBeenCalledWith('update_account', { id: 7, input });
  });

  it('removeAccount(id) → remove_account { id }', async () => {
    await api.removeAccount(7);
    expect(mockedInvoke).toHaveBeenCalledWith('remove_account', { id: 7 });
  });

  it('testAccount(id) → test_account { id }', async () => {
    await api.testAccount(7);
    expect(mockedInvoke).toHaveBeenCalledWith('test_account', { id: 7 });
  });

  it('connectMicrosoftAccount(id) → connect_microsoft_account { id }', async () => {
    await api.connectMicrosoftAccount(7);
    expect(mockedInvoke).toHaveBeenCalledWith('connect_microsoft_account', { id: 7 });
  });

  it('getCatalog() → get_catalog {}', async () => {
    await api.getCatalog();
    expect(mockedInvoke).toHaveBeenCalledWith('get_catalog', {});
  });

  it('getUniversities() → get_universities {}', async () => {
    await api.getUniversities();
    expect(mockedInvoke).toHaveBeenCalledWith('get_universities', {});
  });

  it('lmsProbe(url, platform) → lms_probe { url, platform }', async () => {
    await api.lmsProbe('aulavirtual.uv.es', 'moodle');
    expect(mockedInvoke).toHaveBeenCalledWith('lms_probe', {
      url: 'aulavirtual.uv.es',
      platform: 'moodle',
    });
  });

  it('addUniversity(input) → add_university { input }', async () => {
    const input: LmsUniversityInput = { name: 'UV', platform: 'moodle', baseUrl: 'aulavirtual.uv.es' };
    await api.addUniversity(input);
    expect(mockedInvoke).toHaveBeenCalledWith('add_university', { input });
  });

  it('renameUniversity(id, name) → rename_university { id, name }', async () => {
    await api.renameUniversity(1, 'UV');
    expect(mockedInvoke).toHaveBeenCalledWith('rename_university', { id: 1, name: 'UV' });
  });

  it('removeUniversity(id) → remove_university { id }', async () => {
    await api.removeUniversity(1);
    expect(mockedInvoke).toHaveBeenCalledWith('remove_university', { id: 1 });
  });

  it('lmsSignIn(id) → lms_sign_in { id }', async () => {
    await api.lmsSignIn(1);
    expect(mockedInvoke).toHaveBeenCalledWith('lms_sign_in', { id: 1 });
  });

  it('lmsSignOut(id) → lms_sign_out { id }', async () => {
    await api.lmsSignOut(1);
    expect(mockedInvoke).toHaveBeenCalledWith('lms_sign_out', { id: 1 });
  });

  it('lmsCheckSession(id) → lms_check_session { id }', async () => {
    await api.lmsCheckSession(1);
    expect(mockedInvoke).toHaveBeenCalledWith('lms_check_session', { id: 1 });
  });

  it('getSettings() → get_settings {}', async () => {
    await api.getSettings();
    expect(mockedInvoke).toHaveBeenCalledWith('get_settings', {});
  });

  it('updateSettings(input) → update_settings { input }', async () => {
    const input: SettingsInput = {
      aiProvider: 'ollama',
      jevModel: 'jev-latest',
      ollamaBaseUrl: 'http://localhost:11434',
      ollamaModel: 'qwen3:8b',
      microsoftClientId: null,
      blockedSenders: [],
    };
    await api.updateSettings(input);
    expect(mockedInvoke).toHaveBeenCalledWith('update_settings', { input });
  });

  it('setJevApiKey(key) → set_jev_api_key { key }', async () => {
    await api.setJevApiKey('secret');
    expect(mockedInvoke).toHaveBeenCalledWith('set_jev_api_key', { key: 'secret' });
  });

  it('clearJevApiKey() → clear_jev_api_key {}', async () => {
    await api.clearJevApiKey();
    expect(mockedInvoke).toHaveBeenCalledWith('clear_jev_api_key', {});
  });

  it('testAiProvider() → test_ai_provider {}', async () => {
    await api.testAiProvider();
    expect(mockedInvoke).toHaveBeenCalledWith('test_ai_provider', {});
  });
});

describe('toCommandError', () => {
  it('passes through an object shaped like a CommandError', () => {
    const error = { kind: 'validation' as const, message: 'Bad input' };
    expect(toCommandError(error)).toEqual(error);
  });

  it('normalizes a string rejection', () => {
    expect(toCommandError('boom')).toEqual({ kind: 'internal', message: 'Something went wrong' });
  });

  it('normalizes null/undefined', () => {
    expect(toCommandError(null)).toEqual({ kind: 'internal', message: 'Something went wrong' });
    expect(toCommandError(undefined)).toEqual({ kind: 'internal', message: 'Something went wrong' });
  });

  it('normalizes a plain Error', () => {
    expect(toCommandError(new Error('network down'))).toEqual({
      kind: 'internal',
      message: 'Something went wrong',
    });
  });

  it('normalizes an object missing kind or message', () => {
    expect(toCommandError({ message: 'no kind' })).toEqual({
      kind: 'internal',
      message: 'Something went wrong',
    });
  });
});
