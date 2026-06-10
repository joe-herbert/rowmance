import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { getAllSettings, setSetting, resetSetting } from './settings';

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  mockInvoke.mockReset();
});

describe('getAllSettings', () => {
  it('invokes settings_get_all with no arguments', async () => {
    mockInvoke.mockResolvedValue({ theme: 'dark' });
    const result = await getAllSettings();
    expect(mockInvoke).toHaveBeenCalledWith('settings_get_all');
    expect(result).toEqual({ theme: 'dark' });
  });
});

describe('setSetting', () => {
  it('invokes settings_set with key and value', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await setSetting('theme', 'dark');
    expect(mockInvoke).toHaveBeenCalledWith('settings_set', { key: 'theme', value: 'dark' });
  });

  it('works with numeric values', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await setSetting('pageSize', 100);
    expect(mockInvoke).toHaveBeenCalledWith('settings_set', { key: 'pageSize', value: 100 });
  });

  it('works with boolean values', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await setSetting('autoSaveEdits', true);
    expect(mockInvoke).toHaveBeenCalledWith('settings_set', { key: 'autoSaveEdits', value: true });
  });
});

describe('resetSetting', () => {
  it('invokes settings_reset with the key', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await resetSetting('theme');
    expect(mockInvoke).toHaveBeenCalledWith('settings_reset', { key: 'theme' });
  });
});
