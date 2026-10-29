import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router';
import Layout from './Layout';
import type { DashboardDto, LmsUniversityDto } from '../lib/api/types';

const { getDashboard, getUniversities } = vi.hoisted(() => ({ getDashboard: vi.fn(), getUniversities: vi.fn() }));

vi.mock('../lib/api/client', () => ({
  api: { getDashboard, getUniversities },
}));

const STORAGE_KEY = 'academic-assistant.sidebar-collapsed';
const NAV_LABELS = ['Dashboard', 'Announcements', 'Universities', 'Email accounts', 'Email runs', 'Settings'];
const DISABLED_TITLE = 'Add a university and track courses first';

function universityFixture(tracked: number): LmsUniversityDto {
  return {
    id: 1,
    name: 'Universidad Demo',
    platform: 'moodle',
    baseUrl: 'https://lms.example',
    loginState: 'ok',
    accountLabel: null,
    lastSync: { at: null, outcome: null, error: null },
    courses: { tracked, total: 3, unreviewed: 0 },
  };
}

function renderLayout() {
  const queryClient = new QueryClient();
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <Layout>
          <p>Page content</p>
        </Layout>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  localStorage.clear();
  getDashboard.mockReset().mockRejectedValue(new Error('not available outside Tauri'));
  getUniversities.mockReset().mockRejectedValue(new Error('not available outside Tauri'));
});

describe('Layout', () => {
  it('renders the six navigation links', () => {
    renderLayout();

    for (const label of NAV_LABELS) {
      expect(screen.getByRole('link', { name: label })).toBeInTheDocument();
    }
  });

  it('renders the content passed as children', () => {
    renderLayout();

    expect(screen.getByText('Page content')).toBeInTheDocument();
  });

  it('renders without the unseen badge when the dashboard query fails', async () => {
    renderLayout();

    const announcementsLink = await screen.findByRole('link', { name: 'Announcements' });
    expect(announcementsLink).toHaveTextContent('Announcements');
    expect(screen.queryByText(/^\d+$/)).not.toBeInTheDocument();
  });

  describe('Announcements gating', () => {
    it('is a disabled non-link while the universities have no tracked course', async () => {
      getUniversities.mockResolvedValue([universityFixture(0)]);

      renderLayout();

      await waitFor(() => expect(screen.queryByRole('link', { name: 'Announcements' })).not.toBeInTheDocument());
      const disabled = screen.getByText('Announcements').closest('[aria-disabled="true"]');
      expect(disabled).not.toBeNull();
      expect(disabled).toHaveAttribute('title', DISABLED_TITLE);
      expect(screen.getByRole('link', { name: 'Universities' })).toBeInTheDocument();
    });

    it('is disabled when there are no universities at all', async () => {
      getUniversities.mockResolvedValue([]);

      renderLayout();

      await waitFor(() => expect(screen.queryByRole('link', { name: 'Announcements' })).not.toBeInTheDocument());
    });

    it('is a link once at least one course is tracked', async () => {
      getUniversities.mockResolvedValue([universityFixture(0), universityFixture(1)]);

      renderLayout();

      const link = await screen.findByRole('link', { name: 'Announcements' });
      expect(link).toHaveAttribute('href', '/announcements');
      expect(link).not.toHaveAttribute('aria-disabled');
    });

    it('keeps the disabled tooltip and icon when the sidebar is collapsed', async () => {
      getUniversities.mockResolvedValue([universityFixture(0)]);
      renderLayout();
      await waitFor(() => expect(screen.queryByRole('link', { name: 'Announcements' })).not.toBeInTheDocument());

      fireEvent.click(screen.getByRole('button', { name: 'Collapse sidebar' }));

      const disabled = screen.getByText('Announcements').closest('[aria-disabled="true"]');
      expect(disabled).toHaveAttribute('title', DISABLED_TITLE);
      expect(disabled?.querySelector('svg[aria-hidden="true"]')).not.toBeNull();
    });
  });

  describe('collapsible sidebar', () => {
    it('shows an icon and a visible label for every link, plus the title, when expanded', () => {
      renderLayout();

      expect(screen.getByText('Academic Assistant')).toBeInTheDocument();
      for (const label of NAV_LABELS) {
        const link = screen.getByRole('link', { name: label });
        expect(link.querySelector('svg[aria-hidden="true"]')).not.toBeNull();
        expect(within(link).getByText(label)).not.toHaveClass('sr-only');
        expect(link).not.toHaveAttribute('title');
      }
      const toggle = screen.getByRole('button', { name: 'Collapse sidebar' });
      expect(toggle).toHaveAttribute('aria-expanded', 'true');
    });

    it('collapses to icons only while keeping accessible names and tooltips', () => {
      renderLayout();

      fireEvent.click(screen.getByRole('button', { name: 'Collapse sidebar' }));

      const toggle = screen.getByRole('button', { name: 'Expand sidebar' });
      expect(toggle).toHaveAttribute('aria-expanded', 'false');
      expect(screen.queryByText('Academic Assistant')).not.toBeInTheDocument();
      for (const label of NAV_LABELS) {
        const link = screen.getByRole('link', { name: label });
        expect(link.querySelector('svg[aria-hidden="true"]')).not.toBeNull();
        expect(within(link).getByText(label)).toHaveClass('sr-only');
        expect(link).toHaveAttribute('title', label);
      }
    });

    it('expands again when the toggle is pressed a second time', () => {
      renderLayout();

      fireEvent.click(screen.getByRole('button', { name: 'Collapse sidebar' }));
      fireEvent.click(screen.getByRole('button', { name: 'Expand sidebar' }));

      expect(screen.getByRole('button', { name: 'Collapse sidebar' })).toHaveAttribute('aria-expanded', 'true');
      expect(screen.getByText('Academic Assistant')).toBeInTheDocument();
    });

    it('persists the preference to localStorage', () => {
      renderLayout();

      fireEvent.click(screen.getByRole('button', { name: 'Collapse sidebar' }));
      expect(localStorage.getItem(STORAGE_KEY)).toBe('true');

      fireEvent.click(screen.getByRole('button', { name: 'Expand sidebar' }));
      expect(localStorage.getItem(STORAGE_KEY)).toBe('false');
    });

    it('restores a collapsed preference from localStorage', () => {
      localStorage.setItem(STORAGE_KEY, 'true');

      renderLayout();

      expect(screen.getByRole('button', { name: 'Expand sidebar' })).toHaveAttribute('aria-expanded', 'false');
      expect(within(screen.getByRole('link', { name: 'Dashboard' })).getByText('Dashboard')).toHaveClass('sr-only');
    });

    it('shows the count pill when expanded and still reports the count when collapsed', async () => {
      getDashboard.mockResolvedValue({ counts: { newTotal: 3 } } as DashboardDto);
      getUniversities.mockResolvedValue([universityFixture(1)]);

      renderLayout();

      expect(await screen.findByRole('link', { name: /^Announcements\s*3$/ })).toBeInTheDocument();
      expect(screen.getByText('3')).toBeInTheDocument();

      fireEvent.click(screen.getByRole('button', { name: 'Collapse sidebar' }));

      const announcementsLink = screen.getByRole('link', { name: /^Announcements\s*3 unseen$/ });
      expect(within(announcementsLink).getByText('3 unseen')).toHaveClass('sr-only');
      expect(screen.queryByText('3')).not.toBeInTheDocument();
    });
  });
});
