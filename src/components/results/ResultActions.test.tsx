import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import ResultActions from './ResultActions';

const { openUrl } = vi.hoisted(() => ({ openUrl: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl }));

beforeEach(() => {
  openUrl.mockReset().mockResolvedValue(undefined);
});

describe('ResultActions', () => {
  it('opens the source URL when present', async () => {
    const user = userEvent.setup();
    render(<ResultActions result={{ sourceUrl: 'https://mail.example/msg/1', messageIdHeader: null }} />);

    await user.click(screen.getByRole('button', { name: 'Open email' }));

    expect(openUrl).toHaveBeenCalledWith('https://mail.example/msg/1');
  });

  it('copies the Message-ID when there is no source URL', async () => {
    // `userEvent.setup()` installs its own `navigator.clipboard` stub, so the
    // spy must be attached to that instance (after setup), not a standalone mock.
    const user = userEvent.setup();
    const writeText = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue(undefined);
    render(<ResultActions result={{ sourceUrl: null, messageIdHeader: '<abc@uv.es>' }} />);

    await user.click(screen.getByRole('button', { name: 'Copy Message-ID' }));

    expect(writeText).toHaveBeenCalledWith('<abc@uv.es>');
    expect(await screen.findByText('Copied')).toBeInTheDocument();
  });

  it('renders nothing when neither is available', () => {
    const { container } = render(<ResultActions result={{ sourceUrl: null, messageIdHeader: null }} />);
    expect(container).toBeEmptyDOMElement();
  });

  it('falls back to Copy Message-ID for a non-https source URL', () => {
    render(
      <ResultActions result={{ sourceUrl: 'file:///C:/mail/1.eml', messageIdHeader: '<abc@uv.es>' }} />,
    );

    expect(screen.queryByRole('button', { name: 'Open email' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Copy Message-ID' })).toBeInTheDocument();
  });

  it('renders nothing for an unparseable source URL and no Message-ID', () => {
    const { container } = render(<ResultActions result={{ sourceUrl: 'not a url', messageIdHeader: null }} />);
    expect(container).toBeEmptyDOMElement();
  });

  it('shows inline feedback when opening the email fails', async () => {
    openUrl.mockRejectedValueOnce(new Error('denied'));
    const user = userEvent.setup();
    render(<ResultActions result={{ sourceUrl: 'https://mail.example/msg/1', messageIdHeader: null }} />);

    await user.click(screen.getByRole('button', { name: 'Open email' }));

    expect(await screen.findByText("Couldn't open the email")).toBeInTheDocument();
  });

  it('shows inline feedback when copying fails', async () => {
    const user = userEvent.setup();
    vi.spyOn(navigator.clipboard, 'writeText').mockRejectedValue(new Error('denied'));
    render(<ResultActions result={{ sourceUrl: null, messageIdHeader: '<abc@uv.es>' }} />);

    await user.click(screen.getByRole('button', { name: 'Copy Message-ID' }));

    expect(await screen.findByText("Couldn't copy")).toBeInTheDocument();
  });
});
