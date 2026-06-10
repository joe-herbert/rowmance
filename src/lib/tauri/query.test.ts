import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { executeQuery, executeSelection, cancelQuery } from './query';

const mockInvoke = vi.mocked(invoke);

const stubResult = {
  queryId: 'q-1',
  columns: [],
  rows: [],
  totalRows: null,
  durationMs: 10,
  affectedRows: null,
  error: null,
};

beforeEach(() => {
  mockInvoke.mockReset();
});

describe('executeQuery', () => {
  it('invokes query_execute with all four arguments', async () => {
    mockInvoke.mockResolvedValue(stubResult);
    await executeQuery('conn-1', 'SELECT 1', 1, 50);
    expect(mockInvoke).toHaveBeenCalledWith('query_execute', {
      connectionId: 'conn-1',
      sql: 'SELECT 1',
      page: 1,
      pageSize: 50,
    });
  });

  it('returns the QueryResult from invoke', async () => {
    mockInvoke.mockResolvedValue(stubResult);
    const result = await executeQuery('c', 'SELECT 1', 1, 50);
    expect(result).toBe(stubResult);
  });
});

describe('executeSelection', () => {
  it('invokes query_execute_selection with connectionId and sql', async () => {
    mockInvoke.mockResolvedValue(stubResult);
    await executeSelection('conn-2', 'SELECT 2');
    expect(mockInvoke).toHaveBeenCalledWith('query_execute_selection', {
      connectionId: 'conn-2',
      sql: 'SELECT 2',
    });
  });
});

describe('cancelQuery', () => {
  it('invokes query_cancel with the queryId', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await cancelQuery('q-id');
    expect(mockInvoke).toHaveBeenCalledWith('query_cancel', { queryId: 'q-id' });
  });
});
