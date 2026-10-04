import type { InputHTMLAttributes } from 'react';

export type TextInputProps = InputHTMLAttributes<HTMLInputElement>;

/** A styled `<input>` for use inside a `Field`. Forwards every native input prop. */
export default function TextInput(props: TextInputProps) {
  return (
    <input
      {...props}
      className={`rounded border border-neutral-300 px-3 py-1.5 text-sm text-neutral-900 focus:border-neutral-500 focus:outline-none ${props.className ?? ''}`}
    />
  );
}
