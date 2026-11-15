import { afterEach, describe, expect, it, vi } from 'vitest';

async function loadEmailFlag(): Promise<boolean> {
  vi.resetModules();
  return (await import('./features')).EMAIL_FEATURES_ENABLED;
}

afterEach(() => {
  vi.unstubAllEnvs();
});

describe('EMAIL_FEATURES_ENABLED', () => {
  it('is on when VITE_EMAIL_FEATURES is "true" outside Android', async () => {
    vi.stubEnv('MODE', 'production');
    vi.stubEnv('VITE_EMAIL_FEATURES', 'true');

    expect(await loadEmailFlag()).toBe(true);
  });

  it('is off on Android even when VITE_EMAIL_FEATURES is "true"', async () => {
    vi.stubEnv('MODE', 'android');
    vi.stubEnv('VITE_EMAIL_FEATURES', 'true');

    expect(await loadEmailFlag()).toBe(false);
  });
});
