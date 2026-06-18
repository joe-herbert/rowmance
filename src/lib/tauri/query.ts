/** Typed wrappers around the Tauri `query::*` commands. */
import { invoke } from '@tauri-apps/api/core';
import type { QueryResult, ExplainResult } from '$lib/types';

export async function executeQuery(
  connectionId: string,
  sql: string,
  page: number,
  pageSize: number,
): Promise<QueryResult> {
  return invoke<QueryResult>('query_execute', { connectionId, sql, page, pageSize });
}

export async function executeSelection(connectionId: string, sql: string): Promise<QueryResult> {
  return invoke<QueryResult>('query_execute_selection', { connectionId, sql });
}

export async function cancelQuery(queryId: string): Promise<void> {
  return invoke<void>('query_cancel', { queryId });
}

export async function formatQuery(sql: string, dialect: string): Promise<string> {
  return invoke<string>('query_format', { sql, dialect });
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
): Promise<UpdateResult> {
  return invoke<UpdateResult>('query_update_rows', { connectionId, database, table, changes });
}

export async function insertRow(
  connectionId: string,
  database: string,
  table: string,
  values: Record<string, unknown>,
): Promise<void> {
  return invoke<void>('query_insert_row', { connectionId, database, table, values });
}

export async function explainQuery(connectionId: string, sql: string): Promise<ExplainResult> {
  return invoke<ExplainResult>('query_explain', { connectionId, sql });
}
