import { describe, expect, it } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import { useOpenEntry } from './useOpenEntry';
import { personalEntry, type TrackerEntry } from './entries';

function entry(id: number, title = `Task ${id}`): TrackerEntry {
  return personalEntry({ id, title, date: '2026-11-05', note: null, done: false });
}

interface Props {
  entries: TrackerEntry[] | undefined;
  isPlaceholderData: boolean;
}

function setup(entries: TrackerEntry[] | undefined, isPlaceholderData = false) {
  return renderHook((props: Props) => useOpenEntry(props.entries, props.isPlaceholderData), {
    initialProps: { entries, isPlaceholderData },
  });
}

describe('useOpenEntry', () => {
  it('starts with nothing open', () => {
    const { result } = setup([entry(1)]);

    expect(result.current.openEntry).toBeNull();
  });

  it('returns the live entry, with updated fields, after entries change', () => {
    const { result, rerender } = setup([entry(1, 'Old title')]);

    act(() => result.current.open(entry(1, 'Old title')));
    expect(result.current.openEntry).toEqual(entry(1, 'Old title'));

    rerender({ entries: [entry(1, 'New title')], isPlaceholderData: false });
    expect(result.current.openEntry).toEqual(entry(1, 'New title'));
  });

  it('closes on demand', () => {
    const { result } = setup([entry(1)]);

    act(() => result.current.open(entry(1)));
    act(() => result.current.close());

    expect(result.current.openEntry).toBeNull();
  });

  it('forgets the entry once fresh data lacks it, and does not reopen it when it returns', () => {
    const { result, rerender } = setup([entry(1), entry(2)]);
    act(() => result.current.open(entry(1)));

    rerender({ entries: [entry(2)], isPlaceholderData: false });
    expect(result.current.openEntry).toBeNull();

    rerender({ entries: [entry(1), entry(2)], isPlaceholderData: false });
    expect(result.current.openEntry).toBeNull();
  });

  it('keeps the key while the data is a placeholder', () => {
    const { result, rerender } = setup([entry(1)]);
    act(() => result.current.open(entry(1)));

    rerender({ entries: [entry(2)], isPlaceholderData: true });
    expect(result.current.openEntry).toBeNull();

    rerender({ entries: [entry(1)], isPlaceholderData: true });
    expect(result.current.openEntry).toEqual(entry(1));
  });

  it('keeps the key while there is no data yet', () => {
    const { result, rerender } = setup([entry(1)]);
    act(() => result.current.open(entry(1)));

    rerender({ entries: undefined, isPlaceholderData: false });
    rerender({ entries: [entry(1)], isPlaceholderData: false });

    expect(result.current.openEntry).toEqual(entry(1));
  });
});
