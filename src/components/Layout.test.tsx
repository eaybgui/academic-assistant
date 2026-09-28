import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router';
import Layout from './Layout';

vi.mock('../lib/api/client', () => ({
  api: {
    getDashboard: vi.fn().mockRejectedValue(new Error('not available outside Tauri')),
  },
}));

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

describe('Layout', () => {
  it('renders the six navigation links', () => {
    renderLayout();

    for (const label of ['Dashboard', 'Results', 'Runs', 'Accounts', 'Catalog', 'Settings']) {
      expect(screen.getByRole('link', { name: label })).toBeInTheDocument();
    }
  });

  it('renders the content passed as children', () => {
    renderLayout();

    expect(screen.getByText('Page content')).toBeInTheDocument();
  });

  it('renders without the unseen badge when the dashboard query fails', async () => {
    renderLayout();

    const resultsLink = await screen.findByRole('link', { name: 'Results' });
    expect(resultsLink).toHaveTextContent('Results');
    expect(screen.queryByText(/^\d+$/)).not.toBeInTheDocument();
  });
});
