/** Canonical SQL generation helpers driven by DialectInfo. */
import type { DialectInfo } from '$lib/types';

/** Safe fallback when dialectInfo is not yet available (neutral defaults). */
export const defaultDialectInfo: DialectInfo = {
  identifierOpen: '"',
  identifierClose: '"',
  identifierEscape: '""',
  usesSchema: true,
  dbLabel: 'Database',
  selectTop: false,
  booleanLiterals: false,
  usesIlike: false,
  castToText: 'CAST({col} AS TEXT)',
  supportsUserManagement: false,
  hostBasedUsers: false,
  supportsRoles: false,
  detectsSqlVariables: false,
  warnsTxDatabaseMismatch: false,
  displayName: '',
  defaultColumnType: 'INT',
  commonColumnTypes: [],
  supportsAutoIncrement: false,
  supportsColumnComment: false,
  supportsChangeColumn: false,
  supportsRenameColumn: false,
  usesForeignKeyKeyword: false,
  dropIndexSyntax: 'simple',
  defaultNewColumnType: 'TEXT',
  usesDatabaseKeyword: false,
  dropSchemaCascade: false,
  isFileBased: false,
  defaultPort: 0,
  urlSchemes: [],
  urlTemplate: '',
  fkViolation: null,
  editorDialect: 'sql',
  explainFormat: 'mysql_json',
  systemDatabases: [],
  fileExtensions: [],
};

/** Apply the castToText template to a pre-quoted column expression. */
export function castToText(quotedCol: string, d: DialectInfo): string {
  return d.castToText.replace('{col}', quotedCol);
}

/** Quote an identifier (table name, column name, etc.) for the given dialect. */
export function qi(name: string, d: DialectInfo): string {
  const escaped = name.split(d.identifierClose).join(d.identifierEscape);
  return d.identifierOpen + escaped + d.identifierClose;
}

/** Build a qualified table reference, omitting the schema prefix for SQLite. */
export function tableRef(database: string, table: string, d: DialectInfo): string {
  if (!d.usesSchema) return qi(table, d);
  return `${qi(database, d)}.${qi(table, d)}`;
}

/** Format a SQL value literal for the given dialect. */
export function formatSqlValue(val: unknown, d: DialectInfo): string {
  if (val === null || val === undefined) return 'NULL';
  if (typeof val === 'boolean') {
    if (d.booleanLiterals) return val ? 'TRUE' : 'FALSE';
    return val ? '1' : '0';
  }
  if (typeof val === 'number') return String(val);
  return `'${String(val).replace(/'/g, "''")}'`;
}
