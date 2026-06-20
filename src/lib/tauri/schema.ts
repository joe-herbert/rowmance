/** Typed wrappers around the Tauri `schema::*` commands. */
import { invoke } from '@tauri-apps/api/core';
import type { TableInfo, ColumnInfo, IndexInfo, ForeignKeyInfo } from '$lib/types';

export async function listDatabases(connectionId: string): Promise<string[]> {
  return invoke<string[]>('schema_list_databases', { connectionId });
}

export async function listTables(connectionId: string, database: string): Promise<TableInfo[]> {
  return invoke<TableInfo[]>('schema_list_tables', { connectionId, database });
}

export async function listColumns(
  connectionId: string,
  database: string,
  table: string,
): Promise<ColumnInfo[]> {
  return invoke<ColumnInfo[]>('schema_list_columns', { connectionId, database, table });
}

export async function listIndexes(
  connectionId: string,
  database: string,
  table: string,
): Promise<IndexInfo[]> {
  return invoke<IndexInfo[]>('schema_list_indexes', { connectionId, database, table });
}

export async function listForeignKeys(
  connectionId: string,
  database: string,
  table: string,
): Promise<ForeignKeyInfo[]> {
  return invoke<ForeignKeyInfo[]>('schema_list_foreign_keys', { connectionId, database, table });
}

export async function executeDdl(connectionId: string, sql: string): Promise<void> {
  return invoke<void>('schema_execute_ddl', { connectionId, sql });
}

export async function getDdl(
  connectionId: string,
  database: string,
  objectName: string,
  objectType: 'table' | 'view',
): Promise<string> {
  return invoke<string>('schema_get_ddl', { connectionId, database, objectName, objectType });
}
