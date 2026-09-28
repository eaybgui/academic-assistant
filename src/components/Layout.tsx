import type { ReactNode } from 'react';
import { NavLink } from 'react-router';
import { useQuery } from '@tanstack/react-query';
import { api } from '../lib/api/client';
import { queryKeys } from '../lib/api/queryKeys';

const NAV_ITEMS = [
  { to: '/', label: 'Dashboard', end: true },
  { to: '/results', label: 'Results', end: false },
  { to: '/runs', label: 'Runs', end: false },
  { to: '/accounts', label: 'Accounts', end: false },
  { to: '/catalog', label: 'Catalog', end: false },
  { to: '/settings', label: 'Settings', end: false },
] as const;

export interface LayoutProps {
  children: ReactNode;
}

/**
 * App shell: sidebar navigation plus a content outlet. The Results link shows
 * an unseen-results badge fed by `getDashboard('upcoming')`. When that query
 * fails (e.g. rendered outside Tauri, in plain-browser dev or tests without a
 * mocked backend) the layout still renders normally, just without the badge.
 */
export default function Layout({ children }: LayoutProps) {
  const { data } = useQuery({
    queryKey: queryKeys.dashboard('upcoming'),
    queryFn: () => api.getDashboard('upcoming'),
    retry: false,
  });

  const unseenCount = data?.counts.newTotal;

  return (
    <div className="flex min-h-screen bg-neutral-50">
      <nav className="w-56 shrink-0 border-r border-neutral-200 bg-white px-3 py-4">
        <p className="mb-4 px-2 text-sm font-semibold text-neutral-900">Academic Assistant</p>
        <ul className="space-y-1">
          {NAV_ITEMS.map((item) => (
            <li key={item.to}>
              <NavLink
                to={item.to}
                end={item.end}
                className={({ isActive }) =>
                  `flex items-center justify-between rounded px-2 py-1.5 text-sm ${
                    isActive ? 'bg-neutral-900 text-white' : 'text-neutral-700 hover:bg-neutral-100'
                  }`
                }
              >
                <span>{item.label}</span>
                {item.to === '/results' && unseenCount ? (
                  <span className="rounded-full bg-red-600 px-1.5 text-xs font-medium text-white">
                    {unseenCount}
                  </span>
                ) : null}
              </NavLink>
            </li>
          ))}
        </ul>
      </nav>
      <main className="flex-1 px-8 py-6">{children}</main>
    </div>
  );
}
