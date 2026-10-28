import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import ItemGroups, { type GroupBy } from './ItemGroups';
import type { LmsItemDto } from '../../lib/api/types';

vi.mock('../../lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/api/client')>();
  return { ...actual, api: { setItemDone: vi.fn(), dismissItemChange: vi.fn() } };
});

function item(id: number, overrides: Partial<LmsItemDto> = {}): LmsItemDto {
  return {
    id,
    universityId: 1,
    universityName: 'Universidad Demo',
    courseId: 3,
    courseName: 'Álgebra Lineal',
    courseShortName: 'ALG',
    externalId: `_${id}_1`,
    kind: 'assignment',
    title: `Hoja ${id}`,
    dueAt: null,
    url: null,
    status: 'pending',
    grade: null,
    change: null,
    removedAt: null,
    done: false,
    hidden: false,
    note: null,
    ...overrides,
  };
}

const ITEMS = [item(1), item(2, { status: 'submitted' }), item(3, { status: 'graded' })];

function tree(items: LmsItemDto[], groupBy: GroupBy) {
  return (
    <QueryClientProvider client={new QueryClient()}>
      <ItemGroups items={items} groupBy={groupBy} showUniversity={false} onOpen={vi.fn()} />
    </QueryClientProvider>
  );
}

function detailsOf(heading: string): HTMLDetailsElement {
  const details = screen.getByRole('heading', { name: heading }).closest('details');
  if (!details) throw new Error(`No <details> around "${heading}"`);
  return details;
}

describe('ItemGroups sections', () => {
  it.each([
    ['Pending 1', 'status'],
    ['Submitted 1', 'status'],
    ['Graded 1', 'status'],
    ['No date 1', 'time'],
  ] as const)('renders the "%s" group open by default', (heading, groupBy) => {
    render(tree(groupBy === 'time' ? [item(1)] : ITEMS, groupBy));
    expect(detailsOf(heading).open).toBe(true);
  });

  it('keeps the completed section closed by default', () => {
    render(tree([item(1), item(2, { status: 'submitted' })], 'time'));
    const completed = screen.getByText('Completed (1)').closest('details');
    expect(completed).not.toBeNull();
    expect(completed?.open).toBe(false);
  });

  it('collapses a group when its summary is clicked, leaving the others open', async () => {
    render(tree(ITEMS, 'status'));

    await userEvent.click(screen.getByRole('heading', { name: 'Pending 1' }));

    expect(detailsOf('Pending 1').open).toBe(false);
    expect(detailsOf('Submitted 1').open).toBe(true);
    expect(detailsOf('Graded 1').open).toBe(true);
  });

  it('keeps a collapsed group collapsed across a refetch with new items', async () => {
    const { rerender } = render(tree(ITEMS, 'status'));
    await userEvent.click(screen.getByRole('heading', { name: 'Pending 1' }));

    rerender(tree([...ITEMS.map((i) => ({ ...i })), item(4)], 'status'));

    expect(detailsOf('Pending 2').open).toBe(false);
    expect(detailsOf('Submitted 1').open).toBe(true);
  });
});
