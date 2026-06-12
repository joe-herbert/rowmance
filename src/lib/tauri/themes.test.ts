import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { themesList, themesRead, themesWrite, themesDelete, themesDuplicate } from './themes';
import type { ThemeData } from '$lib/types';

const mockInvoke = vi.mocked(invoke);

const stubMeta = { name: 'dracula', extends: 'dark' };
const stubData: ThemeData = {
  name: 'dracula',
  extends: 'dark',
  variables: { '--color-bg-primary': '#282a36' },
};

beforeEach(() => {
  mockInvoke.mockReset();
});

describe('themesList', () => {
  it('invokes themes_list with no arguments', async () => {
    mockInvoke.mockResolvedValue([stubMeta]);
    await themesList();
    expect(mockInvoke).toHaveBeenCalledWith('themes_list');
  });

  it('returns an array of ThemeMeta', async () => {
    mockInvoke.mockResolvedValue([stubMeta]);
    const result = await themesList();
    expect(result).toEqual([stubMeta]);
  });

  it('returns empty array when no themes exist', async () => {
    mockInvoke.mockResolvedValue([]);
    const result = await themesList();
    expect(result).toEqual([]);
  });
});

describe('themesRead', () => {
  it('invokes themes_read with name', async () => {
    mockInvoke.mockResolvedValue(stubData);
    await themesRead('dracula');
    expect(mockInvoke).toHaveBeenCalledWith('themes_read', { name: 'dracula' });
  });

  it('returns ThemeData', async () => {
    mockInvoke.mockResolvedValue(stubData);
    const result = await themesRead('dracula');
    expect(result).toBe(stubData);
  });
});

describe('themesWrite', () => {
  it('invokes themes_write with name and data', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await themesWrite('dracula', stubData);
    expect(mockInvoke).toHaveBeenCalledWith('themes_write', { name: 'dracula', data: stubData });
  });
});

describe('themesDelete', () => {
  it('invokes themes_delete with name', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await themesDelete('dracula');
    expect(mockInvoke).toHaveBeenCalledWith('themes_delete', { name: 'dracula' });
  });
});

describe('themesDuplicate', () => {
  it('invokes themes_duplicate with source and newName', async () => {
    mockInvoke.mockResolvedValue(stubMeta);
    await themesDuplicate('dark', 'dracula');
    expect(mockInvoke).toHaveBeenCalledWith('themes_duplicate', { source: 'dark', newName: 'dracula' });
  });

  it('returns the new ThemeMeta', async () => {
    mockInvoke.mockResolvedValue(stubMeta);
    const result = await themesDuplicate('dark', 'dracula');
    expect(result).toBe(stubMeta);
  });
});
