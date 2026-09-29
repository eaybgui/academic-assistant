import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import Modal from './Modal';

describe('Modal', () => {
  it('renders title and children when open', () => {
    render(
      <Modal open title="Add account" onClose={() => {}}>
        <p>Modal body</p>
      </Modal>,
    );

    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(screen.getByText('Add account')).toBeInTheDocument();
    expect(screen.getByText('Modal body')).toBeInTheDocument();
  });

  it('renders the footer when provided', () => {
    render(
      <Modal open title="Add account" onClose={() => {}} footer={<button type="button">Save</button>}>
        <p>Modal body</p>
      </Modal>,
    );

    expect(screen.getByRole('button', { name: 'Save' })).toBeInTheDocument();
  });

  it('does not open the dialog when open=false', () => {
    render(
      <Modal open={false} title="Add account" onClose={() => {}}>
        <p>Modal body</p>
      </Modal>,
    );

    expect(screen.getByRole('dialog', { hidden: true })).not.toHaveAttribute('open');
  });

  it('calls onClose on Escape', () => {
    const onClose = vi.fn();
    render(
      <Modal open title="Add account" onClose={onClose}>
        <p>Modal body</p>
      </Modal>,
    );

    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('calls onClose when mousedown and mouseup both land on the backdrop', () => {
    const onClose = vi.fn();
    render(
      <Modal open title="Add account" onClose={onClose}>
        <p>Modal body</p>
      </Modal>,
    );

    const dialog = screen.getByRole('dialog');
    fireEvent.mouseDown(dialog);
    fireEvent.mouseUp(dialog);

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('does not call onClose when the mousedown starts inside the content and the mouseup lands on the backdrop', () => {
    const onClose = vi.fn();
    render(
      <Modal open title="Add account" onClose={onClose}>
        <p>Modal body</p>
      </Modal>,
    );

    const dialog = screen.getByRole('dialog');
    fireEvent.mouseDown(screen.getByText('Modal body'));
    fireEvent.mouseUp(dialog);

    expect(onClose).not.toHaveBeenCalled();
  });

  it('does not call onClose when clicking inside the content', () => {
    const onClose = vi.fn();
    render(
      <Modal open title="Add account" onClose={onClose}>
        <p>Modal body</p>
      </Modal>,
    );

    fireEvent.mouseDown(screen.getByText('Modal body'));
    fireEvent.mouseUp(screen.getByText('Modal body'));

    expect(onClose).not.toHaveBeenCalled();
  });

  it('calls onClose when the close button is clicked', () => {
    const onClose = vi.fn();
    render(
      <Modal open title="Add account" onClose={onClose}>
        <p>Modal body</p>
      </Modal>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Close' }));

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('is centered and height-capped so tall content scrolls inside the viewport', () => {
    render(
      <Modal open title="Add account" onClose={() => {}}>
        <p>Modal body</p>
      </Modal>,
    );

    const dialog = screen.getByRole('dialog');
    expect(dialog).toHaveClass('m-auto', 'max-h-[calc(100vh-2rem)]');
    expect(screen.getByText('Modal body').parentElement).toHaveClass('overflow-y-auto');
  });

  it('does not render the body while closed', () => {
    render(
      <Modal open={false} title="Add account" onClose={() => {}}>
        <p>Modal body</p>
      </Modal>,
    );

    expect(screen.queryByText('Modal body')).not.toBeInTheDocument();
  });

  it('focuses the first focusable element in the body when opened', () => {
    render(
      <Modal open title="Add account" onClose={() => {}} footer={<button type="button">Save</button>}>
        <input aria-label="Name" />
      </Modal>,
    );

    expect(screen.getByLabelText('Name')).toHaveFocus();
  });

  it('falls back to focusing the dialog when the body has no focusable element', () => {
    render(
      <Modal open title="Add account" onClose={() => {}}>
        <p>Modal body</p>
      </Modal>,
    );

    expect(screen.getByRole('dialog')).toHaveFocus();
  });
});
