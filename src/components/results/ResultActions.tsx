import { openUrl } from '@tauri-apps/plugin-opener';
import type { ResultDto } from '../../lib/api/types';

export interface ResultActionsProps {
  result: Pick<ResultDto, 'sourceUrl' | 'messageIdHeader'>;
  className?: string;
}

const DEFAULT_CLASS_NAME = 'text-neutral-600 hover:text-neutral-900';

/**
 * "Open email" (when the result has a `sourceUrl`, opened in the system browser)
 * or "Copy Message-ID" (when it has a `messageIdHeader`, copied to the clipboard).
 * Renders nothing when neither is available. Shared by `ResultCard` (dashboard)
 * and the results table's row detail.
 */
export default function ResultActions({ result, className }: ResultActionsProps) {
  async function handleOpenEmail() {
    if (result.sourceUrl) {
      await openUrl(result.sourceUrl);
    }
  }

  async function handleCopyMessageId() {
    if (result.messageIdHeader) {
      await navigator.clipboard.writeText(result.messageIdHeader);
    }
  }

  if (result.sourceUrl) {
    return (
      <button type="button" onClick={() => void handleOpenEmail()} className={className ?? DEFAULT_CLASS_NAME}>
        Open email
      </button>
    );
  }

  if (result.messageIdHeader) {
    return (
      <button type="button" onClick={() => void handleCopyMessageId()} className={className ?? DEFAULT_CLASS_NAME}>
        Copy Message-ID
      </button>
    );
  }

  return null;
}
