import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { ConnectionProfile, ConnectionGroup, ConnectionProfileInput } from '$lib/types';

// Mocks are established fresh before each test via vi.doMock + vi.resetModules
// so that module-level $state is re-initialised and mock references stay current.

let useConnections: typeof import('./connections.svelte').useConnections;
let mockApi: {
  listConnections: ReturnType<typeof vi.fn>;
  listConnectionGroups: ReturnType<typeof vi.fn>;
  listActiveConnections: ReturnType<typeof vi.fn>;
  createConnection: ReturnType<typeof vi.fn>;
  updateConnection: ReturnType<typeof vi.fn>;
  deleteConnection: ReturnType<typeof vi.fn>;
  connectToDatabase: ReturnType<typeof vi.fn>;
  disconnectFromDatabase: ReturnType<typeof vi.fn>;
};

function makeProfile(overrides: Partial<ConnectionProfile> = {}): ConnectionProfile {
  return {
    id: 'p-1',
    groupId: null,
    name: 'Local Postgres',
    dbType: 'postgres',
    host: 'localhost',
    port: 5432,
    database: 'mydb',
    username: 'user',
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
    poolMin: 1,
    poolMax: 5,
    createdAt: '2024-01-01T00:00:00Z',
    updatedAt: '2024-01-01T00:00:00Z',
    ...overrides,
  };
}

function makeGroup(overrides: Partial<ConnectionGroup> = {}): ConnectionGroup {
  return { id: 'g-1', name: 'Production', parentId: null, position: 0, ...overrides };
}

const minimalInput: ConnectionProfileInput = {
  name: 'Test',
  dbType: 'postgres',
  host: 'localhost',
  port: 5432,
  database: 'db',
  username: 'user',
};

beforeEach(async () => {
  vi.resetModules();

  mockApi = {
    listConnections: vi.fn().mockResolvedValue([]),
    listConnectionGroups: vi.fn().mockResolvedValue([]),
    listActiveConnections: vi.fn().mockResolvedValue([]),
    createConnection: vi.fn(),
    updateConnection: vi.fn(),
    deleteConnection: vi.fn().mockResolvedValue(undefined),
    connectToDatabase: vi.fn().mockResolvedValue(undefined),
    disconnectFromDatabase: vi.fn().mockResolvedValue(undefined),
  };

  vi.doMock('$lib/tauri/connections', () => mockApi);
  ({ useConnections } = await import('./connections.svelte'));
});

describe('initial state', () => {
  it('starts with empty profiles, groups, and active ids', () => {
    const store = useConnections();
    expect(store.profiles).toHaveLength(0);
    expect(store.groups).toHaveLength(0);
    expect(store.activeIds.size).toBe(0);
  });
});

describe('load()', () => {
  it('populates profiles, groups, and activeIds from the backend', async () => {
    const profile = makeProfile();
    const group = makeGroup();
    mockApi.listConnections.mockResolvedValue([profile]);
    mockApi.listConnectionGroups.mockResolvedValue([group]);
    mockApi.listActiveConnections.mockResolvedValue(['p-1']);

    const store = useConnections();
    await store.load();

    expect(store.profiles).toEqual([profile]);
    expect(store.groups).toEqual([group]);
    expect(store.activeIds).toEqual(new Set(['p-1']));
  });

  it('leaves activeIds empty when no connections are active', async () => {
    const store = useConnections();
    await store.load();
    expect(store.activeIds.size).toBe(0);
  });
});

describe('create()', () => {
  it('appends the new profile to local state', async () => {
    const created = makeProfile({ id: 'new-1', name: 'New' });
    mockApi.createConnection.mockResolvedValue(created);

    const store = useConnections();
    const result = await store.create(minimalInput);

    expect(result).toBe(created);
    expect(store.profiles).toEqual([created]);
    expect(mockApi.createConnection).toHaveBeenCalledWith(minimalInput);
  });
});

