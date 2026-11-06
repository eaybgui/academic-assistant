import type { Category } from '../../lib/api/types';
import { CATEGORY_LABELS } from './labels';
import { CHIP_NEUTRAL } from '../styles';

export interface CategoryLabelProps {
  category: Category;
}

/** Neutral pill showing a result's category ("Notice", "Task", "Exam", ...). */
export default function CategoryLabel({ category }: CategoryLabelProps) {
  return (
    <span className={CHIP_NEUTRAL}>
      {CATEGORY_LABELS[category]}
    </span>
  );
}
