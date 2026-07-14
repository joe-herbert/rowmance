import * as schemaApi from '$lib/tauri/schema';
import type { useConnections } from '$lib/stores/connections.svelte';
import type { usePanels } from '$lib/stores/panels.svelte';

type ConnectionStore = ReturnType<typeof useConnections>;
type PanelStore = ReturnType<typeof usePanels>;

export function qi(name: string, dbType: string): string {
  if (dbType === 'mysql' || dbType === 'mariadb') return '`' + name.replace(/`/g, '``') + '`';
  return '"' + name.replace(/"/g, '""') + '"';
}

export function tableRef(database: string, table: string, dbType: string): string {
  if (dbType === 'sqlite') return qi(table, dbType);
  return `${qi(database, dbType)}.${qi(table, dbType)}`;
}

export function generateSqlSelectAll(
  connectionId: string,
  database: string,
  table: string,
  connectionStore: ConnectionStore,
  panelStore: PanelStore,
) {
  const profile = connectionStore.getById(connectionId);
  if (!profile) return;
  const ref = tableRef(database, table, profile.dbType);
  panelStore.openCopyInFocused({
    kind: 'query_editor',
    connectionId,
    database,
    initialSql: `SELECT * FROM ${ref}`,
  });
}

export function generateSqlSelectFirst(
  connectionId: string,
  database: string,
  table: string,
  connectionStore: ConnectionStore,
  panelStore: PanelStore,
) {
  const profile = connectionStore.getById(connectionId);
  if (!profile) return;
  const ref = tableRef(database, table, profile.dbType);
  panelStore.openCopyInFocused({
    kind: 'query_editor',
    connectionId,
    database,
    initialSql: `SELECT * FROM ${ref} LIMIT `,
  });
}

export async function generateSqlInsert(
  connectionId: string,
  database: string,
  table: string,
  connectionStore: ConnectionStore,
  panelStore: PanelStore,
) {
  const profile = connectionStore.getById(connectionId);
  if (!profile) return;
  const ref = tableRef(database, table, profile.dbType);
  let sql: string;
  try {
    const columns = await schemaApi.listColumns(connectionId, database, table);
    const insertCols = columns.filter((c) => !c.isAutoIncrement);
    const colList = insertCols.map((c) => qi(c.name, profile.dbType)).join(', ');
    const valList = insertCols.map(() => '').join(', ');
    sql = `INSERT INTO ${ref} (${colList})\nVALUES (${valList})`;
  } catch {
    sql = `INSERT INTO ${ref} ()\nVALUES ()`;
  }
  panelStore.openCopyInFocused({ kind: 'query_editor', connectionId, database, initialSql: sql });
}

export async function generateSqlUpdate(
  connectionId: string,
  database: string,
  table: string,
  connectionStore: ConnectionStore,
  panelStore: PanelStore,
) {
  const profile = connectionStore.getById(connectionId);
  if (!profile) return;
  const ref = tableRef(database, table, profile.dbType);
  let sql: string;
  try {
    const columns = await schemaApi.listColumns(connectionId, database, table);
    const pkCols = columns.filter((c) => c.isPrimaryKey);
    const dataCols = columns.filter((c) => !c.isPrimaryKey);
    const setCols = dataCols.length > 0 ? dataCols : columns;
    const setClauses = setCols.map((c) => `    ${qi(c.name, profile.dbType)} = `).join(',\n');
    const whereClauses =
      pkCols.length > 0
        ? pkCols.map((c) => `${qi(c.name, profile.dbType)} = `).join(' AND ')
        : '';
    sql = `UPDATE ${ref}\nSET\n${setClauses}\nWHERE ${whereClauses}`;
  } catch {
    sql = `UPDATE ${ref}\nSET\n    \nWHERE `;
  }
  panelStore.openCopyInFocused({ kind: 'query_editor', connectionId, database, initialSql: sql });
}

export async function generateSqlDelete(
  connectionId: string,
  database: string,
  table: string,
  connectionStore: ConnectionStore,
  panelStore: PanelStore,
) {
  const profile = connectionStore.getById(connectionId);
  if (!profile) return;
  const ref = tableRef(database, table, profile.dbType);
  let sql: string;
  try {
    const columns = await schemaApi.listColumns(connectionId, database, table);
    const pkCols = columns.filter((c) => c.isPrimaryKey);
    const whereClauses =
      pkCols.length > 0
        ? pkCols.map((c) => `${qi(c.name, profile.dbType)} = `).join(' AND ')
        : '';
    sql = `DELETE FROM ${ref}\nWHERE ${whereClauses}`;
  } catch {
    sql = `DELETE FROM ${ref}\nWHERE `;
  }
  panelStore.openCopyInFocused({ kind: 'query_editor', connectionId, database, initialSql: sql });
}
