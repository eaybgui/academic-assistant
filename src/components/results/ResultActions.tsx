import { useState } from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';
import type { ResultDto } from '../../lib/api/types';

export interface ResultActionsProps {
  result: Pick<ResultDto, 'sourceUrl' | 'messageIdHeader'>;
  className?: string;
}

const DEFAULT_CLASS_NAME = 'text-neutral-600 hover:text-neutral-900';

/** Whether `url` is safe to hand to the system browser: parses, and is `https:`. */
function isOpenableUrl(url: string): boolean {
  try {
    return new URL(url).protocol === 'https:';
  } catch {
    return false;
  }
}

/**
 * "Open email" (when the result has an `https` `sourceUrl`, opened in the system browser) or
 * "Copy Message-ID" (when it has a `messageIdHeader`, copied to the clipboard). Renders nothing
 * when neither is available. Used by the announcements results table row
 * detail.
 */
export default function ResultActions({ result, className }: ResultActionsProps) {
  const [feedback, setFeedback] = useState<string | null>(null);
  const canOpenEmail = Boolean(result.sourceUrl && isOpenableUrl(result.sourceUrl));

  async function handleOpenEmail() {
    if (!result.sourceUrl) return;
    try {
      await openUrl(result.sourceUrl);
      setFeedback(null);
    } catch {
      setFeedback("Couldn't open the email");
    }
  }

  async function handleCopyMessageId() {
    if (!result.messageIdHeader) return;
    try {
      await navigator.clipboard.writeText(result.messageIdHeader);
      setFeedback('Copied');
    } catch {
      setFeedback("Couldn't copy");
    }
  }

  if (canOpenEmail) {
    return (
      <span className="inline-flex items-center gap-2">
        <button type="button" onClick={() => void handleOpenEmail()} className={className ?? DEFAULT_CLASS_NAME}>
          Open email
        </button>
        {feedback ? <span className="text-xs text-neutral-500">{feedback}</span> : null}
      </span>
    );
  }

  if (result.messageIdHeader) {
    return (
      <span className="inline-flex items-center gap-2">
        <button
          type="button"
          onClick={() => void handleCopyMessageId()}
          className={className ?? DEFAULT_CLASS_NAME}
        >
          Copy Message-ID
        </button>
        {feedback ? <span className="text-xs text-neutral-500">{feedback}</span> : null}
      </span>
    );
  }

  return null;
}
