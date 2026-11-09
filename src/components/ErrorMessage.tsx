import type { CommandError } from '../lib/api/types';
import { TEXT_ERROR } from './styles';

export interface ErrorMessageProps {
  error: CommandError | null;
}

/** Renders a command error inline, next to the control that triggered it. Renders nothing when there is no error. */
export default function ErrorMessage({ error }: ErrorMessageProps) {
  if (!error) return null;

  return (
    <p role="alert" className={TEXT_ERROR}>
      {error.message}
    </p>
  );
}
