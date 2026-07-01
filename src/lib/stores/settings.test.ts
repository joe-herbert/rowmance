import { describe, it, expect, vi, beforeEach } from 'vitest';
import { DEFAULT_SETTINGS } from '$lib/types';

let useSettings: typeof import('./settings.svelte').useSettings;
let mockGetAllSettings: ReturnType<typeof vi.fn>;
let mockSetSetting: ReturnType<typeof vi.fn>;

beforeEach(async () => {
  vi.resetModules();

  mockGetAllSettings = vi.fn().mockResolvedValue({});
  mockSetSetting = vi.fn().mockResolvedValue(undefined);

  vi.doMock('$lib/tauri/settings', () => ({
    getAllSettings: mockGetAllSettings,
    setSetting: mockSetSetting,
    resetSetting: vi.fn().mockResolvedValue(undefined),
  }));

  vi.doMock('$lib/tauri/window', () => ({
    syncTrafficLightPosition: vi.fn(),
  }));

  ({ useSettings } = await import('./settings.svelte'));
});

describe('initial state', () => {
  it('starts with default settings and loaded=false', () => {
    const store = useSettings();
    expect(store.settings).toEqual(DEFAULT_SETTINGS);
    expect(store.loaded).toBe(false);
  });
});

describe('load()', () => {
  it('merges persisted settings over defaults', async () => {
    mockGetAllSettings.mockResolvedValue({ theme: 'dark', pageSize: 100 });

    const store = useSettings();
    await store.load();

    expect(store.settings.theme).toBe('dark');
    expect(store.settings.pageSize).toBe(100);
    expect(store.settings.autoSaveEdits).toBe(DEFAULT_SETTINGS.autoSaveEdits);
    expect(store.loaded).toBe(true);
  });

  it('uses defaults when the backend call fails', async () => {
    mockGetAllSettings.mockRejectedValue(new Error('IPC unavailable'));

    const store = useSettings();
    await store.load();

    expect(store.settings).toEqual(DEFAULT_SETTINGS);
    expect(store.loaded).toBe(true);
  });

  it('sets the data-theme attribute on document.documentElement', async () => {
    mockGetAllSettings.mockResolvedValue({ theme: 'light' });

    const store = useSettings();
    await store.load();

    expect(document.documentElement.getAttribute('data-theme')).toBe('light');
  });

  it('persists the theme to localStorage', async () => {
    mockGetAllSettings.mockResolvedValue({ theme: 'dark' });

    const store = useSettings();
    await store.load();

    expect(localStorage.getItem('rowmance_theme')).toBe('dark');
  });

  it('sets font CSS variables from persisted settings', async () => {
    mockGetAllSettings.mockResolvedValue({ fontFamily: 'monospace', fontSize: 16 });

    const store = useSettings();
    await store.load();

    expect(document.documentElement.style.getPropertyValue('--font-family-ui')).toBe('monospace');
    expect(document.documentElement.style.getPropertyValue('--font-size-md')).toBe('16px');
  });
});

describe('set()', () => {
  it('updates the in-memory setting immediately', async () => {
    const store = useSettings();
    await store.load();

    await store.set('pageSize', 200);

    expect(store.settings.pageSize).toBe(200);
  });

  it('calls setSetting on the backend', async () => {
    const store = useSettings();
    await store.load();

    await store.set('pageSize', 200);

    expect(mockSetSetting).toHaveBeenCalledWith('pageSize', 200);
  });

  it('updates data-theme when theme setting changes', async () => {
    const store = useSettings();
    await store.load();

    await store.set('theme', 'dark');

    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
  });

  it('updates font CSS variables when fontFamily changes', async () => {
    const store = useSettings();
    await store.load();

    await store.set('fontFamily', 'Georgia');

    expect(document.documentElement.style.getPropertyValue('--font-family-ui')).toBe('Georgia');
  });

  it('updates font CSS variables when fontSize changes', async () => {
    const store = useSettings();
    await store.load();

    await store.set('fontSize', 14);

    expect(document.documentElement.style.getPropertyValue('--font-size-md')).toBe('14px');
  });

  it('does not update DOM when an unrelated setting changes', async () => {
    mockGetAllSettings.mockResolvedValue({ theme: 'light' });

    const store = useSettings();
    await store.load();
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');

    await store.set('pageSize', 75);

    expect(document.documentElement.getAttribute('data-theme')).toBe('light');
  });
});
