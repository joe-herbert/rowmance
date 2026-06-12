import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { getErdGraph } from './erd';

const mockInvoke = vi.mocked(invoke);

const stubGraph = {
  nodes: [
    { name: 'users', columns: [{ name: 'id', dataType: 'int', isPrimaryKey: true }] },
  ],
  edges: [],
};

beforeEach(() => {
  mockInvoke.mockReset();
});

describe('getErdGraph', () => {
  it('invokes erd_get_graph with connectionId and database', async () => {
    mockInvoke.mockResolvedValue(stubGraph);
    await getErdGraph('conn-1', 'mydb');
    expect(mockInvoke).toHaveBeenCalledWith('erd_get_graph', {
      connectionId: 'conn-1',
      database: 'mydb',
    });
  });

  it('returns the ErdGraph from invoke', async () => {
    mockInvoke.mockResolvedValue(stubGraph);
    const result = await getErdGraph('conn-1', 'mydb');
    expect(result).toBe(stubGraph);
  });

  it('propagates invoke errors', async () => {
    mockInvoke.mockRejectedValue({ code: 'CONNECTION_NOT_FOUND', message: 'Not found', detail: null });
    await expect(getErdGraph('bad', 'db')).rejects.toMatchObject({ code: 'CONNECTION_NOT_FOUND' });
  });
});
