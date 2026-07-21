// Shared across all TableBrowser instances and the live-view poller — persists across remounts.
import type { ColumnInfo, IndexInfo, ForeignKeyInfo, QueryResult } from '$lib/types';

export type CachedSchema = {
  columns: ColumnInfo[];
  indexes: IndexInfo[];
  foreignKeys: ForeignKeyInfo[];
};

export type CachedData = {
  result: QueryResult;
  unfilteredTotal: number | null;
  foreignKeys: ForeignKeyInfo[];
};

export const tableSchemaCache = new Map<string, CachedSchema>();
export const tableDataCache = new Map<string, CachedData>();