describe('update()', () => {
  it('replaces the matching profile in local state', async () => {
    const original = makeProfile({ id: 'u-1', name: 'Original' });
    const updated = makeProfile({ id: 'u-1', name: 'Updated' });
    mockApi.listConnections.mockResolvedValue([original]);
    mockApi.listActiveConnections.mockResolvedValue([]);
    mockApi.updateConnection.mockResolvedValue(updated);

    const store = useConnections();
    await store.load();
    await store.update('u-1', minimalInput);

    expect(store.profiles).toEqual([updated]);
    expect(store.profiles[0].name).toBe('Updated');
  });
});

describe('delete()', () => {
  it('removes the profile from local state', async () => {
    const profile = makeProfile({ id: 'd-1' });
    mockApi.listConnections.mockResolvedValue([profile]);
    mockApi.listActiveConnections.mockResolvedValue([]);

    const store = useConnections();
    await store.load();
    await store.delete('d-1');

    expect(store.profiles).toHaveLength(0);
    expect(mockApi.deleteConnection).toHaveBeenCalledWith('d-1');
  });

  it('disconnects before deleting if the connection is active', async () => {
    const profile = makeProfile({ id: 'active-1' });
    mockApi.listConnections.mockResolvedValue([profile]);
    mockApi.listActiveConnections.mockResolvedValue(['active-1']);

    const store = useConnections();
    await store.load();
    await store.delete('active-1');

    expect(mockApi.disconnectFromDatabase).toHaveBeenCalledWith('active-1');
    expect(mockApi.deleteConnection).toHaveBeenCalledWith('active-1');
    expect(store.activeIds.has('active-1')).toBe(false);
  });
});

describe('connect()', () => {
  it('adds the id to activeIds on success', async () => {
    const store = useConnections();
    await store.connect('c-1');

    expect(store.activeIds.has('c-1')).toBe(true);
    expect(mockApi.connectToDatabase).toHaveBeenCalledWith('c-1');
  });

  it('removes the id from connectingIds after a successful connect', async () => {
    const store = useConnections();
    await store.connect('c-1');
    expect(store.connectingIds.has('c-1')).toBe(false);
  });

  it('records the error and does not add to activeIds when the backend throws', async () => {
    mockApi.connectToDatabase.mockRejectedValue(new Error('refused'));

    const store = useConnections();
    await expect(store.connect('fail-1')).rejects.toThrow('refused');

    expect(store.activeIds.has('fail-1')).toBe(false);
    expect(store.errorIds.get('fail-1')).toBe('refused');
    expect(store.connectingIds.has('fail-1')).toBe(false);
  });

  it('clears a previous error for the id before attempting', async () => {
    mockApi.connectToDatabase.mockRejectedValueOnce(new Error('refused'));
    const store = useConnections();
    await expect(store.connect('err-id')).rejects.toThrow();
    expect(store.errorIds.has('err-id')).toBe(true);

    mockApi.connectToDatabase.mockResolvedValue(undefined);
    await store.connect('err-id');
    expect(store.errorIds.has('err-id')).toBe(false);
  });
});

describe('disconnect()', () => {
  it('removes the id from activeIds', async () => {
    const store = useConnections();
    await store.connect('dc-1');
    expect(store.activeIds.has('dc-1')).toBe(true);

    await store.disconnect('dc-1');
    expect(store.activeIds.has('dc-1')).toBe(false);
    expect(mockApi.disconnectFromDatabase).toHaveBeenCalledWith('dc-1');
  });
});

describe('isActive()', () => {
  it('returns true for an active id and false otherwise', async () => {
    const store = useConnections();
    await store.connect('chk-1');
    expect(store.isActive('chk-1')).toBe(true);
    expect(store.isActive('other')).toBe(false);
  });
});

describe('getById()', () => {
  it('returns the matching profile', async () => {
    const profile = makeProfile({ id: 'find-1' });
    mockApi.listConnections.mockResolvedValue([profile]);
    mockApi.listActiveConnections.mockResolvedValue([]);

    const store = useConnections();
    await store.load();

    expect(store.getById('find-1')).toStrictEqual(profile);
  });

  it('returns undefined for an unknown id', async () => {
    const store = useConnections();
    await store.load();
    expect(store.getById('nope')).toBeUndefined();
  });
});
