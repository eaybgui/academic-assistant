import type { ComponentType } from 'react';
import { EMAIL_FEATURES_ENABLED } from '../config/features';
import {
  AccountsIcon,
  CalendarIcon,
  DashboardIcon,
  ResultsIcon,
  RunsIcon,
  SettingsIcon,
  UniversityIcon,
} from './navIcons';

export const ANNOUNCEMENTS_PATH = '/announcements';

export interface NavItemConfig {
  to: string;
  label: string;
  end: boolean;
  Icon: ComponentType;
}

const ALL_NAV_ITEMS: readonly NavItemConfig[] = [
  { to: '/', label: 'Dashboard', end: true, Icon: DashboardIcon },
  { to: '/calendar', label: 'Calendar', end: false, Icon: CalendarIcon },
  { to: ANNOUNCEMENTS_PATH, label: 'Announcements', end: false, Icon: ResultsIcon },
  { to: '/universities', label: 'Universities', end: false, Icon: UniversityIcon },
  { to: '/accounts', label: 'Email accounts', end: false, Icon: AccountsIcon },
  { to: '/runs', label: 'Email runs', end: false, Icon: RunsIcon },
  { to: '/settings', label: 'Settings', end: false, Icon: SettingsIcon },
];

const EMAIL_ONLY_PATHS: readonly string[] = [ANNOUNCEMENTS_PATH, '/accounts', '/runs', '/settings'];

export function visibleNavItems(): readonly NavItemConfig[] {
  return EMAIL_FEATURES_ENABLED ? ALL_NAV_ITEMS : ALL_NAV_ITEMS.filter((item) => !EMAIL_ONLY_PATHS.includes(item.to));
}
