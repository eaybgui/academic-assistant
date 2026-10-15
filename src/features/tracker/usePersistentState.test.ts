import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import { usePersistentState } from './usePersistentState';

const KEY = 'test.persistent-state';
const ALLOWED = ['time', 'status'] as const;

beforeEach(() => localStorage.clear());
afterEach(() => vi.restoreAllMocks());

describe('usePersistentState', () => {
  it('falls back when nothing is stored', () => {
    const { result } = renderHook(() => usePersistentState(KEY, 'time', ALLOWED));
    expect(result.current[0]).toBe('time');
  });

  it('reads an allowed stored value', () => {
    localStorage.setItem(KEY, 'status');
    const { result } = renderHook(() => usePersistentState(KEY, 'time', ALLOWED));
    expect(result.current[0]).toBe('status');
  });

  it('ignores a stored value that is not allowed', () => {
    localStorage.setItem(KEY, 'bogus');
    const { result } = renderHook(() => usePersistentState(KEY, 'time', ALLOWED));
    expect(result.current[0]).toBe('time');
  });

  it('accepts a validator function instead of a list', () => {
    localStorage.setItem(KEY, '42');
    const isNumeric = (v: string): v is string => /^\d+$/.test(v);
    const { result } = renderHook(() => usePersistentState<string>(KEY, 'all', isNumeric));
    expect(result.current[0]).toBe('42');
  });

  it('stores the value it is given', () => {
    const { result } = renderHook(() => usePersistentState(KEY, 'time', ALLOWED));
    act(() => result.current[1]('status'));
    expect(result.current[0]).toBe('status');
    expect(localStorage.getItem(KEY)).toBe('status');
  });

  it('survives localStorage throwing', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('denied');
    });
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('denied');
    });
    const { result } = renderHook(() => usePersistentState(KEY, 'time', ALLOWED));
    expect(result.current[0]).toBe('time');
    act(() => result.current[1]('status'));
    expect(result.current[0]).toBe('status');
  });
});
