import type { InputHTMLAttributes } from 'react';
import { TEXT_FIELD } from '../styles';

export type TextInputProps = InputHTMLAttributes<HTMLInputElement>;

/** A styled `<input>` for use inside a `Field`. Forwards every native input prop. */
export default function TextInput(props: TextInputProps) {
  return (
    <input
      {...props}
      className={`${TEXT_FIELD} ${props.className ?? ''}`}
    />
  );
}
