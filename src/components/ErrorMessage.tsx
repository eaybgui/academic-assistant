import type { CommandError } from '../lib/api/types';

export interface ErrorMessageProps {
  error: CommandError | null;
}

/** Renders a command error inline, next to the control that triggered it. Renders nothing when there is no error. */
export default function ErrorMessage({ error }: ErrorMessageProps) {
  if (!error) return null;

  return (
    <p role="alert" className="text-sm text-red-600">
      {error.message}
    </p>
  );
}
