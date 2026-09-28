import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import ResultCard from './ResultCard';
import type { ResultDto } from '../../lib/api/types';

vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

function resultFixture(overrides: Partial<ResultDto> = {}): ResultDto {
  return {
    id: 1,
    accountId: 1,
    accountName: 'UV',
    subject: 'Exam date change',
    senderName: 'Office',
    senderEmail: 'office@uv.es',
    universityName: 'UV',
    courseName: 'Algebra',
    category: 'exam',
    priority: 'urgent',
    actionRequired: true,
    possibleChange: true,
    dates: [{ date: '2026-10-27', time: '10:00', kind: 'exam' }],
    receivedAt: '2026-09-27T08:00:00Z',
    detectedAt: '2026-09-27T08:05:00Z',
    sourceUrl: 'https://mail.example/msg/1',
    messageIdHeader: '<abc@uv.es>',
    seen: false,
    seenAt: null,
    ...overrides,
  };
}

describe('ResultCard', () => {
  it('renders subject, priority, meta line, category, dates and flags', () => {
    render(<ResultCard result={resultFixture()} onSetSeen={vi.fn()} />);

    expect(screen.getByText('Exam date change')).toBeInTheDocument();
    expect(screen.getByText('Urgent')).toBeInTheDocument();
    expect(screen.getByText(/Office/)).toBeInTheDocument();
    expect(screen.getByText(/Algebra \(UV\)/)).toBeInTheDocument();
    expect(screen.getByText('Exam')).toBeInTheDocument();
    expect(screen.getByText('Exam: 27 Oct 10:00')).toBeInTheDocument();
    expect(screen.getByText('Action required')).toBeInTheDocument();
    expect(screen.getByText('Possible change')).toBeInTheDocument();
  });

  it('calls onSetSeen(id, true) when Seen is clicked', async () => {
    const onSetSeen = vi.fn();
    const user = userEvent.setup();
    render(<ResultCard result={resultFixture()} onSetSeen={onSetSeen} />);

    await user.click(screen.getByRole('button', { name: 'Seen' }));

    expect(onSetSeen).toHaveBeenCalledWith(1, true);
  });

  it('shows Open email when sourceUrl is present', () => {
    render(<ResultCard result={resultFixture()} onSetSeen={vi.fn()} />);
    expect(screen.getByRole('button', { name: 'Open email' })).toBeInTheDocument();
  });

  it('shows Copy Message-ID when there is no sourceUrl', () => {
    render(<ResultCard result={resultFixture({ sourceUrl: null })} onSetSeen={vi.fn()} />);
    expect(screen.getByRole('button', { name: 'Copy Message-ID' })).toBeInTheDocument();
  });
});
