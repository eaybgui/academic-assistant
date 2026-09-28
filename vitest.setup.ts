import '@testing-library/jest-dom/vitest';

// jsdom does not implement <dialog> behavior (showModal/close, the `open`
// attribute side effects). Modal.tsx relies on the native dialog element, so
// tests need a minimal polyfill.
if (typeof HTMLDialogElement !== 'undefined') {
  if (!HTMLDialogElement.prototype.showModal) {
    HTMLDialogElement.prototype.showModal = function showModal(this: HTMLDialogElement) {
      this.setAttribute('open', '');
    };
  }
  if (!HTMLDialogElement.prototype.close) {
    HTMLDialogElement.prototype.close = function close(this: HTMLDialogElement) {
      this.removeAttribute('open');
      this.dispatchEvent(new Event('close'));
    };
  }
}
