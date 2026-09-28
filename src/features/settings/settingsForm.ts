import type { SettingsDto, SettingsInput } from '../../lib/api/types';

/** Editable state behind the settings form. `blockedSenders` stays as raw textarea text while being edited. */
export interface SettingsFormState {
  aiProvider: 'ollama' | 'jev';
  jevModel: string;
  ollamaBaseUrl: string;
  ollamaModel: string;
  microsoftClientId: string;
  blockedSenders: string;
}

export function settingsToFormState(settings: SettingsDto): SettingsFormState {
  return {
    aiProvider: settings.aiProvider,
    jevModel: settings.jevModel,
    ollamaBaseUrl: settings.ollamaBaseUrl,
    ollamaModel: settings.ollamaModel,
    microsoftClientId: settings.microsoftClientId ?? '',
    blockedSenders: settings.blockedSenders.join('\n'),
  };
}

/** Parses the blocked-senders textarea (one entry per line) into a trimmed, non-empty array. */
export function parseBlockedSenders(raw: string): string[] {
  return raw
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
}

export function buildSettingsInput(state: SettingsFormState): SettingsInput {
  return {
    aiProvider: state.aiProvider,
    jevModel: state.jevModel.trim(),
    ollamaBaseUrl: state.ollamaBaseUrl.trim(),
    ollamaModel: state.ollamaModel.trim(),
    microsoftClientId: state.microsoftClientId.trim() ? state.microsoftClientId.trim() : null,
    blockedSenders: parseBlockedSenders(state.blockedSenders),
  };
}
