import type { ReactNode } from 'react';

/** Clickable section header with a chevron that turns when the enclosing `details.group` is open. */
export default function CollapsibleSummary({ children }: { children: ReactNode }) {
  return (
    <summary className="mb-2 flex cursor-pointer list-none items-center gap-2 [&::-webkit-details-marker]:hidden">
      <svg
        aria-hidden="true"
        viewBox="0 0 12 12"
        className="h-3 w-3 shrink-0 text-neutral-500 transition-transform group-open:rotate-90"
      >
        <path d="M4 2l4 4-4 4" fill="none" stroke="currentColor" strokeWidth="1.5" />
      </svg>
      {children}
    </summary>
  );
}
