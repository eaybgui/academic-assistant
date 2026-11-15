import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import BottomTabBar from './BottomTabBar';

vi.mock('../config/features', () => ({ EMAIL_FEATURES_ENABLED: false }));

function renderBar(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <BottomTabBar />
    </MemoryRouter>,
  );
}

describe('BottomTabBar', () => {
  it('lists the four sections in a labelled navigation', () => {
    renderBar('/');

    const nav = screen.getByRole('navigation', { name: 'Sections' });
    expect(Array.from(nav.querySelectorAll('a')).map((link) => link.textContent)).toEqual([
      'Dashboard',
      'Calendar',
      'Marks',
      'Universities',
    ]);
  });

  it('marks only the current route as the current page', () => {
    renderBar('/calendar');

    expect(screen.getByRole('link', { name: 'Calendar' })).toHaveAttribute('aria-current', 'page');
    expect(screen.getByRole('link', { name: 'Dashboard' })).not.toHaveAttribute('aria-current');
    expect(screen.getByRole('link', { name: 'Universities' })).not.toHaveAttribute('aria-current');
  });

  it('has no email section', () => {
    renderBar('/');

    for (const label of ['Announcements', 'Email accounts', 'Email runs', 'Settings']) {
      expect(screen.queryByRole('link', { name: label })).not.toBeInTheDocument();
    }
  });
});
