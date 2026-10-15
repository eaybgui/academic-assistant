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

const ANNOUNCEMENTS_PATH = '/announcements';

interface NavItemConfig {
  to: string;
  label: string;
  end: boolean;
  Icon: ComponentType;
}

const NAV_ITEMS: readonly NavItemConfig[] = [
  { to: '/', label: 'Dashboard', end: true, Icon: DashboardIcon },
  { to: ANNOUNCEMENTS_PATH, label: 'Announcements', end: false, Icon: ResultsIcon },
  { to: '/universities', label: 'Universities', end: false, Icon: UniversityIcon },
  { to: '/accounts', label: 'Email accounts', end: false, Icon: AccountsIcon },
  { to: '/runs', label: 'Email runs', end: false, Icon: RunsIcon },
  { to: '/settings', label: 'Settings', end: false, Icon: SettingsIcon },
];

const ANNOUNCEMENTS_DISABLED_TITLE = 'Add a university and track courses first';
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
  disabled?: boolean;
}

function SidebarNavItem({ item, collapsed, unseenCount, disabled = false }: SidebarNavItemProps) {
  const { to, label, end, Icon } = item;
  const showBadge = to === ANNOUNCEMENTS_PATH && Boolean(unseenCount);

  const content = (
    <>
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
    </>
  );

  if (disabled) {
    return (
      <span
        aria-disabled="true"
        title={ANNOUNCEMENTS_DISABLED_TITLE}
        className="flex cursor-not-allowed items-center gap-3 rounded px-2 py-1.5 text-sm text-neutral-400"
      >
        {content}
      </span>
    );
  }

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
      {content}
    </NavLink>
  );
}

export interface LayoutProps {
  children: ReactNode;
}

/**
 * App shell: collapsible sidebar navigation plus a content outlet. The Announcements
 * link shows an unseen-results badge fed by `getDashboard('upcoming')` and is disabled
 * until at least one course is tracked (`getUniversities`). When a query fails (e.g.
 * rendered outside Tauri, in plain-browser dev or tests without a mocked backend) the
 * layout still renders normally, just without the badge and with Announcements enabled.
 * The collapsed preference is kept in `localStorage`.
 */
export default function Layout({ children }: LayoutProps) {
  const [collapsed, setCollapsed] = useState(readCollapsedPreference);
  const { data } = useQuery({
    queryKey: queryKeys.dashboard('upcoming'),
    queryFn: () => api.getDashboard('upcoming'),
    retry: false,
  });

  const { data: universities } = useQuery({
    queryKey: queryKeys.universities,
    queryFn: api.getUniversities,
    retry: false,
  });

  const unseenCount = data?.counts.newTotal;
  const announcementsDisabled =
    universities !== undefined && universities.reduce((sum, u) => sum + u.courses.tracked, 0) === 0;

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
              <SidebarNavItem
                item={item}
                collapsed={collapsed}
                unseenCount={unseenCount}
                disabled={item.to === ANNOUNCEMENTS_PATH && announcementsDisabled}
              />
            </li>
          ))}
        </ul>
      </nav>
      <main className="flex-1 px-8 py-6">{children}</main>
    </div>
  );
}
