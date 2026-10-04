import { useState, type ComponentType, type ReactNode } from 'react';
import { NavLink } from 'react-router';
import { useQuery } from '@tanstack/react-query';
import { api } from '../lib/api/client';
import { queryKeys } from '../lib/api/queryKeys';
import {
  AccountsIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  DashboardIcon,
  ResultsIcon,
  RunsIcon,
  SettingsIcon,
  UniversityIcon,
} from './navIcons';

interface NavItemConfig {
  to: string;
  label: string;
  end: boolean;
  Icon: ComponentType;
}

const NAV_ITEMS: readonly NavItemConfig[] = [
  { to: '/', label: 'Dashboard', end: true, Icon: DashboardIcon },
  { to: '/results', label: 'Results', end: false, Icon: ResultsIcon },
  { to: '/runs', label: 'Runs', end: false, Icon: RunsIcon },
  { to: '/accounts', label: 'Accounts', end: false, Icon: AccountsIcon },
  { to: '/universities', label: 'Universities', end: false, Icon: UniversityIcon },
  { to: '/settings', label: 'Settings', end: false, Icon: SettingsIcon },
];

const RESULTS_PATH = '/results';
const COLLAPSED_STORAGE_KEY = 'academic-assistant.sidebar-collapsed';

function readCollapsedPreference(): boolean {
  try {
    return localStorage.getItem(COLLAPSED_STORAGE_KEY) === 'true';
  } catch {
    return false;
  }
}

function writeCollapsedPreference(collapsed: boolean) {
  try {
    localStorage.setItem(COLLAPSED_STORAGE_KEY, String(collapsed));
  } catch {
    // Storage unavailable: the preference just won't persist.
  }
}

interface SidebarNavItemProps {
  item: NavItemConfig;
  collapsed: boolean;
  unseenCount?: number;
}

function SidebarNavItem({ item, collapsed, unseenCount }: SidebarNavItemProps) {
  const { to, label, end, Icon } = item;
  const showBadge = to === RESULTS_PATH && Boolean(unseenCount);

  return (
    <NavLink
      to={to}
      end={end}
      title={collapsed ? label : undefined}
      className={({ isActive }) =>
        `flex items-center gap-3 rounded px-2 py-1.5 text-sm ${
          isActive ? 'bg-neutral-900 text-white' : 'text-neutral-700 hover:bg-neutral-100'
        }`
      }
    >
      <span className="relative flex">
        <Icon />
        {showBadge && collapsed ? (
          <span
            aria-hidden="true"
            className="absolute -right-1 -top-1 h-2.5 w-2.5 rounded-full bg-red-600"
          />
        ) : null}
      </span>
      <span className={collapsed ? 'sr-only' : 'flex-1'}>{label}</span>
      {showBadge && !collapsed ? (
        <span className="rounded-full bg-red-600 px-1.5 text-xs font-medium text-white">{unseenCount}</span>
      ) : null}
      {showBadge && collapsed ? <span className="sr-only">{unseenCount} unseen</span> : null}
    </NavLink>
  );
}

export interface LayoutProps {
  children: ReactNode;
}

/**
 * App shell: collapsible sidebar navigation plus a content outlet. The Results
 * link shows an unseen-results badge fed by `getDashboard('upcoming')`. When
 * that query fails (e.g. rendered outside Tauri, in plain-browser dev or tests
 * without a mocked backend) the layout still renders normally, just without
 * the badge. The collapsed preference is kept in `localStorage`.
 */
export default function Layout({ children }: LayoutProps) {
  const [collapsed, setCollapsed] = useState(readCollapsedPreference);
  const { data } = useQuery({
    queryKey: queryKeys.dashboard('upcoming'),
    queryFn: () => api.getDashboard('upcoming'),
    retry: false,
  });

  const unseenCount = data?.counts.newTotal;

  function toggleCollapsed() {
    const next = !collapsed;
    setCollapsed(next);
    writeCollapsedPreference(next);
  }

  return (
    <div className="flex min-h-screen bg-neutral-50">
      <nav
        className={`shrink-0 border-r border-neutral-200 bg-white px-3 py-4 transition-[width] ${
          collapsed ? 'w-16' : 'w-56'
        }`}
      >
        <div className={`mb-4 flex items-center ${collapsed ? 'justify-center' : 'justify-between gap-2'}`}>
          {collapsed ? null : <p className="px-2 text-sm font-semibold text-neutral-900">Academic Assistant</p>}
          <button
            type="button"
            aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
            aria-expanded={!collapsed}
            onClick={toggleCollapsed}
            className="rounded p-1.5 text-neutral-500 hover:bg-neutral-100 hover:text-neutral-900"
          >
            {collapsed ? <ChevronRightIcon /> : <ChevronLeftIcon />}
          </button>
        </div>
        <ul className="space-y-1">
          {NAV_ITEMS.map((item) => (
            <li key={item.to}>
              <SidebarNavItem item={item} collapsed={collapsed} unseenCount={unseenCount} />
            </li>
          ))}
        </ul>
      </nav>
      <main className="flex-1 px-8 py-6">{children}</main>
    </div>
  );
}
