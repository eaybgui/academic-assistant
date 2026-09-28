import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { api, toCommandError } from './client';
import type { AccountInput, ResultQuery, SettingsInput } from './types';

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

  it('addUniversity(name) → add_university { name }', async () => {
    await api.addUniversity('UV');
    expect(mockedInvoke).toHaveBeenCalledWith('add_university', { name: 'UV' });
  });

  it('updateUniversity(id, name) → update_university { id, name }', async () => {
    await api.updateUniversity(1, 'UV');
    expect(mockedInvoke).toHaveBeenCalledWith('update_university', { id: 1, name: 'UV' });
  });

  it('removeUniversity(id) → remove_university { id }', async () => {
    await api.removeUniversity(1);
    expect(mockedInvoke).toHaveBeenCalledWith('remove_university', { id: 1 });
  });

  it('addCourse(universityId, name, aliases) → add_course { universityId, name, aliases }', async () => {
    await api.addCourse(1, 'Algebra', ['ALG']);
    expect(mockedInvoke).toHaveBeenCalledWith('add_course', {
      universityId: 1,
      name: 'Algebra',
      aliases: ['ALG'],
    });
  });

  it('updateCourse(id, name, aliases) → update_course { id, name, aliases }', async () => {
    await api.updateCourse(3, 'Algebra II', []);
    expect(mockedInvoke).toHaveBeenCalledWith('update_course', {
      id: 3,
      name: 'Algebra II',
      aliases: [],
    });
  });

  it('removeCourse(id) → remove_course { id }', async () => {
    await api.removeCourse(3);
    expect(mockedInvoke).toHaveBeenCalledWith('remove_course', { id: 3 });
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
