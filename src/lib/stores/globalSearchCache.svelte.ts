import type { ConnectionProfile, ColumnInfo, DbType } from '$lib/types';
import { listDatabases, listTables, listColumns } from '$lib/tauri/schema';

export type DbEntry = {
  connectionId: string;
  connectionName: string;
  connectionColor: string | null;
  connectionDbType: DbType;
  connectionReadOnly: boolean;
  connectionGroupId: string | null;
  database: string;
};

export type TableEntry = {
  connectionId: string;
  connectionName: string;
  connectionColor: string | null;
  connectionDbType: DbType;
  connectionReadOnly: boolean;
  connectionGroupId: string | null;
  database: string;
  name: string;
  tableType: 'table' | 'view';
};

export type ColumnEntry = {
  connectionId: string;
  connectionName: string;
  connectionColor: string | null;
  connectionDbType: DbType;
  connectionReadOnly: boolean;
  connectionGroupId: string | null;
  database: string;
  table: string;
  name: string;
  dataType: string;
  isPrimaryKey: boolean;
};

let databaseEntries = $state<DbEntry[]>([]);
let tableEntries = $state<TableEntry[]>([]);
let columnEntries = $state<ColumnEntry[]>([]);
let loadedConnectionIds = $state(new Set<string>());
let loadingConnectionIds = $state(new Set<string>());

export function useGlobalSearchCache() {
  return {
    get databaseEntries() {
      return databaseEntries;
    },
    get tableEntries() {
      return tableEntries;
    },
    get columnEntries() {
      return columnEntries;
    },
    get isLoading() {
      return loadingConnectionIds.size > 0;
    },

    async populate(profiles: ConnectionProfile[]) {
      const toLoad = profiles.filter(
        (p) => !loadedConnectionIds.has(p.id) && !loadingConnectionIds.has(p.id),
      );
      if (toLoad.length === 0) return;

      loadingConnectionIds = new Set([...loadingConnectionIds, ...toLoad.map((p) => p.id)]);

      const dbResults = await Promise.allSettled(
        toLoad.map(async (profile) => {
          const dbs = await listDatabases(profile.id);
          return { profile, dbs };
        }),
      );

      const newDbs: DbEntry[] = [];
      for (const result of dbResults) {
        if (result.status !== 'fulfilled') continue;
        const { profile, dbs } = result.value;
        for (const db of dbs) {
          newDbs.push({
            connectionId: profile.id,
            connectionName: profile.name,
            connectionColor: profile.color,
            connectionDbType: profile.dbType,
            connectionReadOnly: profile.readOnly,
            connectionGroupId: profile.groupId,
            database: db,
          });
        }
      }
      databaseEntries = [...databaseEntries, ...newDbs];

      const tableResults = await Promise.allSettled(
        newDbs.map(async (entry) => {
          const tables = await listTables(entry.connectionId, entry.database);
          return { entry, tables };
        }),
      );

      const newTables: TableEntry[] = [];
      for (const result of tableResults) {
        if (result.status !== 'fulfilled') continue;
        const { entry, tables } = result.value;
        for (const t of tables) {
          newTables.push({
            connectionId: entry.connectionId,
            connectionName: entry.connectionName,
            connectionColor: entry.connectionColor,
            connectionDbType: entry.connectionDbType,
            connectionReadOnly: entry.connectionReadOnly,
            connectionGroupId: entry.connectionGroupId,
            database: entry.database,
            name: t.name,
            tableType: t.tableType,
          });
        }
      }
      tableEntries = [...tableEntries, ...newTables];

      await Promise.allSettled(
        newTables.map(async (entry) => {
          try {
            const cols = await listColumns(entry.connectionId, entry.database, entry.name);
            const newCols: ColumnEntry[] = cols.map((c: ColumnInfo) => ({
              connectionId: entry.connectionId,
              connectionName: entry.connectionName,
              connectionColor: entry.connectionColor,
              connectionDbType: entry.connectionDbType,
              connectionReadOnly: entry.connectionReadOnly,
              connectionGroupId: entry.connectionGroupId,
              database: entry.database,
              table: entry.name,
              name: c.name,
              dataType: c.dataType,
              isPrimaryKey: c.isPrimaryKey,
            }));
            columnEntries = [...columnEntries, ...newCols];
          } catch {
            // ignore
          }
        }),
      );

      const loadedNow = new Set(toLoad.map((p) => p.id));
      loadedConnectionIds = new Set([...loadedConnectionIds, ...loadedNow]);
      loadingConnectionIds = new Set([...loadingConnectionIds].filter((id) => !loadedNow.has(id)));
    },

    invalidate(connectionId: string) {
      databaseEntries = databaseEntries.filter((e) => e.connectionId !== connectionId);
      tableEntries = tableEntries.filter((e) => e.connectionId !== connectionId);
      columnEntries = columnEntries.filter((e) => e.connectionId !== connectionId);
      loadedConnectionIds = new Set([...loadedConnectionIds].filter((id) => id !== connectionId));
      loadingConnectionIds = new Set([...loadingConnectionIds].filter((id) => id !== connectionId));
    },
  };
}
