import type { TextareaHTMLAttributes } from 'react';

export type TextareaProps = TextareaHTMLAttributes<HTMLTextAreaElement>;

/** A styled `<textarea>` for use inside a `Field`. Forwards every native textarea prop. */
export default function Textarea(props: TextareaProps) {
  return (
    <textarea
      {...props}
      className={`rounded border border-neutral-300 px-3 py-1.5 text-sm text-neutral-900 focus:border-neutral-500 focus:outline-none ${props.className ?? ''}`}
    />
  );
}
