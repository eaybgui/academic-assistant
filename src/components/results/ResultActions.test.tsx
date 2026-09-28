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
  });

  it('renders nothing when neither is available', () => {
    const { container } = render(<ResultActions result={{ sourceUrl: null, messageIdHeader: null }} />);
    expect(container).toBeEmptyDOMElement();
  });
});
