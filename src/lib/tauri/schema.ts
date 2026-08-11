/** Typed wrappers around the Tauri `schema::*` commands. */
import { invoke } from '@tauri-apps/api/core';
import type {
  TableInfo,
  ColumnInfo,
  BulkColumnInfo,
  IndexInfo,
  ForeignKeyInfo,
  ViewInfo,
  CheckConstraintInfo,
  TriggerInfo,
} from '$lib/types';

export async function listDatabases(connectionId: string): Promise<string[]> {
  return invoke<string[]>('schema_list_databases', { connectionId });
}

export async function listSchemas(connectionId: string, instanceDb: string): Promise<string[]> {
  return invoke<string[]>('schema_list_schemas', { connectionId, instanceDb });
}

export async function listTables(
  connectionId: string,
  database: string,
  instanceDb?: string | null,
): Promise<TableInfo[]> {
  return invoke<TableInfo[]>('schema_list_tables', {
    connectionId,
    database,
    instanceDb: instanceDb ?? null,
  });
}

export async function listColumns(
  connectionId: string,
  database: string,
  table: string,
  instanceDb?: string | null,
): Promise<ColumnInfo[]> {
  return invoke<ColumnInfo[]>('schema_list_columns', {
    connectionId,
    database,
    table,
    instanceDb: instanceDb ?? null,
  });
}

export async function listAllColumns(
  connectionId: string,
  database: string,
  instanceDb?: string | null,
): Promise<BulkColumnInfo[]> {
  return invoke<BulkColumnInfo[]>('schema_list_all_columns', {
    connectionId,
    database,
    instanceDb: instanceDb ?? null,
  });
}

export async function listIndexes(
  connectionId: string,
  database: string,
  table: string,
  instanceDb?: string | null,
): Promise<IndexInfo[]> {
  return invoke<IndexInfo[]>('schema_list_indexes', {
    connectionId,
    database,
    table,
    instanceDb: instanceDb ?? null,
  });
}

export async function listForeignKeys(
  connectionId: string,
  database: string,
  table: string,
  instanceDb?: string | null,
): Promise<ForeignKeyInfo[]> {
  return invoke<ForeignKeyInfo[]>('schema_list_foreign_keys', {
    connectionId,
    database,
    table,
    instanceDb: instanceDb ?? null,
  });
}

export async function listCheckConstraints(
  connectionId: string,
  database: string,
  table?: string | null,
  instanceDb?: string | null,
): Promise<CheckConstraintInfo[]> {
  return invoke<CheckConstraintInfo[]>('schema_list_check_constraints', {
    connectionId,
    database,
    table: table ?? null,
    instanceDb: instanceDb ?? null,
  });
}

export async function listTriggers(
  connectionId: string,
  database: string,
  table?: string | null,
  instanceDb?: string | null,
): Promise<TriggerInfo[]> {
  return invoke<TriggerInfo[]>('schema_list_triggers', {
    connectionId,
    database,
    table: table ?? null,
    instanceDb: instanceDb ?? null,
  });
}

export async function listViews(
  connectionId: string,
  database: string,
  instanceDb?: string | null,
): Promise<ViewInfo[]> {
  return invoke<ViewInfo[]>('schema_list_views', {
    connectionId,
    database,
    instanceDb: instanceDb ?? null,
  });
}

export async function listAllIndexes(
  connectionId: string,
  database: string,
  instanceDb?: string | null,
): Promise<Array<IndexInfo & { tableName: string }>> {
  return invoke<Array<IndexInfo & { tableName: string }>>('schema_list_all_indexes', {
    connectionId,
    database,
    instanceDb: instanceDb ?? null,
  });
}

export async function listAllForeignKeys(
  connectionId: string,
  database: string,
  instanceDb?: string | null,
): Promise<Array<ForeignKeyInfo & { tableName: string }>> {
  return invoke<Array<ForeignKeyInfo & { tableName: string }>>('schema_list_all_foreign_keys', {
    connectionId,
    database,
    instanceDb: instanceDb ?? null,
  });
}

export async function executeDdl(connectionId: string, sql: string): Promise<void> {
  return invoke<void>('schema_execute_ddl', { connectionId, sql });
}

export async function getDdl(
  connectionId: string,
  database: string,
  objectName: string,
  instanceDb?: string | null,
): Promise<string> {
  return invoke<string>('schema_get_ddl', {
    connectionId,
    database,
    objectName,
    instanceDb: instanceDb ?? null,
  });
}
