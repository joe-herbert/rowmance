/** Tests for import.ts typed wrappers. */
import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/core';
import { importCsvPreview, importCsvExecute, importSqlFile } from './import';

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  mockInvoke.mockReset();
});

describe('importCsvPreview', () => {
  it('calls import_csv_preview with filePath', async () => {
    const preview = { columns: [], previewRows: [] };
    mockInvoke.mockResolvedValue(preview);
    const result = await importCsvPreview('/tmp/data.csv');
    expect(mockInvoke).toHaveBeenCalledWith('import_csv_preview', { filePath: '/tmp/data.csv' });
    expect(result).toBe(preview);
  });
});

describe('importCsvExecute', () => {
  it('calls import_csv_execute with all args', async () => {
    mockInvoke.mockResolvedValue(42);
    const overrides = [{ name: 'id', dbType: 'INTEGER' }];
    const count = await importCsvExecute('conn-1', '/tmp/data.csv', 'users', true, overrides);
    expect(count).toBe(42);
    expect(mockInvoke).toHaveBeenCalledWith('import_csv_execute', {
      connectionId: 'conn-1',
      filePath: '/tmp/data.csv',
      tableName: 'users',
      createTable: true,
      columnOverrides: overrides,
    });
  });
});

describe('importSqlFile', () => {
  it('calls import_sql_file with correct args', async () => {
    mockInvoke.mockResolvedValue(10);
    const count = await importSqlFile('conn-1', '/tmp/schema.sql', false);
    expect(count).toBe(10);
    expect(mockInvoke).toHaveBeenCalledWith('import_sql_file', {
      connectionId: 'conn-1',
      filePath: '/tmp/schema.sql',
      stopOnError: false,
    });
  });
});
