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

  it('calls onClose on backdrop click', () => {
    const onClose = vi.fn();
    render(
      <Modal open title="Add account" onClose={onClose}>
        <p>Modal body</p>
      </Modal>,
    );

    fireEvent.click(screen.getByRole('dialog'));

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('does not call onClose when clicking inside the content', () => {
    const onClose = vi.fn();
    render(
      <Modal open title="Add account" onClose={onClose}>
        <p>Modal body</p>
      </Modal>,
    );

    fireEvent.click(screen.getByText('Modal body'));

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
});
