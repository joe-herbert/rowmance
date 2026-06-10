import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { listHistory, clearHistory, deleteHistoryEntry } from './history';

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  mockInvoke.mockReset();
});

describe('listHistory', () => {
  it('invokes history_list with connectionId and default limit/offset', async () => {
    mockInvoke.mockResolvedValue([]);
    await listHistory('conn-1');
    expect(mockInvoke).toHaveBeenCalledWith('history_list', {
      connectionId: 'conn-1',
      limit: 100,
      offset: 0,
    });
  });

  it('passes explicit limit and offset values', async () => {
    mockInvoke.mockResolvedValue([]);
    await listHistory('conn-1', 25, 50);
    expect(mockInvoke).toHaveBeenCalledWith('history_list', {
      connectionId: 'conn-1',
      limit: 25,
      offset: 50,
    });
  });

  it('returns the entries from invoke', async () => {
    const entries = [{ id: 'h1', sql: 'SELECT 1' }];
    mockInvoke.mockResolvedValue(entries);
    const result = await listHistory('conn-1');
    expect(result).toBe(entries);
  });
});

describe('clearHistory', () => {
  it('invokes history_clear with connectionId', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await clearHistory('conn-2');
    expect(mockInvoke).toHaveBeenCalledWith('history_clear', { connectionId: 'conn-2' });
  });
});

describe('deleteHistoryEntry', () => {
  it('invokes history_delete with the id', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await deleteHistoryEntry('h-id');
    expect(mockInvoke).toHaveBeenCalledWith('history_delete', { id: 'h-id' });
  });
});
