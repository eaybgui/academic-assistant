import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router';
import CalendarPage from './CalendarPage';
import { formatLongDay, formatMonthYear, toLocalIsoDay, todayLocalIso } from '../../components/results/format';
import type { PersonalTaskDto, TrackerDto } from '../../lib/api/types';

const { getTracker, createPersonalTask } = vi.hoisted(() => ({
  getTracker: vi.fn(),
  createPersonalTask: vi.fn(),
}));

vi.mock('../../lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/api/client')>();
  return {
    ...actual,
    api: {
      getTracker,
      createPersonalTask,
      updatePersonalTask: vi.fn(),
      setPersonalTaskDone: vi.fn(),
      deletePersonalTask: vi.fn(),
    },
  };
});

const NOW = new Date();
const TODAY = todayLocalIso();
/** A day of the current month other than today (every month has at least 28 days). */
const OTHER_DAY = toLocalIsoDay(new Date(NOW.getFullYear(), NOW.getMonth(), NOW.getDate() === 20 ? 21 : 20));

function personalTask(overrides: Partial<PersonalTaskDto> = {}): PersonalTaskDto {
  return { id: 1, title: 'Go to the doctor', date: TODAY, note: null, done: false, ...overrides };
}

function tracker(personalTasks: PersonalTaskDto[]): TrackerDto {
  return { items: [], universities: [], personalTasks, syncRunning: false };
}

function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <CalendarPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

function dayButton(isoDay: string) {
  return screen.findByRole('button', { name: formatLongDay(isoDay) });
}

beforeEach(() => {
  vi.resetAllMocks();
  getTracker.mockResolvedValue(tracker([]));
});

describe('CalendarPage', () => {
  it('starts on the current month with today selected', async () => {
    renderPage();

    const today = await dayButton(TODAY);
    expect(today).toHaveAttribute('aria-current', 'date');
    expect(today).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByText(formatMonthYear(NOW.getFullYear(), NOW.getMonth()))).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: formatLongDay(TODAY) })).toBeInTheDocument();
    expect(screen.getByText('Nothing on this day.')).toBeInTheDocument();
  });

  it('fills the window width and sizes day cells with the viewport height', async () => {
    const { container } = renderPage();

    const today = await dayButton(TODAY);
    expect(container.firstElementChild).not.toHaveClass('max-w-3xl');
    expect(today).not.toHaveClass('h-16');
    expect(today).toHaveClass('h-[clamp(3.5rem,11vh,8rem)]');
  });

  it('shows 6 dots and "+2" for a crowded day and lists every entry', async () => {
    const tasks = Array.from({ length: 8 }, (_, index) =>
      personalTask({ id: index + 1, title: `Task ${index + 1}`, date: OTHER_DAY }),
    );
    getTracker.mockResolvedValue(tracker(tasks));
    const user = userEvent.setup();
    renderPage();

    const day = await dayButton(OTHER_DAY);
    expect(day.querySelectorAll('span[title]')).toHaveLength(6);
    expect(within(day).getByText('+2')).toBeInTheDocument();

    await user.click(day);

    expect(screen.getByRole('heading', { name: formatLongDay(OTHER_DAY) })).toBeInTheDocument();
    expect(screen.getAllByRole('checkbox', { name: /^Mark Task \d as done$/ })).toHaveLength(8);
  });

  it('gives each dot the title of its entry', async () => {
    getTracker.mockResolvedValue(tracker([personalTask({ title: 'Ask for a reference' })]));
    renderPage();

    const today = await dayButton(TODAY);

    const dot = within(today).getByTitle('Ask for a reference');
    expect(dot).toHaveAttribute('aria-label', 'Ask for a reference');
  });

  it('goes from December to January of the next year', async () => {
    const user = userEvent.setup();
    renderPage();
    await dayButton(TODAY);
    const nextMonth = screen.getByRole('button', { name: 'Next month' });
    const year = NOW.getFullYear();

    for (let guard = 0; guard < 12 && !screen.queryByText(formatMonthYear(year, 11)); guard += 1) {
      await user.click(nextMonth);
    }
    expect(screen.getByText(formatMonthYear(year, 11))).toBeInTheDocument();

    await user.click(nextMonth);

    expect(screen.getByText(formatMonthYear(year + 1, 0))).toBeInTheDocument();
  });

  it('returns to the current month and selects today with the Today button', async () => {
    const user = userEvent.setup();
    renderPage();
    await user.click(await screen.findByRole('button', { name: 'Next month' }));
    await user.click(screen.getByRole('button', { name: 'Today' }));

    expect(screen.getByText(formatMonthYear(NOW.getFullYear(), NOW.getMonth()))).toBeInTheDocument();
    expect(await dayButton(TODAY)).toHaveAttribute('aria-pressed', 'true');
  });

  it('adds a task for the selected day and clears the input', async () => {
    createPersonalTask.mockResolvedValue(personalTask({ id: 9 }));
    const user = userEvent.setup();
    renderPage();
    const input = await screen.findByRole('textbox', { name: 'Add a task for this day' });

    await user.type(input, 'Go to the doctor');
    await user.click(screen.getByRole('button', { name: 'Add' }));

    await waitFor(() => expect(input).toHaveValue(''));
    expect(createPersonalTask).toHaveBeenCalledWith({ title: 'Go to the doctor', date: TODAY, note: null });
  });

  it('shows a rejected add inline and keeps the text', async () => {
    createPersonalTask.mockRejectedValue({ kind: 'validation', message: 'Enter a title' });
    const user = userEvent.setup();
    renderPage();
    const input = await screen.findByRole('textbox', { name: 'Add a task for this day' });

    await user.type(input, '   ');
    await user.click(screen.getByRole('button', { name: 'Add' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('Enter a title');
    expect(input).toHaveValue('   ');
  });

  it('drops the add error and the typed text when another day is selected', async () => {
    createPersonalTask.mockRejectedValue({ kind: 'validation', message: 'Enter a title' });
    const user = userEvent.setup();
    renderPage();
    const input = await screen.findByRole('textbox', { name: 'Add a task for this day' });
    await user.type(input, '   ');
    await user.click(screen.getByRole('button', { name: 'Add' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Enter a title');

    await user.click(await dayButton(OTHER_DAY));

    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: 'Add a task for this day' })).toHaveValue('');
  });

  it('renders personal tasks with no universities instead of the first-run card', async () => {
    getTracker.mockResolvedValue(tracker([personalTask()]));
    renderPage();

    const today = await dayButton(TODAY);

    expect(within(today).getByTitle('Go to the doctor')).toBeInTheDocument();
    expect(screen.queryByText('Add your first university')).not.toBeInTheDocument();
  });

  it('opens the details of a clicked entry', async () => {
    getTracker.mockResolvedValue(tracker([personalTask()]));
    const user = userEvent.setup();
    renderPage();

    await user.click(await screen.findByRole('button', { name: 'Go to the doctor' }));

    expect(await screen.findByRole('dialog', { name: 'Edit task' })).toBeInTheDocument();
  });
});
