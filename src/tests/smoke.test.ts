/**
 * End-to-end smoke test for the core user path:
 *   create a connection → connect → execute query → edit cell → save
 *
 * All Tauri invoke() calls are mocked — no real backend required.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { ConnectionProfile, ConnectionProfileInput, QueryResult } from '$lib/types';

// ── Shared test fixtures ──────────────────────────────────────────────────────

function makeProfile(overrides: Partial<ConnectionProfile> = {}): ConnectionProfile {
  return {
    id: 'smoke-conn-1',
    groupId: null,
    name: 'Smoke Test DB',
    dbType: 'postgres',
    host: 'localhost',
    port: 5432,
    database: 'smokedb',
    username: 'smokeuser',
    color: null,
    readOnly: false,
    sshEnabled: false,
    sshHost: null,
    sshPort: null,
    sshUser: null,
    sshAuthType: null,
    sshKeyPath: null,
    sslEnabled: false,
    sslCaPath: null,
    sslCertPath: null,
    sslKeyPath: null,
    poolMax: 5,
    pingInterval: null,
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:00:00Z',
    ...overrides,
  };
}

function makeQueryResult(overrides: Partial<QueryResult> = {}): QueryResult {
  return {
    queryId: 'qr-1',
    columns: [
      { name: 'id', dataType: 'INT', nullable: false, isPrimaryKey: true, isForeignKey: false },
      {
        name: 'email',
        dataType: 'VARCHAR(255)',
        nullable: false,
        isPrimaryKey: false,
        isForeignKey: false,
      },
    ],
    rows: [
      [1, 'alice@example.com'],
      [2, 'bob@example.com'],
    ],
    totalRows: 2,
    durationUs: 12,
    affectedRows: null,
    error: null,
    ...overrides,
  };
}

const connectionInput: ConnectionProfileInput = {
  name: 'Smoke Test DB',
  dbType: 'postgres',
  host: 'localhost',
  port: 5432,
  database: 'smokedb',
  username: 'smokeuser',
};

// ── Mock setup (reset per test so module-level $state is fresh) ───────────────

let mockConnectionsApi: {
  listConnections: ReturnType<typeof vi.fn>;
  listConnectionGroups: ReturnType<typeof vi.fn>;
  listActiveConnections: ReturnType<typeof vi.fn>;
  createConnection: ReturnType<typeof vi.fn>;
  updateConnection: ReturnType<typeof vi.fn>;
  deleteConnection: ReturnType<typeof vi.fn>;
  connectToDatabase: ReturnType<typeof vi.fn>;
  disconnectFromDatabase: ReturnType<typeof vi.fn>;
};

let mockQueryApi: {
  executeQuery: ReturnType<typeof vi.fn>;
  updateRows: ReturnType<typeof vi.fn>;
};

let useConnections: typeof import('$lib/stores/connections.svelte').useConnections;

beforeEach(async () => {
  vi.resetModules();

  const profile = makeProfile();

  mockConnectionsApi = {
    listConnections: vi.fn().mockResolvedValue([profile]),
    listConnectionGroups: vi.fn().mockResolvedValue([]),
    listActiveConnections: vi.fn().mockResolvedValue([]),
    createConnection: vi.fn().mockResolvedValue(profile),
    updateConnection: vi.fn().mockResolvedValue(profile),
    deleteConnection: vi.fn().mockResolvedValue(undefined),
    connectToDatabase: vi.fn().mockResolvedValue(undefined),
    disconnectFromDatabase: vi.fn().mockResolvedValue(undefined),
  };

  mockQueryApi = {
    executeQuery: vi.fn().mockResolvedValue(makeQueryResult()),
    updateRows: vi.fn().mockResolvedValue({ updatedCount: 1 }),
  };

  vi.doMock('$lib/tauri/connections', () => mockConnectionsApi);
  vi.doMock('$lib/tauri/query', () => mockQueryApi);

  ({ useConnections } = await import('$lib/stores/connections.svelte'));
});

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('Smoke test: core path — connect → query → edit → save', () => {
  it('creates a connection profile and persists it in the store', async () => {
    const store = useConnections();
    const created = await store.create(connectionInput);

    expect(mockConnectionsApi.createConnection).toHaveBeenCalledWith(connectionInput);
    expect(created.name).toBe('Smoke Test DB');
    expect(store.profiles).toHaveLength(1);
    expect(store.profiles[0].id).toBe('smoke-conn-1');
  });

  it('connects to the database and marks it active', async () => {
    const store = useConnections();
    await store.create(connectionInput);

    await store.connect('smoke-conn-1');

    expect(mockConnectionsApi.connectToDatabase).toHaveBeenCalledWith('smoke-conn-1');
    expect(store.isActive('smoke-conn-1')).toBe(true);
    expect(store.connectingIds.has('smoke-conn-1')).toBe(false);
  });

  it('executes a query and returns rows with columns', async () => {
    const { executeQuery } = await import('$lib/tauri/query');

    const result = await executeQuery('smoke-conn-1', 'SELECT * FROM users', 0, 50);

    expect(mockQueryApi.executeQuery).toHaveBeenCalledWith(
      'smoke-conn-1',
      'SELECT * FROM users',
      0,
      50,
    );
    expect(result.error).toBeNull();
    expect(result.columns).toHaveLength(2);
    expect(result.rows).toHaveLength(2);
    expect(result.rows[0]).toEqual([1, 'alice@example.com']);
  });

  it('saves an edited cell via updateRows', async () => {
    const { updateRows } = await import('$lib/tauri/query');

    const change = {
      primaryKeys: { id: 1 },
      changes: { email: 'alice-updated@example.com' },
    };

    const result = await updateRows('smoke-conn-1', 'smokedb', 'users', [change]);

    expect(mockQueryApi.updateRows).toHaveBeenCalledWith('smoke-conn-1', 'smokedb', 'users', [
      change,
    ]);
    expect(result.updatedCount).toBe(1);
  });

  it('surfaces a connection error and records it in errorIds', async () => {
    mockConnectionsApi.connectToDatabase.mockRejectedValue(new Error('Connection refused'));

    const store = useConnections();
    await store.create(connectionInput);

    await expect(store.connect('smoke-conn-1')).rejects.toThrow('Connection refused');

    expect(store.isActive('smoke-conn-1')).toBe(false);
    expect(store.errorIds.get('smoke-conn-1')).toBe('Connection refused');
  });

  it('surfaces a query error when the backend returns one', async () => {
    mockQueryApi.executeQuery.mockResolvedValue(
      makeQueryResult({ error: 'relation "users" does not exist', rows: [], columns: [] }),
    );

    const { executeQuery } = await import('$lib/tauri/query');
    const result = await executeQuery('smoke-conn-1', 'SELECT * FROM users', 0, 50);

    expect(result.error).toBe('relation "users" does not exist');
    expect(result.rows).toHaveLength(0);
  });

  it('full path: create → connect → query → edit → save without errors', async () => {
    const store = useConnections();

    // 1. Create
    await store.create(connectionInput);
    expect(store.profiles[0].id).toBe('smoke-conn-1');

    // 2. Connect
    await store.connect('smoke-conn-1');
    expect(store.isActive('smoke-conn-1')).toBe(true);

    // 3. Query
    const { executeQuery, updateRows } = await import('$lib/tauri/query');
    const result = await executeQuery('smoke-conn-1', 'SELECT * FROM users', 0, 50);
    expect(result.error).toBeNull();
    expect(result.rows).toHaveLength(2);

    // 4. Edit + Save
    const saved = await updateRows('smoke-conn-1', 'smokedb', 'users', [
      { primaryKeys: { id: 1 }, changes: { email: 'new@example.com' } },
    ]);
    expect(saved.updatedCount).toBe(1);
  });
});
