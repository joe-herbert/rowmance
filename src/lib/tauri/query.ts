/** Typed wrappers around the Tauri `query::*` commands. */
import { invoke } from '@tauri-apps/api/core';
import type { QueryResult } from '$lib/types';

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
