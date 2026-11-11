import { NavLink } from 'react-router';
import { visibleNavItems } from './navItems';

/** Phone navigation: one equal-width tab per visible section, pinned to the bottom edge above the system bar. */
export default function BottomTabBar() {
  return (
    <nav
      aria-label="Sections"
      className="fixed inset-x-0 bottom-0 z-10 flex border-t border-neutral-200 bg-white pb-[env(safe-area-inset-bottom)] md:hidden"
    >
      {visibleNavItems().map(({ to, label, end, Icon }) => (
        <NavLink
          key={to}
          to={to}
          end={end}
          className={({ isActive }) =>
            `flex h-14 min-w-0 flex-1 flex-col items-center justify-center gap-0.5 text-xs ${
              isActive ? 'font-medium text-neutral-900' : 'text-neutral-500'
            }`
          }
        >
          <Icon />
          <span className="truncate">{label}</span>
        </NavLink>
      ))}
    </nav>
  );
}
