import type { ConnectionProfile, DbType } from '$lib/types';
import { listDatabases, listTables, listAllColumns } from '$lib/tauri/schema';

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

      // Each profile is pipelined independently: databases → tables+columns per db.
      // This avoids waiting for a slow connection before starting faster ones.
      await Promise.allSettled(
        toLoad.map(async (profile) => {
          let dbs: string[];
          try {
            dbs = await listDatabases(profile.id);
          } catch {
            return;
          }

          const newDbs: DbEntry[] = dbs.map((db) => ({
            connectionId: profile.id,
            connectionName: profile.name,
            connectionColor: profile.color,
            connectionDbType: profile.dbType,
            connectionReadOnly: profile.readOnly,
            connectionGroupId: profile.groupId,
            database: db,
          }));
          databaseEntries = [...databaseEntries, ...newDbs];

          // For each database, fetch tables and columns in parallel.
          // Tables appear immediately; columns follow in the same round-trip pair.
          await Promise.allSettled(
            newDbs.map(async (dbEntry) => {
              const [tableResult, colResult] = await Promise.allSettled([
                listTables(dbEntry.connectionId, dbEntry.database),
                listAllColumns(dbEntry.connectionId, dbEntry.database),
              ]);

              if (tableResult.status === 'fulfilled') {
                const newTables: TableEntry[] = tableResult.value.map((t) => ({
                  connectionId: dbEntry.connectionId,
                  connectionName: dbEntry.connectionName,
                  connectionColor: dbEntry.connectionColor,
                  connectionDbType: dbEntry.connectionDbType,
                  connectionReadOnly: dbEntry.connectionReadOnly,
                  connectionGroupId: dbEntry.connectionGroupId,
                  database: dbEntry.database,
                  name: t.name,
                  tableType: t.tableType,
                }));
                tableEntries = [...tableEntries, ...newTables];
              }

              if (colResult.status === 'fulfilled') {
                const newCols: ColumnEntry[] = colResult.value.map((c) => ({
                  connectionId: dbEntry.connectionId,
                  connectionName: dbEntry.connectionName,
                  connectionColor: dbEntry.connectionColor,
                  connectionDbType: dbEntry.connectionDbType,
                  connectionReadOnly: dbEntry.connectionReadOnly,
                  connectionGroupId: dbEntry.connectionGroupId,
                  database: dbEntry.database,
                  table: c.tableName,
                  name: c.name,
                  dataType: c.dataType,
                  isPrimaryKey: c.isPrimaryKey,
                }));
                columnEntries = [...columnEntries, ...newCols];
              }
            }),
          );
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
