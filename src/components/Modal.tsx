import { useEffect, useId, useRef, type KeyboardEvent, type MouseEvent, type ReactNode } from 'react';

export interface ModalProps {
  open: boolean;
  title: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
}

const FOCUSABLE_SELECTOR =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * A modal built on the native `<dialog>` element. Closes on Escape and on a
 * click outside the dialog's content (the native backdrop area).
 *
 * `m-auto` restores the UA centering that Tailwind's preflight (`margin: 0`)
 * removes. The height cap makes only the body scroll, keeping the header and
 * footer visible.
 *
 * The dialog's own `close` event is the single source of truth for calling
 * `onClose`: both the Escape key and the backdrop click ask the dialog to
 * close itself, and that event notifies the caller so `open` can be updated.
 *
 * The body (and its form state, owned by the caller) is only rendered while
 * `open` is true, so a caller that keeps its state in the modal's own
 * component tree gets a fresh mount every time the modal opens instead of
 * silently reusing whatever was left over from the last time it was shown.
 */
export default function Modal({ open, title, onClose, children, footer }: ModalProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const bodyRef = useRef<HTMLDivElement>(null);
  const titleId = useId();
  // Tracks whether the mousedown that might start a backdrop click landed on
  // the backdrop itself, so a drag that starts on the backdrop but ends
  // inside the content (or vice versa) doesn't close the dialog.
  const backdropMouseDownRef = useRef(false);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    if (open && !dialog.open) {
      dialog.showModal();
      const focusTarget = bodyRef.current?.querySelector<HTMLElement>(FOCUSABLE_SELECTOR) ?? dialog;
      focusTarget.focus();
    } else if (!open && dialog.open) {
      dialog.close();
    }
  }, [open]);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    dialog.addEventListener('close', onClose);
    return () => dialog.removeEventListener('close', onClose);
  }, [onClose]);

  function handleKeyDown(event: KeyboardEvent<HTMLDialogElement>) {
    if (event.key === 'Escape') {
      event.preventDefault();
      dialogRef.current?.close();
    }
  }

  function handleBackdropMouseDown(event: MouseEvent<HTMLDialogElement>) {
    backdropMouseDownRef.current = event.target === dialogRef.current;
  }

  function handleBackdropMouseUp(event: MouseEvent<HTMLDialogElement>) {
    if (backdropMouseDownRef.current && event.target === dialogRef.current) {
      dialogRef.current?.close();
    }
    backdropMouseDownRef.current = false;
  }

  return (
    <dialog
      ref={dialogRef}
      tabIndex={-1}
      aria-labelledby={titleId}
      onKeyDown={handleKeyDown}
      onMouseDown={handleBackdropMouseDown}
      onMouseUp={handleBackdropMouseUp}
      className="m-auto max-h-[calc(100vh-2rem)] w-full max-w-lg rounded-lg border border-neutral-200 bg-white p-0 shadow-xl backdrop:bg-neutral-900/40 open:flex open:flex-col"
    >
      {open ? (
        <div className="flex min-h-0 flex-1 flex-col" onClick={(e) => e.stopPropagation()}>
          <header className="flex shrink-0 items-center justify-between border-b border-neutral-200 px-5 py-4">
            <h2 id={titleId} className="text-lg font-semibold text-neutral-900">
              {title}
            </h2>
            <button
              type="button"
              aria-label="Close"
              onClick={() => dialogRef.current?.close()}
              className="rounded text-neutral-400 hover:text-neutral-700"
            >
              &times;
            </button>
          </header>
          <div ref={bodyRef} className="min-h-0 flex-1 overflow-y-auto px-5 py-4">
            {children}
          </div>
          {footer ? (
            <footer className="flex shrink-0 justify-end gap-2 border-t border-neutral-200 px-5 py-3">{footer}</footer>
          ) : null}
        </div>
      ) : null}
    </dialog>
  );
}
