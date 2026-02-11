import { beforeEach, describe, expect, it, vi } from 'vitest';

describe('app updater service env helpers', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.unstubAllEnvs();
  });

  it('reads disabled flag from env', async () => {
    vi.stubEnv('VITE_AUTO_UPDATER_DISABLED', 'true');
    const mod = await import('./app-updater');
    expect(mod.isAutoUpdaterDisabledByEnv()).toBe(true);
  });

  it('uses default interval for invalid env values', async () => {
    vi.stubEnv('VITE_UPDATE_CHECK_INTERVAL_MS', '-10');
    const mod = await import('./app-updater');
    expect(mod.getUpdateCheckIntervalMs()).toBe(3600000);
  });

  it('uses interval from env when valid', async () => {
    vi.stubEnv('VITE_UPDATE_CHECK_INTERVAL_MS', '90000');
    const mod = await import('./app-updater');
    expect(mod.getUpdateCheckIntervalMs()).toBe(90000);
  });
});
