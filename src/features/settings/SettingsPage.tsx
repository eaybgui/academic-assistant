import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api, toCommandError } from '../../lib/api/client';
import { queryKeys } from '../../lib/api/queryKeys';
import ErrorMessage from '../../components/ErrorMessage';
import Field from '../../components/forms/Field';
import TextInput from '../../components/forms/TextInput';
import Textarea from '../../components/forms/Textarea';
import type { CommandError } from '../../lib/api/types';
import JevKeyModal from './JevKeyModal';
import { buildSettingsInput, settingsToFormState, type SettingsFormState } from './settingsForm';

const PROVIDERS = [
  {
    value: 'jev' as const,
    title: 'jev (external)',
    description: 'Sends email text to TypeSafe.',
  },
  {
    value: 'ollama' as const,
    title: 'Ollama (local)',
    description: 'Stays on this PC.',
  },
];

/** Settings screen: AI provider selection, jev/Ollama configuration, Microsoft sign-in and filters. */
export default function SettingsPage() {
  const queryClient = useQueryClient();
  const settingsQuery = useQuery({ queryKey: queryKeys.settings, queryFn: api.getSettings });

  const [form, setForm] = useState<SettingsFormState | null>(null);
  const [keyModalOpen, setKeyModalOpen] = useState(false);
  const [saveError, setSaveError] = useState<CommandError | null>(null);
  const [testResult, setTestResult] = useState<string | null>(null);

  // Initialize (or resync) the editable form whenever a fresh `SettingsDto` arrives, without
  // clobbering in-progress edits on every render. Setting state during render (guarded by
  // comparing against the last seen data) is React's documented alternative to an effect here.
  const [loadedSettings, setLoadedSettings] = useState(settingsQuery.data);
  if (settingsQuery.data && settingsQuery.data !== loadedSettings) {
    setLoadedSettings(settingsQuery.data);
    setForm(settingsToFormState(settingsQuery.data));
  }

  function invalidateSettings() {
    void queryClient.invalidateQueries({ queryKey: queryKeys.settings });
  }

  const saveMutation = useMutation({
    mutationFn: (state: SettingsFormState) => api.updateSettings(buildSettingsInput(state)),
    onSuccess: () => {
      setSaveError(null);
      invalidateSettings();
    },
    onError: (e) => setSaveError(toCommandError(e)),
  });

  const removeKeyMutation = useMutation({
    mutationFn: () => api.clearJevApiKey(),
    onSuccess: invalidateSettings,
    onError: (e) => setSaveError(toCommandError(e)),
  });

  const testProviderMutation = useMutation({
    mutationFn: () => api.testAiProvider(),
    onSuccess: (result) =>
      setTestResult(result.ok ? 'Connection successful.' : (result.message ?? 'Connection failed.')),
    onError: (e) => setTestResult(toCommandError(e).message),
  });

  if (!form) {
    return (
      <div className="flex flex-col gap-4">
        <h1 className="text-2xl font-semibold text-neutral-900">Settings</h1>
        {settingsQuery.isLoading ? <p className="text-sm text-neutral-500">Loading…</p> : null}
      </div>
    );
  }

  function update<K extends keyof SettingsFormState>(key: K, value: SettingsFormState[K]) {
    setForm((prev) => (prev ? { ...prev, [key]: value } : prev));
  }

  return (
    <div className="flex flex-col gap-6">
      <h1 className="text-2xl font-semibold text-neutral-900">Settings</h1>

      <section className="flex flex-col gap-3">
        <h2 className="text-lg font-semibold text-neutral-900">AI provider</h2>
        <div className="grid grid-cols-2 gap-3">
          {PROVIDERS.map((provider) => (
            <button
              key={provider.value}
              type="button"
              onClick={() => update('aiProvider', provider.value)}
              className={`rounded border px-4 py-3 text-left ${
                form.aiProvider === provider.value
                  ? 'border-neutral-900 bg-neutral-50'
                  : 'border-neutral-200 bg-white hover:bg-neutral-50'
              }`}
            >
              <p className="font-semibold text-neutral-900">{provider.title}</p>
              <p className="text-sm text-neutral-500">{provider.description}</p>
            </button>
          ))}
        </div>

        <div className="grid grid-cols-2 gap-4">
          <Field label="jev API key" htmlFor="jev-key-status">
            <div className="flex items-center gap-2">
              <TextInput
                id="jev-key-status"
                readOnly
                value={settingsQuery.data?.jevApiKeyConfigured ? 'Configured' : 'Not configured'}
              />
            </div>
            <div className="mt-1 flex gap-3 text-sm">
              <button type="button" onClick={() => setKeyModalOpen(true)} className="text-neutral-700 hover:underline">
                Replace key
              </button>
              <button
                type="button"
                disabled={!settingsQuery.data?.jevApiKeyConfigured || removeKeyMutation.isPending}
                onClick={() => removeKeyMutation.mutate()}
                className="text-neutral-700 hover:underline disabled:opacity-50"
              >
                Remove key
              </button>
            </div>
          </Field>
          <Field label="jev model" htmlFor="jev-model">
            <TextInput id="jev-model" value={form.jevModel} onChange={(e) => update('jevModel', e.target.value)} />
          </Field>
          <Field label="Ollama URL" htmlFor="ollama-url">
            <TextInput
              id="ollama-url"
              value={form.ollamaBaseUrl}
              onChange={(e) => update('ollamaBaseUrl', e.target.value)}
            />
          </Field>
          <Field label="Ollama model" htmlFor="ollama-model">
            <TextInput
              id="ollama-model"
              value={form.ollamaModel}
              onChange={(e) => update('ollamaModel', e.target.value)}
            />
          </Field>
        </div>

        <div className="flex items-center gap-3">
          <button
            type="button"
            disabled={testProviderMutation.isPending}
            onClick={() => testProviderMutation.mutate()}
            className="rounded border border-neutral-300 px-3 py-1.5 text-sm text-neutral-700 hover:bg-neutral-50 disabled:opacity-50"
          >
            {testProviderMutation.isPending ? 'Testing…' : 'Test provider'}
          </button>
          {testResult ? <p className="text-sm text-neutral-700">{testResult}</p> : null}
        </div>
      </section>

      <section className="flex flex-col gap-3">
        <h2 className="text-lg font-semibold text-neutral-900">Microsoft sign-in</h2>
        <Field label="Entra application (client) ID" htmlFor="microsoft-client-id">
          <TextInput
            id="microsoft-client-id"
            placeholder="00000000-0000-0000-0000-000000000000"
            value={form.microsoftClientId}
            onChange={(e) => update('microsoftClientId', e.target.value)}
          />
        </Field>
      </section>

      <section className="flex flex-col gap-3">
        <h2 className="text-lg font-semibold text-neutral-900">Filters</h2>
        <Field label="Blocked senders and domains (one per line)" htmlFor="blocked-senders">
          <Textarea
            id="blocked-senders"
            rows={5}
            value={form.blockedSenders}
            onChange={(e) => update('blockedSenders', e.target.value)}
          />
        </Field>
      </section>

      <ErrorMessage error={saveError} />

      <div>
        <button
          type="button"
          disabled={saveMutation.isPending}
          onClick={() => saveMutation.mutate(form)}
          className="rounded bg-neutral-900 px-4 py-2 text-sm font-medium text-white hover:bg-neutral-800 disabled:opacity-50"
        >
          {saveMutation.isPending ? 'Saving…' : 'Save'}
        </button>
      </div>

      <JevKeyModal open={keyModalOpen} onClose={() => setKeyModalOpen(false)} />
    </div>
  );
}
