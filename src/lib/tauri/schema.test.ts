import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import {
  listDatabases,
  listTables,
  listColumns,
  listIndexes,
  listForeignKeys,
  listCheckConstraints,
  listTriggers,
  listViews,
  listAllIndexes,
  listAllForeignKeys,
  getDdl,
} from './schema';

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
      instanceDb: null,
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
      instanceDb: null,
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
      instanceDb: null,
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
      instanceDb: null,
      table: 'orders',
    });
  });
});

describe('listCheckConstraints', () => {
  it('invokes schema_list_check_constraints with connectionId, database, and table', async () => {
    mockInvoke.mockResolvedValue([]);
    await listCheckConstraints('conn-1', 'my_db', 'orders');
    expect(mockInvoke).toHaveBeenCalledWith('schema_list_check_constraints', {
      connectionId: 'conn-1',
      database: 'my_db',
      table: 'orders',
      instanceDb: null,
    });
  });

  it('passes null table when omitted (database-wide listing)', async () => {
    mockInvoke.mockResolvedValue([]);
    await listCheckConstraints('conn-1', 'my_db');
    expect(mockInvoke).toHaveBeenCalledWith('schema_list_check_constraints', {
      connectionId: 'conn-1',
      database: 'my_db',
      table: null,
      instanceDb: null,
    });
  });

  it('passes instanceDb when provided', async () => {
    mockInvoke.mockResolvedValue([]);
    await listCheckConstraints('conn-1', 'my_db', 'orders', 'instance_db');
    expect(mockInvoke).toHaveBeenCalledWith('schema_list_check_constraints', {
      connectionId: 'conn-1',
      database: 'my_db',
      table: 'orders',
      instanceDb: 'instance_db',
    });
  });
});

describe('listTriggers', () => {
  it('invokes schema_list_triggers with connectionId, database, and table', async () => {
    mockInvoke.mockResolvedValue([]);
    await listTriggers('conn-1', 'my_db', 'orders');
    expect(mockInvoke).toHaveBeenCalledWith('schema_list_triggers', {
      connectionId: 'conn-1',
      database: 'my_db',
      table: 'orders',
      instanceDb: null,
    });
  });

  it('passes null table when omitted (database-wide listing)', async () => {
    mockInvoke.mockResolvedValue([]);
    await listTriggers('conn-1', 'my_db');
    expect(mockInvoke).toHaveBeenCalledWith('schema_list_triggers', {
      connectionId: 'conn-1',
      database: 'my_db',
      table: null,
      instanceDb: null,
    });
  });

  it('passes instanceDb when provided', async () => {
    mockInvoke.mockResolvedValue([]);
    await listTriggers('conn-1', 'my_db', 'orders', 'instance_db');
    expect(mockInvoke).toHaveBeenCalledWith('schema_list_triggers', {
      connectionId: 'conn-1',
      database: 'my_db',
      table: 'orders',
      instanceDb: 'instance_db',
    });
  });
});

describe('listViews', () => {
  it('invokes schema_list_views with connectionId and database', async () => {
    mockInvoke.mockResolvedValue([]);
    await listViews('conn-1', 'my_db');
    expect(mockInvoke).toHaveBeenCalledWith('schema_list_views', {
      connectionId: 'conn-1',
      database: 'my_db',
      instanceDb: null,
    });
  });

  it('passes instanceDb when provided', async () => {
    mockInvoke.mockResolvedValue([]);
    await listViews('conn-1', 'my_db', 'instance_db');
    expect(mockInvoke).toHaveBeenCalledWith('schema_list_views', {
      connectionId: 'conn-1',
      database: 'my_db',
      instanceDb: 'instance_db',
    });
  });
});

describe('listAllIndexes', () => {
  it('invokes schema_list_all_indexes with connectionId and database', async () => {
    mockInvoke.mockResolvedValue([]);
    await listAllIndexes('conn-1', 'my_db');
    expect(mockInvoke).toHaveBeenCalledWith('schema_list_all_indexes', {
      connectionId: 'conn-1',
      database: 'my_db',
      instanceDb: null,
    });
  });

  it('passes instanceDb when provided', async () => {
    mockInvoke.mockResolvedValue([]);
    await listAllIndexes('conn-1', 'my_db', 'instance_db');
    expect(mockInvoke).toHaveBeenCalledWith('schema_list_all_indexes', {
      connectionId: 'conn-1',
      database: 'my_db',
      instanceDb: 'instance_db',
    });
  });
});

describe('listAllForeignKeys', () => {
  it('invokes schema_list_all_foreign_keys with connectionId and database', async () => {
    mockInvoke.mockResolvedValue([]);
    await listAllForeignKeys('conn-1', 'my_db');
    expect(mockInvoke).toHaveBeenCalledWith('schema_list_all_foreign_keys', {
      connectionId: 'conn-1',
      database: 'my_db',
      instanceDb: null,
    });
  });

  it('passes instanceDb when provided', async () => {
    mockInvoke.mockResolvedValue([]);
    await listAllForeignKeys('conn-1', 'my_db', 'instance_db');
    expect(mockInvoke).toHaveBeenCalledWith('schema_list_all_foreign_keys', {
      connectionId: 'conn-1',
      database: 'my_db',
      instanceDb: 'instance_db',
    });
  });
});

describe('getDdl', () => {
  it('invokes schema_get_ddl', async () => {
    mockInvoke.mockResolvedValue('CREATE TABLE ...');
    const result = await getDdl('conn-1', 'my_db', 'users');
    expect(mockInvoke).toHaveBeenCalledWith('schema_get_ddl', {
      connectionId: 'conn-1',
      database: 'my_db',
      objectName: 'users',
      instanceDb: null,
    });
    expect(result).toBe('CREATE TABLE ...');
  });

  it('passes instanceDb when provided', async () => {
    mockInvoke.mockResolvedValue('CREATE VIEW ...');
    await getDdl('conn-1', 'my_db', 'active_users', 'instance_db');
    expect(mockInvoke).toHaveBeenCalledWith(
      'schema_get_ddl',
      expect.objectContaining({ instanceDb: 'instance_db' }),
    );
  });
});
