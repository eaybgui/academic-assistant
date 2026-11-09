import type { ReactNode } from 'react';
import { TEXT_ERROR, TEXT_MUTED_XS } from '../styles';

export interface FieldProps {
  label: string;
  htmlFor: string;
  error?: string;
  hint?: string;
  children: ReactNode;
}

/** A labeled form field wrapper: label, the control, and an optional hint or validation error below it. */
export default function Field({ label, htmlFor, error, hint, children }: FieldProps) {
  return (
    <div className="flex flex-col gap-1">
      <label htmlFor={htmlFor} className="text-sm font-medium text-neutral-700">
        {label}
      </label>
      {children}
      {error ? (
        <p role="alert" className={TEXT_ERROR}>
          {error}
        </p>
      ) : hint ? (
        <p className={TEXT_MUTED_XS}>{hint}</p>
      ) : null}
    </div>
  );
}
