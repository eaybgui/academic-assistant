import type { TextareaHTMLAttributes } from 'react';
import { TEXT_FIELD } from '../styles';

export type TextareaProps = TextareaHTMLAttributes<HTMLTextAreaElement>;

/** A styled `<textarea>` for use inside a `Field`. Forwards every native textarea prop. */
export default function Textarea(props: TextareaProps) {
  return (
    <textarea
      {...props}
      className={`${TEXT_FIELD} ${props.className ?? ''}`}
    />
  );
}
