import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { useState } from 'react';
import Modal from '../../components/Modal';
import { useAndroidBack } from './useAndroidBack';

const { onBackButtonPress, androidLeaveApp, unregister } = vi.hoisted(() => ({
  onBackButtonPress: vi.fn(),
  androidLeaveApp: vi.fn(),
  unregister: vi.fn(),
}));
const platform = vi.hoisted(() => ({ IS_ANDROID: true }));

vi.mock('@tauri-apps/api/app', () => ({ onBackButtonPress }));
vi.mock('../../lib/api/client', () => ({ api: { androidLeaveApp } }));
vi.mock('../../config/platform', () => platform);

type BackHandler = (payload: { canGoBack: boolean }) => void;

function DialogHost({ title }: { title: string }) {
  const [open, setOpen] = useState(true);
  return (
    <Modal open={open} title={title} onClose={() => setOpen(false)}>
      <p>{title} body</p>
    </Modal>
  );
}

function Harness({ dialogs }: { dialogs: number }) {
  useAndroidBack();
  return (
    <>
      {Array.from({ length: dialogs }, (_, index) => (
        <DialogHost key={index} title={`Dialog ${index + 1}`} />
      ))}
    </>
  );
}

async function renderAndCaptureHandler(dialogs = 0): Promise<BackHandler> {
  render(<Harness dialogs={dialogs} />);
  await waitFor(() => expect(onBackButtonPress).toHaveBeenCalledTimes(1));
  return onBackButtonPress.mock.calls[0][0] as BackHandler;
}

beforeEach(() => {
  platform.IS_ANDROID = true;
  onBackButtonPress.mockReset().mockResolvedValue({ unregister });
  androidLeaveApp.mockReset().mockResolvedValue(undefined);
  unregister.mockReset();
  vi.spyOn(window.history, 'back').mockImplementation(() => {});
});

describe('useAndroidBack', () => {
  it('closes an open dialog and neither navigates nor exits', async () => {
    const handler = await renderAndCaptureHandler(1);
    expect(screen.getByRole('dialog')).toBeInTheDocument();

    handler({ canGoBack: true });

    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(window.history.back).not.toHaveBeenCalled();
    expect(androidLeaveApp).not.toHaveBeenCalled();
  });

  it('closes only the topmost dialog when two are open', async () => {
    const handler = await renderAndCaptureHandler(2);

    handler({ canGoBack: true });

    await waitFor(() => expect(screen.queryByText('Dialog 2 body')).not.toBeInTheDocument());
    expect(screen.getByText('Dialog 1 body')).toBeInTheDocument();
  });

  it('goes back in the history when no dialog is open and the webview can go back', async () => {
    const handler = await renderAndCaptureHandler();

    handler({ canGoBack: true });

    expect(window.history.back).toHaveBeenCalledTimes(1);
    expect(androidLeaveApp).not.toHaveBeenCalled();
  });

  it('leaves the app when no dialog is open and the webview cannot go back', async () => {
    const handler = await renderAndCaptureHandler();

    handler({ canGoBack: false });

    expect(androidLeaveApp).toHaveBeenCalledTimes(1);
    expect(window.history.back).not.toHaveBeenCalled();
  });

  it('stops listening when unmounted', async () => {
    const { unmount } = render(<Harness dialogs={0} />);
    await waitFor(() => expect(onBackButtonPress).toHaveBeenCalled());
    await Promise.resolve();

    unmount();

    expect(unregister).toHaveBeenCalledTimes(1);
  });

  it('does not register a listener when not on Android', () => {
    platform.IS_ANDROID = false;

    render(<Harness dialogs={0} />);

    expect(onBackButtonPress).not.toHaveBeenCalled();
  });
});
