import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import CourseSelectionList from './CourseSelectionList';
import type { LmsCourseDto } from '../../lib/api/types';

function course(overrides: Partial<LmsCourseDto> = {}): LmsCourseDto {
  return {
    id: 1,
    universityId: 1,
    name: 'Álgebra Lineal',
    shortName: null,
    url: null,
    tracked: false,
    available: true,
    isNew: false,
    ...overrides,
  };
}

const COURSES = [
  course({ id: 2, name: 'Programación', shortName: 'PRG', isNew: true }),
  course({ id: 1, name: 'Álgebra Lineal', available: false }),
  course({ id: 3, name: 'Cálculo', shortName: 'Cálculo' }),
];

describe('CourseSelectionList', () => {
  it('renders course names sorted, with the new tag and the unavailable note', () => {
    render(<CourseSelectionList courses={COURSES} selected={new Set()} onChange={vi.fn()} />);
    expect(screen.getByRole('group', { name: 'Courses' })).toBeInTheDocument();
    expect(screen.getAllByRole('checkbox')).toHaveLength(3);
    expect(screen.getByText('New')).toBeInTheDocument();
    expect(screen.getByText('Not visible on the platform')).toBeInTheDocument();
    expect(screen.getByText('PRG')).toBeInTheDocument();
    // A short name equal to the name is not repeated.
    expect(screen.getAllByText('Cálculo')).toHaveLength(1);
    const order = screen.getAllByRole('checkbox').map((box) => box.closest('label')?.textContent ?? '');
    expect(order[0]).toContain('Álgebra Lineal');
    expect(order[1]).toContain('Cálculo');
    expect(order[2]).toContain('Programación');
  });

  it('reflects the selection', () => {
    render(<CourseSelectionList courses={COURSES} selected={new Set([2])} onChange={vi.fn()} />);
    expect(screen.getByRole('checkbox', { name: /Programación/ })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: /Cálculo/ })).not.toBeChecked();
  });

  it('selects none, then reports a single ticked course', () => {
    const onChange = vi.fn();
    const { rerender } = render(
      <CourseSelectionList courses={COURSES} selected={new Set([1, 2, 3])} onChange={onChange} />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Select none' }));
    expect(onChange).toHaveBeenLastCalledWith(new Set());

    rerender(<CourseSelectionList courses={COURSES} selected={new Set()} onChange={onChange} />);
    fireEvent.click(screen.getByRole('checkbox', { name: /Cálculo/ }));
    expect(onChange).toHaveBeenLastCalledWith(new Set([3]));
  });

  it('selects all and unticks a course', () => {
    const onChange = vi.fn();
    const { rerender } = render(<CourseSelectionList courses={COURSES} selected={new Set()} onChange={onChange} />);
    fireEvent.click(screen.getByRole('button', { name: 'Select all' }));
    expect(onChange).toHaveBeenLastCalledWith(new Set([1, 2, 3]));

    rerender(<CourseSelectionList courses={COURSES} selected={new Set([1, 2, 3])} onChange={onChange} />);
    fireEvent.click(screen.getByRole('checkbox', { name: /Cálculo/ }));
    expect(onChange).toHaveBeenLastCalledWith(new Set([1, 2]));
  });
});
