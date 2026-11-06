/** Tailwind class strings reused across files. Variants are composed from a base so the class order stays identical. */

// Primary (dark, filled) buttons.
export const PRIMARY_BUTTON_BASE =
  'rounded bg-neutral-900 px-3 py-1.5 text-sm font-medium text-white hover:bg-neutral-800';
export const PRIMARY_BUTTON = `${PRIMARY_BUTTON_BASE} disabled:opacity-50`;

// Secondary (outlined) buttons.
export const SECONDARY_BUTTON_BASE =
  'rounded border border-neutral-300 px-3 py-1.5 text-sm text-neutral-700 hover:bg-neutral-50';
export const SECONDARY_BUTTON = `${SECONDARY_BUTTON_BASE} disabled:opacity-50`;

// Text and icon buttons.
export const LINK_BUTTON = 'text-sm text-neutral-700 hover:underline';
export const ICON_DELETE_BUTTON = 'text-neutral-400 hover:text-red-600';

// Cards and list rows.
export const CARD_ROW = 'rounded-lg border border-neutral-200 bg-white px-4 py-3';
export const LIST_CARD = 'rounded border border-neutral-200 bg-white px-4 py-3';

// Form fields. Callers append their own class after a single space.
export const TEXT_FIELD =
  'rounded border border-neutral-300 px-3 py-1.5 text-sm text-neutral-900 focus:border-neutral-500 focus:outline-none';

// Chips and pills. Callers append a colour tone after a single space.
export const CHIP_NEUTRAL =
  'inline-flex items-center rounded-full bg-neutral-100 px-2 py-0.5 text-xs font-medium text-neutral-700';
export const PILL_BASE = 'inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium';

// Background and text colour pairs for pills and badges.
export const TONE_GREEN = 'bg-green-100 text-green-700';
export const TONE_RED = 'bg-red-100 text-red-700';
export const TONE_AMBER = 'bg-amber-100 text-amber-800';
export const TONE_BLUE = 'bg-blue-100 text-blue-700';
export const TONE_NEUTRAL = 'bg-neutral-100 text-neutral-700';

// Headings.
export const PAGE_TITLE = 'text-2xl font-semibold text-neutral-900';
export const SECTION_TITLE = 'text-lg font-semibold text-neutral-900';
export const SUBSECTION_TITLE = 'text-sm font-semibold text-neutral-900';
export const ITEM_TITLE_BOLD = 'font-bold text-neutral-900';
export const ITEM_TITLE = 'font-medium text-neutral-900';

// Text roles.
export const TEXT_MUTED = 'text-sm text-neutral-500';
export const TEXT_MUTED_XS = 'text-xs text-neutral-500';
export const TEXT_SECONDARY = 'text-sm text-neutral-600';
export const TEXT_BODY = 'text-sm text-neutral-700';
export const TEXT_ERROR = 'text-sm text-red-600';
