import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import {
  listConnections,
  createConnection,
  updateConnection,
  deleteConnection,
  testConnection,
  connectToDatabase,
  disconnectFromDatabase,
  listActiveConnections,
  listConnectionGroups,
  createConnectionGroup,
  deleteConnectionGroup,
} from './connections';

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  mockInvoke.mockReset();
});

describe('listConnections', () => {
  it('invokes connections_list with no arguments', async () => {
    mockInvoke.mockResolvedValue([]);
    await listConnections();
    expect(mockInvoke).toHaveBeenCalledWith('connections_list');
  });

  it('returns the value from invoke', async () => {
    const profiles = [{ id: 'abc', name: 'My DB' }];
    mockInvoke.mockResolvedValue(profiles);
    const result = await listConnections();
    expect(result).toBe(profiles);
  });
});

describe('createConnection', () => {
  it('invokes connections_create with the input wrapped in { input }', async () => {
    const input = {
      name: 'Test',
      dbType: 'postgres' as const,
      host: 'localhost',
      port: 5432,
      database: 'db',
      username: 'user',
    };
    mockInvoke.mockResolvedValue({ id: '1', ...input });
    await createConnection(input);
    expect(mockInvoke).toHaveBeenCalledWith('connections_create', { input });
  });
});

describe('updateConnection', () => {
  it('invokes connections_update with id and input', async () => {
    const input = {
      name: 'Updated',
      dbType: 'mysql' as const,
      host: 'localhost',
      port: 3306,
      database: 'db',
      username: 'root',
    };
    mockInvoke.mockResolvedValue({ id: 'x', ...input });
    await updateConnection('x', input);
    expect(mockInvoke).toHaveBeenCalledWith('connections_update', { id: 'x', input });
  });
});

describe('deleteConnection', () => {
  it('invokes connections_delete with the id', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await deleteConnection('del-id');
    expect(mockInvoke).toHaveBeenCalledWith('connections_delete', { id: 'del-id' });
  });
});

describe('testConnection', () => {
  it('invokes connections_test with the id', async () => {
    mockInvoke.mockResolvedValue({ success: true, message: 'ok', latencyMs: 5 });
    await testConnection('t-id');
    expect(mockInvoke).toHaveBeenCalledWith('connections_test', { id: 't-id' });
  });
});

describe('connectToDatabase', () => {
  it('invokes connections_connect with the id', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await connectToDatabase('c-id');
    expect(mockInvoke).toHaveBeenCalledWith('connections_connect', { id: 'c-id' });
  });
});

describe('disconnectFromDatabase', () => {
  it('invokes connections_disconnect with the id', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await disconnectFromDatabase('d-id');
    expect(mockInvoke).toHaveBeenCalledWith('connections_disconnect', { id: 'd-id' });
  });
});

describe('listActiveConnections', () => {
  it('invokes connections_list_active with no arguments', async () => {
    mockInvoke.mockResolvedValue(['id1', 'id2']);
    const result = await listActiveConnections();
    expect(mockInvoke).toHaveBeenCalledWith('connections_list_active');
    expect(result).toEqual(['id1', 'id2']);
  });
});

describe('listConnectionGroups', () => {
  it('invokes connection_groups_list with no arguments', async () => {
    mockInvoke.mockResolvedValue([]);
    await listConnectionGroups();
    expect(mockInvoke).toHaveBeenCalledWith('connection_groups_list');
  });
});

describe('createConnectionGroup', () => {
  it('invokes connection_groups_create with name and no parentId', async () => {
    mockInvoke.mockResolvedValue({ id: 'g1', name: 'Production', parentId: null, position: 0 });
    await createConnectionGroup('Production');
    expect(mockInvoke).toHaveBeenCalledWith('connection_groups_create', {
      input: { name: 'Production', parentId: undefined },
    });
  });

  it('invokes connection_groups_create with parentId when provided', async () => {
    mockInvoke.mockResolvedValue({ id: 'g2', name: 'Sub', parentId: 'parent-id', position: 0 });
    await createConnectionGroup('Sub', 'parent-id');
    expect(mockInvoke).toHaveBeenCalledWith('connection_groups_create', {
      input: { name: 'Sub', parentId: 'parent-id' },
    });
  });
});

describe('deleteConnectionGroup', () => {
  it('invokes connection_groups_delete with the id', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await deleteConnectionGroup('g-id');
    expect(mockInvoke).toHaveBeenCalledWith('connection_groups_delete', { id: 'g-id' });
  });
});
