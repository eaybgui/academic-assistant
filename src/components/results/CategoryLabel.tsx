import type { Category } from '../../lib/api/types';
import { CATEGORY_LABELS } from './labels';

export interface CategoryLabelProps {
  category: Category;
}

/** Neutral pill showing a result's category ("Notice", "Task", "Exam", ...). */
export default function CategoryLabel({ category }: CategoryLabelProps) {
  return (
    <span className="inline-flex items-center rounded-full bg-neutral-100 px-2 py-0.5 text-xs font-medium text-neutral-700">
      {CATEGORY_LABELS[category]}
    </span>
  );
}
