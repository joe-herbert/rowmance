/** Typed wrappers around the Tauri `query::*` commands. */
import { invoke } from '@tauri-apps/api/core';
import type { QueryResult, ExplainResult } from '$lib/types';

export async function executeQuery(
  connectionId: string,
  sql: string,
  page: number,
  pageSize: number,
  database?: string | null,
  instanceDb?: string | null,
): Promise<QueryResult> {
  return invoke<QueryResult>('query_execute', {
    connectionId,
    sql,
    page,
    pageSize,
    database: database ?? null,
    instanceDb: instanceDb ?? null,
  });
}

export async function executeSelection(
  connectionId: string,
  sql: string,
  database?: string | null,
  instanceDb?: string | null,
): Promise<QueryResult> {
  return invoke<QueryResult>('query_execute_selection', {
    connectionId,
    sql,
    database: database ?? null,
    instanceDb: instanceDb ?? null,
  });
}

export async function executeMultiQuery(
  connectionId: string,
  sql: string,
  database?: string | null,
  instanceDb?: string | null,
  sessionId?: string | null,
): Promise<QueryResult[]> {
  return invoke<QueryResult[]>('query_execute_multi', {
    connectionId,
    sql,
    database: database ?? null,
    instanceDb: instanceDb ?? null,
    sessionId: sessionId ?? null,
  });
}

export async function cancelQuery(queryId: string): Promise<void> {
  return invoke<void>('query_cancel', { queryId });
}

export interface RowChange {
  primaryKeys: Record<string, unknown>;
  changes: Record<string, unknown>;
}

export interface UpdateResult {
  updatedCount: number;
}

export async function updateRows(
  connectionId: string,
  database: string,
  table: string,
  changes: RowChange[],
  instanceDb?: string | null,
): Promise<UpdateResult> {
  return invoke<UpdateResult>('query_update_rows', { connectionId, database, table, instanceDb: instanceDb ?? null, changes });
}

export interface RowDelete {
  primaryKeys: Record<string, unknown>;
}

export async function deleteRows(
  connectionId: string,
  database: string,
  table: string,
  rows: RowDelete[],
  instanceDb?: string | null,
): Promise<UpdateResult> {
  return invoke<UpdateResult>('query_delete_rows', { connectionId, database, table, instanceDb: instanceDb ?? null, rows });
}

export async function insertRow(
  connectionId: string,
  database: string,
  table: string,
  values: Record<string, unknown>,
  instanceDb?: string | null,
): Promise<void> {
  return invoke<void>('query_insert_row', { connectionId, database, table, instanceDb: instanceDb ?? null, values });
}

export interface SaveChangesResult {
  updatedCount: number;
  insertedCount: number;
  deletedCount: number;
}

export async function saveTableChanges(
  connectionId: string,
  database: string,
  table: string,
  updates: RowChange[],
  inserts: Record<string, unknown>[],
  deletes: RowDelete[],
  instanceDb?: string | null,
): Promise<SaveChangesResult> {
  return invoke<SaveChangesResult>('query_save_table_changes', {
    connectionId,
    database,
    table,
    instanceDb: instanceDb ?? null,
    updates,
    inserts,
    deletes,
  });
}

export async function explainQuery(
  connectionId: string,
  sql: string,
  database?: string | null,
  instanceDb?: string | null,
): Promise<ExplainResult> {
  return invoke<ExplainResult>('query_explain', { connectionId, sql, database: database ?? null, instanceDb: instanceDb ?? null });
}
