/** Tests for export.ts typed wrappers. */
import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/core';
import { exportResultToClipboard, exportResultToFile } from './export';

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  mockInvoke.mockReset();
});

describe('exportResultToClipboard', () => {
  it('calls export_result_to_clipboard with correct args', async () => {
    mockInvoke.mockResolvedValue(undefined);
    const rows = [[1, 'Alice']];
    const columns = ['id', 'name'];
    await exportResultToClipboard(rows, columns, 'csv');
    expect(mockInvoke).toHaveBeenCalledWith('export_result_to_clipboard', {
      rows,
      columns,
      format: 'csv',
      tableName: undefined,
    });
  });

  it('passes tableName when provided', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await exportResultToClipboard([], [], 'sql_insert', 'my_table');
    expect(mockInvoke).toHaveBeenCalledWith('export_result_to_clipboard', {
      rows: [],
      columns: [],
      format: 'sql_insert',
      tableName: 'my_table',
    });
  });

  it('calls with tab_separated format using correct invoke name and args', async () => {
    mockInvoke.mockResolvedValue(undefined);
    const rows = [
      ['a', 'b'],
      ['c', 'd'],
    ];
    const columns = ['col1', 'col2'];
    await exportResultToClipboard(rows, columns, 'tab_separated');
    expect(mockInvoke).toHaveBeenCalledWith('export_result_to_clipboard', {
      rows,
      columns,
      format: 'tab_separated',
      tableName: undefined,
    });
  });
});

describe('exportResultToFile', () => {
  it('calls export_result_to_file with correct args', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await exportResultToFile([[1]], ['id'], 'json', '/tmp/out.json');
    expect(mockInvoke).toHaveBeenCalledWith('export_result_to_file', {
      rows: [[1]],
      columns: ['id'],
      format: 'json',
      filePath: '/tmp/out.json',
      tableName: undefined,
    });
  });

  it('calls with csv format using correct invoke name and args', async () => {
    mockInvoke.mockResolvedValue(undefined);
    const rows = [
      [1, 'Alice'],
      [2, 'Bob'],
    ];
    const columns = ['id', 'name'];
    await exportResultToFile(rows, columns, 'csv', '/tmp/data.csv');
    expect(mockInvoke).toHaveBeenCalledWith('export_result_to_file', {
      rows,
      columns,
      format: 'csv',
      filePath: '/tmp/data.csv',
      tableName: undefined,
    });
  });

  it('includes table_name in args for sql_insert format', async () => {
    mockInvoke.mockResolvedValue(undefined);
    const rows = [[1, 'Alice']];
    const columns = ['id', 'name'];
    await exportResultToFile(rows, columns, 'sql_insert', '/tmp/inserts.sql', 'users');
    expect(mockInvoke).toHaveBeenCalledWith('export_result_to_file', {
      rows,
      columns,
      format: 'sql_insert',
      filePath: '/tmp/inserts.sql',
      tableName: 'users',
    });
  });

  it('passes tab_separated format with correct invoke args', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await exportResultToFile([[1, 'x']], ['id', 'val'], 'tab_separated', '/tmp/out.tsv');
    expect(mockInvoke).toHaveBeenCalledWith('export_result_to_file', {
      rows: [[1, 'x']],
      columns: ['id', 'val'],
      format: 'tab_separated',
      filePath: '/tmp/out.tsv',
      tableName: undefined,
    });
  });
});
