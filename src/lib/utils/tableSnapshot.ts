// Table data fetching, shared between TableBrowser.svelte's own load() and the
// live-view poller (src/lib/stores/liveView.svelte.ts), which fetches on a timer
// without a mounted TableBrowser instance.
import { executeQuery, executeSelection } from '$lib/tauri/query';
import { listColumns, listIndexes, listForeignKeys } from '$lib/tauri/schema';
import {
  buildWhereClause,
  filterStateIsActive,
  type FilterEditorState,
} from '$lib/components/table/FilterEditor.svelte';
import { qi as dialectQi, castToText as dialectCastToText, defaultDialectInfo } from '$lib/utils/dialect';
import { useVirtualRelations } from '$lib/stores/virtualRelations.svelte';
import { tableSchemaCache, tableDataCache } from '$lib/stores/tableDataCache';
import type { DialectInfo, QueryResult, ColumnInfo, IndexInfo, ForeignKeyInfo } from '$lib/types';

export interface TableSnapshotParams {
  connectionId: string;
  database: string;
  table: string;
  instanceDb?: string | null;
  dialect: DialectInfo | undefined;
  filterEditorState: FilterEditorState;
  searchTerm: string;
  page: number;
  pageSize: number;
  previousTotalRows?: number | null;
  previousUnfilteredTotal?: number | null;
  /** Always run the total-row-count query, even without an active filter. Used by live polling for row-delta toasts. */
  forceCount?: boolean;
}

export interface TableSnapshot {
  result: QueryResult | null;
  error: string | null;
  unfilteredTotal: number | null;
  foreignKeys: ForeignKeyInfo[];
}

function quoteIdentifierFor(name: string, dialect: DialectInfo | undefined): string {
  return dialect ? dialectQi(name, dialect) : `\`${name.replace(/`/g, '``')}\``;
}

