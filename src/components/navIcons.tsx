import type { ReactNode } from 'react';

/** Outline icons for the sidebar: 24x24 viewBox, rendered at 20px, colored by the surrounding text. */

function NavSvg({ children }: { children: ReactNode }) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.75}
      strokeLinecap="round"
      strokeLinejoin="round"
      className="h-5 w-5 shrink-0"
      aria-hidden="true"
    >
      {children}
    </svg>
  );
}

export function DashboardIcon() {
  return (
    <NavSvg>
      <path d="M3 10.5 12 3l9 7.5" />
      <path d="M5 9.5V20a1 1 0 0 0 1 1h4v-6h4v6h4a1 1 0 0 0 1-1V9.5" />
    </NavSvg>
  );
}

export function ResultsIcon() {
  return (
    <NavSvg>
      <path d="M3 13h5l1.5 3h5L16 13h5" />
      <path d="M5.5 5h13l2.5 8v5a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1v-5l2.5-8Z" />
    </NavSvg>
  );
}

export function RunsIcon() {
  return (
    <NavSvg>
      <circle cx="12" cy="12" r="9" />
      <path d="M12 7v5l3 2" />
    </NavSvg>
  );
}

export function AccountsIcon() {
  return (
    <NavSvg>
      <rect x="3" y="5" width="18" height="14" rx="2" />
      <path d="m3.5 6.5 8.5 6.5 8.5-6.5" />
    </NavSvg>
  );
}

export function UniversityIcon() {
  return (
    <NavSvg>
      <path d="M2 9.5 12 4l10 5.5-10 5.5L2 9.5Z" />
      <path d="M6 12v4.5c0 1.4 2.7 2.5 6 2.5s6-1.1 6-2.5V12" />
      <path d="M22 9.5V15" />
    </NavSvg>
  );
}

export function SettingsIcon() {
  return (
    <NavSvg>
      <circle cx="12" cy="12" r="3" />
      <path d="M12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1" />
    </NavSvg>
  );
}

export function ChevronLeftIcon() {
  return (
    <NavSvg>
      <path d="m15 6-6 6 6 6" />
    </NavSvg>
  );
}

export function ChevronRightIcon() {
  return (
    <NavSvg>
      <path d="m9 6 6 6-6 6" />
    </NavSvg>
  );
}
