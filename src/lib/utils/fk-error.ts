/** Generic FK constraint violation error parser driven by DialectInfo.fkViolation hints. */
import type { DialectInfo } from '$lib/types';
import { qi } from '$lib/utils/dialect';

export interface FkNavigationInfo {
  referencingTable: string;
  filterSql: string;
}

/**
 * Parse an FK violation error and return navigation info for the referencing table.
 * Returns null when the error is not an FK violation or the dialect provides no hints.
 */
export function parseFkViolationError(
  err: unknown,
  deletedRows: Map<string, unknown[]>,
  cols: { name: string }[],
  dialect: DialectInfo | undefined,
): FkNavigationInfo | null {
  const hint = dialect?.fkViolation;
  if (!hint) return null;

  const errObj = typeof err === 'object' && err !== null ? (err as Record<string, unknown>) : null;
  const message = errObj ? String(errObj.message ?? '') : String(err);
  const detail = errObj ? String(errObj.detail ?? '') : '';
  const fullText = `${message}\n${detail}`;

  if (!fullText.toLowerCase().includes(hint.detect.toLowerCase())) return null;

  const tableMatch = new RegExp(hint.tablePattern, 'i').exec(fullText);
  if (!tableMatch) return null;
  const referencingTable = tableMatch[1];

  let filterSql = '';

  if (hint.columnPairPattern) {
    // Engine provides FK col (referencing table) and referenced col separately.
    // Value is read from the deleted row using the referenced col name.
    const pairMatch = new RegExp(hint.columnPairPattern, 'i').exec(fullText);
    if (pairMatch) {
      const fkCol = pairMatch[1];
      const referencedCol = pairMatch[2];
      const colIdx = cols.findIndex((c) => c.name === referencedCol);
      const rowValues = [...deletedRows.values()][0];
      const value = colIdx >= 0 ? rowValues?.[colIdx] : undefined;
      if (value !== undefined && value !== null) {
        const quotedFkCol = dialect ? qi(fkCol, dialect) : `\`${fkCol.replace(/`/g, '``')}\``;
        filterSql =
          typeof value === 'number' || typeof value === 'boolean'
            ? `${quotedFkCol} = ${value}`
            : `${quotedFkCol} = '${String(value).replace(/'/g, "''")}'`;
      }
    }
  } else if (hint.columnValuePattern) {
    // Engine embeds both the referenced column name and value in the error text.
    const valMatch = new RegExp(hint.columnValuePattern, 'i').exec(fullText);
    if (valMatch) {
      const referencedCol = valMatch[1];
      const rawValue = valMatch[2];
      const quotedCol = dialect ? qi(referencedCol, dialect) : `"${referencedCol.replace(/"/g, '""')}"`;
      filterSql = /^\d+$/.test(rawValue)
        ? `${quotedCol} = ${rawValue}`
        : `${quotedCol} = '${rawValue.replace(/'/g, "''")}'`;
    }
  }

  return { referencingTable, filterSql };
}
