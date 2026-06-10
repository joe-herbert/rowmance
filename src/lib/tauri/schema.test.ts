import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { listDatabases, listTables, listColumns, listIndexes, listForeignKeys, getDdl } from './schema';

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  mockInvoke.mockReset();
});

describe('listDatabases', () => {
  it('invokes schema_list_databases with connectionId', async () => {
    mockInvoke.mockResolvedValue(['db1', 'db2']);
    const result = await listDatabases('conn-1');
    expect(mockInvoke).toHaveBeenCalledWith('schema_list_databases', { connectionId: 'conn-1' });
    expect(result).toEqual(['db1', 'db2']);
  });
});

describe('listTables', () => {
  it('invokes schema_list_tables with connectionId and database', async () => {
    mockInvoke.mockResolvedValue([]);
    await listTables('conn-1', 'my_db');
    expect(mockInvoke).toHaveBeenCalledWith('schema_list_tables', {
      connectionId: 'conn-1',
      database: 'my_db',
    });
  });
});

describe('listColumns', () => {
  it('invokes schema_list_columns with connectionId, database, and table', async () => {
    mockInvoke.mockResolvedValue([]);
    await listColumns('conn-1', 'my_db', 'users');
    expect(mockInvoke).toHaveBeenCalledWith('schema_list_columns', {
      connectionId: 'conn-1',
      database: 'my_db',
      table: 'users',
    });
  });
});

describe('listIndexes', () => {
  it('invokes schema_list_indexes with connectionId, database, and table', async () => {
    mockInvoke.mockResolvedValue([]);
    await listIndexes('conn-1', 'my_db', 'users');
    expect(mockInvoke).toHaveBeenCalledWith('schema_list_indexes', {
      connectionId: 'conn-1',
      database: 'my_db',
      table: 'users',
    });
  });
});

describe('listForeignKeys', () => {
  it('invokes schema_list_foreign_keys with connectionId, database, and table', async () => {
    mockInvoke.mockResolvedValue([]);
    await listForeignKeys('conn-1', 'my_db', 'orders');
    expect(mockInvoke).toHaveBeenCalledWith('schema_list_foreign_keys', {
      connectionId: 'conn-1',
      database: 'my_db',
      table: 'orders',
    });
  });
});

describe('getDdl', () => {
  it('invokes schema_get_ddl with all four arguments', async () => {
    mockInvoke.mockResolvedValue('CREATE TABLE ...');
    const result = await getDdl('conn-1', 'my_db', 'users', 'table');
    expect(mockInvoke).toHaveBeenCalledWith('schema_get_ddl', {
      connectionId: 'conn-1',
      database: 'my_db',
      objectName: 'users',
      objectType: 'table',
    });
    expect(result).toBe('CREATE TABLE ...');
  });

  it('accepts "view" as objectType', async () => {
    mockInvoke.mockResolvedValue('CREATE VIEW ...');
    await getDdl('conn-1', 'my_db', 'active_users', 'view');
    expect(mockInvoke).toHaveBeenCalledWith('schema_get_ddl', expect.objectContaining({ objectType: 'view' }));
  });
});
