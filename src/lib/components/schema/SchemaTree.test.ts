import { describe, it, expect } from 'vitest';

// Pure function redefined locally to avoid rune compilation issues in test context.
// Mirrors buildSearchItems from SchemaTree.svelte.

interface SearchItem {
  connectionId: string;
  database: string;
  name: string;
  label: string;
}

interface Profile {
  id: string;
  name: string;
}

interface TableInfo {
  name: string;
  tableType: 'table' | 'view';
  rowCount: number | null;
}

function buildSearchItems(
  profiles: Profile[],
  cache: Map<string, Map<string, TableInfo[]>>,
): SearchItem[] {
  const items: SearchItem[] = [];
  for (const profile of profiles) {
    const dbMap = cache.get(profile.id);
    if (!dbMap) continue;
    for (const [database, tables] of dbMap.entries()) {
      for (const table of tables) {
        items.push({
          connectionId: profile.id,
          database,
          name: table.name,
          label: `${profile.name}.${database}.${table.name}`,
        });
      }
    }
  }
  return items;
}

describe('buildSearchItems', () => {
  it('returns empty array when profiles is empty', () => {
    const result = buildSearchItems([], new Map());
    expect(result).toEqual([]);
  });

  it('returns empty array when profile has no cache entry', () => {
    const profiles: Profile[] = [{ id: 'conn-1', name: 'MyDB' }];
    const cache = new Map<string, Map<string, TableInfo[]>>();
    const result = buildSearchItems(profiles, cache);
    expect(result).toEqual([]);
  });

  it('returns one item for single profile, single database, single table', () => {
    const profiles: Profile[] = [{ id: 'conn-1', name: 'Prod' }];
    const tableMap = new Map<string, TableInfo[]>();
    tableMap.set('mydb', [{ name: 'users', tableType: 'table', rowCount: 100 }]);
    const cache = new Map<string, Map<string, TableInfo[]>>();
    cache.set('conn-1', tableMap);

    const result = buildSearchItems(profiles, cache);

    expect(result).toHaveLength(1);
    expect(result[0]).toEqual({
      connectionId: 'conn-1',
      database: 'mydb',
      name: 'users',
      label: 'Prod.mydb.users',
    });
  });

  it('returns multiple items for multiple tables in one database', () => {
    const profiles: Profile[] = [{ id: 'conn-1', name: 'Prod' }];
    const tableMap = new Map<string, TableInfo[]>();
    tableMap.set('mydb', [
      { name: 'users', tableType: 'table', rowCount: 100 },
      { name: 'orders', tableType: 'table', rowCount: 200 },
      { name: 'products', tableType: 'view', rowCount: null },
    ]);
    const cache = new Map<string, Map<string, TableInfo[]>>();
    cache.set('conn-1', tableMap);

    const result = buildSearchItems(profiles, cache);

    expect(result).toHaveLength(3);
    expect(result.map((r) => r.name)).toEqual(['users', 'orders', 'products']);
  });

  it('returns items from all databases when multiple databases exist', () => {
    const profiles: Profile[] = [{ id: 'conn-1', name: 'Prod' }];
    const tableMap = new Map<string, TableInfo[]>();
    tableMap.set('db1', [{ name: 'tableA', tableType: 'table', rowCount: null }]);
    tableMap.set('db2', [{ name: 'tableB', tableType: 'table', rowCount: null }]);
    const cache = new Map<string, Map<string, TableInfo[]>>();
    cache.set('conn-1', tableMap);

    const result = buildSearchItems(profiles, cache);

    expect(result).toHaveLength(2);
    const databases = result.map((r) => r.database);
    expect(databases).toContain('db1');
    expect(databases).toContain('db2');
  });

  it('returns items from all profiles when multiple profiles exist', () => {
    const profiles: Profile[] = [
      { id: 'conn-1', name: 'Prod' },
      { id: 'conn-2', name: 'Staging' },
    ];
    const tableMap1 = new Map<string, TableInfo[]>();
    tableMap1.set('db1', [{ name: 'users', tableType: 'table', rowCount: null }]);
    const tableMap2 = new Map<string, TableInfo[]>();
    tableMap2.set('db2', [{ name: 'accounts', tableType: 'table', rowCount: null }]);
    const cache = new Map<string, Map<string, TableInfo[]>>();
    cache.set('conn-1', tableMap1);
    cache.set('conn-2', tableMap2);

    const result = buildSearchItems(profiles, cache);

    expect(result).toHaveLength(2);
    const connIds = result.map((r) => r.connectionId);
    expect(connIds).toContain('conn-1');
    expect(connIds).toContain('conn-2');
  });

  it('skips profiles that have no cache entry while including those that do', () => {
    const profiles: Profile[] = [
      { id: 'conn-1', name: 'Prod' },
      { id: 'conn-2', name: 'Staging' }, // no cache entry
    ];
    const tableMap1 = new Map<string, TableInfo[]>();
    tableMap1.set('mydb', [{ name: 'users', tableType: 'table', rowCount: null }]);
    const cache = new Map<string, Map<string, TableInfo[]>>();
    cache.set('conn-1', tableMap1);

    const result = buildSearchItems(profiles, cache);

    expect(result).toHaveLength(1);
    expect(result[0].connectionId).toBe('conn-1');
  });

  it('formats label as profileName.database.tableName', () => {
    const profiles: Profile[] = [{ id: 'conn-1', name: 'MyProfile' }];
    const tableMap = new Map<string, TableInfo[]>();
    tableMap.set('myDatabase', [{ name: 'myTable', tableType: 'table', rowCount: null }]);
    const cache = new Map<string, Map<string, TableInfo[]>>();
    cache.set('conn-1', tableMap);

    const result = buildSearchItems(profiles, cache);

    expect(result[0].label).toBe('MyProfile.myDatabase.myTable');
  });
});
