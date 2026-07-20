import * as schemaApi from '$lib/tauri/schema';
import { qi, tableRef } from '$lib/utils/dialect';
import type { useConnections } from '$lib/stores/connections.svelte';
import type { usePanels } from '$lib/stores/panels.svelte';

type ConnectionStore = ReturnType<typeof useConnections>;
type PanelStore = ReturnType<typeof usePanels>;

export function generateSqlSelectAll(
  connectionId: string,
  database: string,
  table: string,
  connectionStore: ConnectionStore,
  panelStore: PanelStore,
) {
  const profile = connectionStore.getById(connectionId);
  if (!profile) return;
  const ref = tableRef(database, table, profile.dialectInfo);
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
  const ref = tableRef(database, table, profile.dialectInfo);
  const initialSql = profile.dialectInfo.selectTop
    ? `SELECT TOP  * FROM ${ref}`
    : `SELECT * FROM ${ref} LIMIT `;
  panelStore.openCopyInFocused({
    kind: 'query_editor',
    connectionId,
    database,
    initialSql,
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
  const ref = tableRef(database, table, profile.dialectInfo);
  let sql: string;
  try {
    const columns = await schemaApi.listColumns(connectionId, database, table);
    const insertCols = columns.filter((c) => !c.isAutoIncrement);
    const colList = insertCols.map((c) => qi(c.name, profile.dialectInfo)).join(', ');
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
  const ref = tableRef(database, table, profile.dialectInfo);
  let sql: string;
  try {
    const columns = await schemaApi.listColumns(connectionId, database, table);
    const pkCols = columns.filter((c) => c.isPrimaryKey);
    const dataCols = columns.filter((c) => !c.isPrimaryKey);
    const setCols = dataCols.length > 0 ? dataCols : columns;
    const setClauses = setCols.map((c) => `    ${qi(c.name, profile.dialectInfo)} = `).join(',\n');
    const whereClauses =
      pkCols.length > 0
        ? pkCols.map((c) => `${qi(c.name, profile.dialectInfo)} = `).join(' AND ')
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
  const ref = tableRef(database, table, profile.dialectInfo);
  let sql: string;
  try {
    const columns = await schemaApi.listColumns(connectionId, database, table);
    const pkCols = columns.filter((c) => c.isPrimaryKey);
    const whereClauses =
      pkCols.length > 0
        ? pkCols.map((c) => `${qi(c.name, profile.dialectInfo)} = `).join(' AND ')
        : '';
    sql = `DELETE FROM ${ref}\nWHERE ${whereClauses}`;
  } catch {
    sql = `DELETE FROM ${ref}\nWHERE `;
  }
  panelStore.openCopyInFocused({ kind: 'query_editor', connectionId, database, initialSql: sql });
}
