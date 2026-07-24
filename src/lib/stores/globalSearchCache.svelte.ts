import type { ConnectionProfile, DbType } from '$lib/types';
import { listDatabases, listSchemas, listTables, listAllColumns } from '$lib/tauri/schema';

export type DbEntry = {
  connectionId: string;
  connectionName: string;
  connectionColor: string | null;
  connectionDbType: DbType;
  connectionReadOnly: boolean;
  connectionGroupId: string | null;
  database: string;
};

/** Schema within an instance database — only populated for engines where
 *  `dialectInfo.hasInstanceDatabases` is true (currently SQL Server). */
export type SchemaEntry = {
  connectionId: string;
  connectionName: string;
  connectionColor: string | null;
  connectionDbType: DbType;
  connectionReadOnly: boolean;
  connectionGroupId: string | null;
  instanceDb: string;
  schema: string;
};

export type TableEntry = {
  connectionId: string;
  connectionName: string;
  connectionColor: string | null;
  connectionDbType: DbType;
  connectionReadOnly: boolean;
  connectionGroupId: string | null;
  database: string;
  instanceDb?: string;
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
  instanceDb?: string;
  table: string;
  name: string;
  dataType: string;
  isPrimaryKey: boolean;
};

type CommonFields = Pick<
  DbEntry,
  | 'connectionId'
  | 'connectionName'
  | 'connectionColor'
  | 'connectionDbType'
  | 'connectionReadOnly'
  | 'connectionGroupId'
>;

let databaseEntries = $state<DbEntry[]>([]);
let schemaEntries = $state<SchemaEntry[]>([]);
let tableEntries = $state<TableEntry[]>([]);
let columnEntries = $state<ColumnEntry[]>([]);
let loadedConnectionIds = $state(new Set<string>());
let loadingConnectionIds = $state(new Set<string>());

async function loadTablesAndColumns(
  connectionId: string,
  database: string,
  instanceDb: string | undefined,
  common: CommonFields,
) {
  const [tableResult, colResult] = await Promise.allSettled([
    listTables(connectionId, database, instanceDb),
    listAllColumns(connectionId, database, instanceDb),
  ]);

  if (tableResult.status === 'fulfilled') {
    const newTables: TableEntry[] = tableResult.value.map((t) => ({
      ...common,
      database,
      instanceDb,
      name: t.name,
      tableType: t.tableType,
    }));
    tableEntries = [...tableEntries, ...newTables];
  }

  if (colResult.status === 'fulfilled') {
    const newCols: ColumnEntry[] = colResult.value.map((c) => ({
      ...common,
      database,
      instanceDb,
      table: c.tableName,
      name: c.name,
      dataType: c.dataType,
      isPrimaryKey: c.isPrimaryKey,
    }));
    columnEntries = [...columnEntries, ...newCols];
  }
}

export function useGlobalSearchCache() {
  return {
    get databaseEntries() {
      return databaseEntries;
    },
    get schemaEntries() {
      return schemaEntries;
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

          const common: CommonFields = {
            connectionId: profile.id,
            connectionName: profile.name,
            connectionColor: profile.color,
            connectionDbType: profile.dbType,
            connectionReadOnly: profile.readOnly,
            connectionGroupId: profile.groupId,
          };

          const newDbs: DbEntry[] = dbs.map((db) => ({ ...common, database: db }));
          databaseEntries = [...databaseEntries, ...newDbs];

          if (profile.dialectInfo.hasInstanceDatabases) {
            // `newDbs` are instance databases here — descend into their schemas,
            // then load tables/columns per schema (mirrors ConnectionTree's traversal).
            await Promise.allSettled(
              newDbs.map(async (dbEntry) => {
                let schemas: string[];
                try {
                  schemas = await listSchemas(profile.id, dbEntry.database);
                } catch {
                  return;
                }

                const newSchemas: SchemaEntry[] = schemas.map((schema) => ({
                  ...common,
                  instanceDb: dbEntry.database,
                  schema,
                }));
                schemaEntries = [...schemaEntries, ...newSchemas];

                await Promise.allSettled(
                  newSchemas.map((schemaEntry) =>
                    loadTablesAndColumns(
                      profile.id,
                      schemaEntry.schema,
                      schemaEntry.instanceDb,
                      common,
                    ),
                  ),
                );
              }),
            );
          } else {
            // Flat engines: `database` is the direct parent of tables (a real
            // database, or a Postgres schema — no separate instance-db level).
            await Promise.allSettled(
              newDbs.map((dbEntry) =>
                loadTablesAndColumns(profile.id, dbEntry.database, undefined, common),
              ),
            );
          }
        }),
      );

      const loadedNow = new Set(toLoad.map((p) => p.id));
      loadedConnectionIds = new Set([...loadedConnectionIds, ...loadedNow]);
      loadingConnectionIds = new Set([...loadingConnectionIds].filter((id) => !loadedNow.has(id)));
    },

    invalidate(connectionId: string) {
      databaseEntries = databaseEntries.filter((e) => e.connectionId !== connectionId);
      schemaEntries = schemaEntries.filter((e) => e.connectionId !== connectionId);
      tableEntries = tableEntries.filter((e) => e.connectionId !== connectionId);
      columnEntries = columnEntries.filter((e) => e.connectionId !== connectionId);
      loadedConnectionIds = new Set([...loadedConnectionIds].filter((id) => id !== connectionId));
      loadingConnectionIds = new Set([...loadingConnectionIds].filter((id) => id !== connectionId));
    },
  };
}