function buildSearchWhereFor(
  term: string,
  columns: ColumnInfo[],
  dialect: DialectInfo | undefined,
): string {
  if (columns.length === 0) return '';
  const escaped = term
    .replace(/\\/g, '\\\\')
    .replace(/%/g, '\\%')
    .replace(/_/g, '\\_')
    .replace(/'/g, "''");
  const pattern = `'%${escaped}%'`;
  const op = dialect?.usesIlike ? 'ILIKE' : 'LIKE';
  return (
    '(' +
    columns
      .map((c) => {
        const cast = dialectCastToText(quoteIdentifierFor(c.name, dialect), dialect ?? defaultDialectInfo);
        return `${cast} ${op} ${pattern}`;
      })
      .join(' OR ') +
    ')'
  );
}

/** Mirrors TableBrowser.svelte's local buildSql(), taking its reactive inputs as plain params. */
export function buildTableSnapshotSql(params: {
  table: string;
  database: string;
  dialect: DialectInfo | undefined;
  filterEditorState: FilterEditorState;
  searchTerm: string;
  knownColumns: ColumnInfo[];
}): string {
  const { table, database, dialect, filterEditorState, searchTerm, knownColumns } = params;
  const quoteIdentifier = (name: string) => quoteIdentifierFor(name, dialect);
  const quotedTable = quoteIdentifier(table);
  const tblTarget = dialect?.usesSchema ? `${quoteIdentifier(database)}.${quotedTable}` : quotedTable;
  let base = `SELECT * FROM ${tblTarget}`;
  const conditions: string[] = [];
  const filterWhere = buildWhereClause(filterEditorState, quoteIdentifier);
  if (filterWhere) conditions.push(filterWhere);
  const searchTrimmed = searchTerm.trim();
  if (searchTrimmed) {
    const searchWhere = buildSearchWhereFor(searchTrimmed, knownColumns, dialect);
    if (searchWhere) conditions.push(searchWhere);
  }
  if (conditions.length > 0) base += ` WHERE ${conditions.join(' AND ')}`;
  return base;
}

/**
 * Fetches one page of table data plus schema/count, writing the result into
 * tableSchemaCache/tableDataCache. Callable with no mounted component.
 */
export async function fetchTableSnapshot(params: TableSnapshotParams): Promise<TableSnapshot> {
  const {
    connectionId,
    database,
    table,
    instanceDb,
    dialect,
    filterEditorState,
    searchTerm,
    page,
    pageSize,
    previousTotalRows = null,
    previousUnfilteredTotal = null,
    forceCount = false,
  } = params;

  const quoteIdentifier = (name: string) => quoteIdentifierFor(name, dialect);
  const filterActive = filterStateIsActive(filterEditorState);
  const countTarget = dialect?.usesSchema
    ? `${quoteIdentifier(database)}.${quoteIdentifier(table)}`
    : quoteIdentifier(table);
  const countSql = `SELECT COUNT(*) FROM ${countTarget}`;

  const schemaKey = `${connectionId}:${database}:${table}`;
  const cachedSchema = tableSchemaCache.get(schemaKey);
  const schemaPromise = cachedSchema
    ? Promise.resolve(cachedSchema)
    : Promise.all([
        listColumns(connectionId, database, table, instanceDb).catch((): ColumnInfo[] => []),
        listIndexes(connectionId, database, table, instanceDb).catch((): IndexInfo[] => []),
        listForeignKeys(connectionId, database, table, instanceDb).catch(
          (): ForeignKeyInfo[] => [],
        ),
      ]).then(([columns, indexes, fks]) => {
        const schema = { columns, indexes, foreignKeys: fks };
        tableSchemaCache.set(schemaKey, schema);
        return schema;
      });

  const knownColumns = cachedSchema?.columns ?? [];
  const sql = buildTableSnapshotSql({ table, database, dialect, filterEditorState, searchTerm, knownColumns });

  const [queryResult, schema, countResult] = await Promise.all([
    executeQuery(connectionId, sql, page, pageSize, database, instanceDb),
    schemaPromise,
    filterActive || forceCount
      ? executeSelection(connectionId, countSql, database, instanceDb)
      : Promise.resolve(null),
  ]);

  const { columns: schemaColumns, indexes, foreignKeys: fks } = schema;

  let unfilteredTotal = previousUnfilteredTotal;
  if (countResult && !countResult.error) {
    const raw = countResult.rows[0]?.[0];
    unfilteredTotal = raw !== null && raw !== undefined ? Number(raw) : null;
  } else if (!filterActive && !forceCount) {
    unfilteredTotal = null;
  }

  if (queryResult.error) {
    return { result: null, error: queryResult.error, unfilteredTotal, foreignKeys: fks };
  }

  if (schemaColumns.length > 0) {
    const vrStore = useVirtualRelations();
    const schemaMap = new Map(schemaColumns.map((c) => [c.name, c]));
    const uniqueColNames = new Set(
      indexes.filter((idx) => idx.unique).flatMap((idx) => idx.columns),
    );
    queryResult.columns = queryResult.columns.map((col) => {
      const s = schemaMap.get(col.name);
      return {
        ...col,
        dataType: s?.dataType ?? col.dataType,
        nullable: s ? s.nullable : col.nullable,
        isPrimaryKey: s?.isPrimaryKey ?? false,
        isForeignKey:
          (s?.isForeignKey ?? false) ||
          vrStore.hasForwardFrom(connectionId, database, table, col.name) ||
          vrStore.hasPolymorphicValueColumn(connectionId, database, table, col.name),
        defaultValue: s?.defaultValue ?? null,
        isAutoIncrement: s?.isAutoIncrement ?? false,
        isUnique: uniqueColNames.has(col.name),
      };
    });
  }

  const displayResult: QueryResult =
    queryResult.totalRows === null && previousTotalRows !== null
      ? { ...queryResult, totalRows: previousTotalRows }
      : queryResult;

  tableDataCache.set(`${connectionId}:${database}:${table}`, {
    result: displayResult,
    unfilteredTotal,
    foreignKeys: fks,
  });

  return { result: displayResult, error: null, unfilteredTotal, foreignKeys: fks };
}
