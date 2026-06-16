import { describe, it, expect } from 'vitest';
import type { ColumnMeta } from '$lib/types';

/**
 * Pure functions extracted from DataTable.svelte for unit testing.
 * These mirror the exported logic in the component script.
 */

type CellValue = string | number | boolean | null;
type SortDir = 'asc' | 'desc' | 'none';

function sortRows(rows: CellValue[][], colIndex: number, dir: SortDir): CellValue[][] {
  if (dir === 'none' || colIndex < 0) return rows;
  const sorted = [...rows].sort((a, b) => {
    const av = a[colIndex];
    const bv = b[colIndex];
    if (av === null && bv === null) return 0;
    if (av === null) return 1;
    if (bv === null) return -1;
    if (typeof av === 'number' && typeof bv === 'number') {
      return dir === 'asc' ? av - bv : bv - av;
    }
    const as = String(av).toLowerCase();
    const bs = String(bv).toLowerCase();
    if (as < bs) return dir === 'asc' ? -1 : 1;
    if (as > bs) return dir === 'asc' ? 1 : -1;
    return 0;
  });
  return sorted;
}

function filterRows(rows: CellValue[][], filters: string[]): CellValue[][] {
  const activeFilters = filters
    .map((f, i) => ({ i, term: f.trim().toLowerCase() }))
    .filter((f) => f.term !== '');
  if (activeFilters.length === 0) return rows;
  return rows.filter((row) =>
    activeFilters.every(({ i, term }) => {
      const cell = row[i];
      const cellStr = cell === null ? '' : String(cell).toLowerCase();
      return cellStr.includes(term);
    }),
  );
}

function buildRowKey(row: CellValue[], columns: ColumnMeta[], rowIndex: number): string {
  const pkCols = columns
    .map((col, idx) => ({ col, idx }))
    .filter(({ col }) => col.isPrimaryKey);
  if (pkCols.length > 0) {
    return pkCols.map(({ idx }) => String(row[idx] ?? '')).join('|');
  }
  return String(rowIndex);
}

// ── sortRows tests ────────────────────────────────────────────────────────────

describe('sortRows', () => {
  const rows: CellValue[][] = [
    ['banana', 2],
    ['apple', 10],
    ['cherry', 1],
    [null, 5],
  ];

  it('sorts strings ascending on col 0', () => {
    const result = sortRows(rows, 0, 'asc');
    expect(result.map((r) => r[0])).toEqual(['apple', 'banana', 'cherry', null]);
  });

  it('sorts strings descending on col 0', () => {
    const result = sortRows(rows, 0, 'desc');
    expect(result.map((r) => r[0])).toEqual(['cherry', 'banana', 'apple', null]);
  });

  it('sorts numbers ascending on col 1', () => {
    const result = sortRows(rows, 1, 'asc');
    expect(result.map((r) => r[1])).toEqual([1, 2, 5, 10]);
  });

  it('sorts numbers descending on col 1', () => {
    const result = sortRows(rows, 1, 'desc');
    expect(result.map((r) => r[1])).toEqual([10, 5, 2, 1]);
  });

  it('returns the same rows when dir is "none"', () => {
    const result = sortRows(rows, 0, 'none');
    expect(result).toBe(rows);
  });

  it('returns the same rows when colIndex is -1', () => {
    const result = sortRows(rows, -1, 'asc');
    expect(result).toBe(rows);
  });

  it('puts null values last when sorting ascending', () => {
    const result = sortRows(rows, 0, 'asc');
    expect(result[result.length - 1][0]).toBeNull();
  });

  it('puts null values last when sorting descending (nulls still last)', () => {
    // Nulls always sort last regardless of direction.
    const result = sortRows(rows, 0, 'desc');
    const nullIdx = result.findIndex((r) => r[0] === null);
    expect(nullIdx).toBe(result.length - 1);
  });

  it('does not mutate the original array', () => {
    const original = [...rows];
    sortRows(rows, 0, 'asc');
    expect(rows).toEqual(original);
  });

  it('is case-insensitive for string comparison', () => {
    const mixedCase: CellValue[][] = [['Zebra'], ['apple'], ['Mango']];
    const result = sortRows(mixedCase, 0, 'asc');
    expect(result.map((r) => r[0])).toEqual(['apple', 'Mango', 'Zebra']);
  });
});

// ── filterRows tests ──────────────────────────────────────────────────────────

describe('filterRows', () => {
  const rows: CellValue[][] = [
    ['Alice', 30, true],
    ['Bob', 25, false],
    ['alice_smith', 40, null],
    [null, 35, true],
  ];

  it('returns all rows when all filters are empty', () => {
    expect(filterRows(rows, ['', '', ''])).toBe(rows);
  });

  it('filters by substring on first column (case-insensitive)', () => {
    const result = filterRows(rows, ['alice', '', '']);
    expect(result).toHaveLength(2);
    expect(result[0][0]).toBe('Alice');
    expect(result[1][0]).toBe('alice_smith');
  });

  it('filters by number value', () => {
    const result = filterRows(rows, ['', '3', '']);
    // rows with col[1] containing "3": 30, 35, 30→"30" has "3" ✓, 25→"25" no, 40→"40" no, 35→"35" has "3" ✓
    expect(result.map((r) => r[1])).toEqual([30, 35]);
  });

  it('filters across multiple columns simultaneously', () => {
    const result = filterRows(rows, ['bob', '25', '']);
    expect(result).toHaveLength(1);
    expect(result[0][0]).toBe('Bob');
  });

  it('treats null cells as empty string for matching', () => {
    // Filter for empty string in col[0] — should not match null (empty string filter is inactive)
    // Filter for a non-empty term in col[0] that would match null (null→"" which includes "" but not a real term)
    const result = filterRows(rows, ['z', '', '']);
    // No row has "z" in col[0]: Alice→no, Bob→no, alice_smith→no, null→""→no
    expect(result).toHaveLength(0);
  });

  it('returns empty array when no rows match', () => {
    const result = filterRows(rows, ['XXXXXXX', '', '']);
    expect(result).toHaveLength(0);
  });

  it('trims whitespace from filter terms', () => {
    const result = filterRows(rows, ['  Alice  ', '', '']);
    expect(result).toHaveLength(2); // Alice and alice_smith
  });
});

// ── buildRowKey tests ─────────────────────────────────────────────────────────

function makeColumn(
  name: string,
  isPrimaryKey: boolean,
  dataType = 'VARCHAR(255)',
): ColumnMeta {
  return { name, dataType, nullable: true, isPrimaryKey, isForeignKey: false };
}

describe('buildRowKey', () => {
  it('uses row index as key when no primary key columns exist', () => {
    const columns = [makeColumn('name', false), makeColumn('age', false)];
    const row: CellValue[] = ['Alice', 30];
    expect(buildRowKey(row, columns, 5)).toBe('5');
    expect(buildRowKey(row, columns, 0)).toBe('0');
  });

  it('uses the primary key value when one PK column exists', () => {
    const columns = [makeColumn('id', true), makeColumn('name', false)];
    const row: CellValue[] = [42, 'Alice'];
    expect(buildRowKey(row, columns, 0)).toBe('42');
  });

  it('joins multiple PK values with pipe when composite PK', () => {
    const columns = [
      makeColumn('tenant_id', true),
      makeColumn('user_id', true),
      makeColumn('name', false),
    ];
    const row: CellValue[] = [10, 99, 'Alice'];
    expect(buildRowKey(row, columns, 0)).toBe('10|99');
  });

  it('handles null PK value by using empty string', () => {
    const columns = [makeColumn('id', true), makeColumn('name', false)];
    const row: CellValue[] = [null, 'Alice'];
    expect(buildRowKey(row, columns, 0)).toBe('');
  });

  it('handles composite PK with one null value', () => {
    const columns = [makeColumn('a', true), makeColumn('b', true)];
    const row: CellValue[] = [1, null];
    expect(buildRowKey(row, columns, 0)).toBe('1|');
  });
});

// ── reorderColumns tests ──────────────────────────────────────────────────────

function reorderColumns(order: number[], fromIdx: number, toIdx: number): number[] {
  if (fromIdx === toIdx) return order;
  const next = [...order];
  const [item] = next.splice(fromIdx, 1);
  next.splice(toIdx, 0, item);
  return next;
}

describe('reorderColumns', () => {
  it('moves a column forward', () => {
    expect(reorderColumns([0, 1, 2, 3], 0, 2)).toEqual([1, 2, 0, 3]);
  });

  it('moves a column backward', () => {
    expect(reorderColumns([0, 1, 2, 3], 3, 1)).toEqual([0, 3, 1, 2]);
  });

  it('returns the same array reference when fromIdx === toIdx', () => {
    const order = [0, 1, 2];
    expect(reorderColumns(order, 1, 1)).toBe(order);
  });

  it('does not mutate the original array', () => {
    const original = [0, 1, 2, 3];
    reorderColumns(original, 0, 3);
    expect(original).toEqual([0, 1, 2, 3]);
  });

  it('moves the last column to the front', () => {
    expect(reorderColumns([0, 1, 2], 2, 0)).toEqual([2, 0, 1]);
  });

  it('moves the first column to the end', () => {
    expect(reorderColumns([0, 1, 2], 0, 2)).toEqual([1, 2, 0]);
  });

  it('handles two-column swap', () => {
    expect(reorderColumns([0, 1], 0, 1)).toEqual([1, 0]);
    expect(reorderColumns([0, 1], 1, 0)).toEqual([1, 0]);
  });
});

// ── keyboard cell navigation ──────────────────────────────────────────────────

// Pure mirror of handleTableKeydown's movement logic, extracted for testing.
function moveFocus(
  current: { row: number; col: number } | null,
  key: string,
  rowCount: number,
  colCount: number,
): { row: number; col: number } | null {
  if (rowCount === 0 || colCount === 0) return current;

  if (!current) {
    if (['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(key)) {
      return { row: 0, col: 0 };
    }
    return null;
  }

  let { row, col } = current;

  if (key === 'ArrowDown') row = Math.min(row + 1, rowCount - 1);
  else if (key === 'ArrowUp') row = Math.max(row - 1, 0);
  else if (key === 'ArrowRight') col = Math.min(col + 1, colCount - 1);
  else if (key === 'ArrowLeft') col = Math.max(col - 1, 0);
  else return current;

  return { row, col };
}

describe('moveFocus — initial selection', () => {
  it('returns {0,0} on ArrowDown when no cell is focused', () => {
    expect(moveFocus(null, 'ArrowDown', 3, 3)).toEqual({ row: 0, col: 0 });
  });

  it('returns {0,0} on ArrowRight when no cell is focused', () => {
    expect(moveFocus(null, 'ArrowRight', 3, 3)).toEqual({ row: 0, col: 0 });
  });

  it('returns null for non-arrow keys when no cell is focused', () => {
    expect(moveFocus(null, 'Enter', 3, 3)).toBeNull();
    expect(moveFocus(null, 'Escape', 3, 3)).toBeNull();
  });

  it('returns current as-is when the table is empty', () => {
    expect(moveFocus(null, 'ArrowDown', 0, 0)).toBeNull();
    expect(moveFocus({ row: 0, col: 0 }, 'ArrowDown', 0, 0)).toEqual({ row: 0, col: 0 });
  });
});

describe('moveFocus — arrow movement', () => {
  it('moves down one row', () => {
    expect(moveFocus({ row: 0, col: 0 }, 'ArrowDown', 3, 3)).toEqual({ row: 1, col: 0 });
  });

  it('moves up one row', () => {
    expect(moveFocus({ row: 2, col: 0 }, 'ArrowUp', 3, 3)).toEqual({ row: 1, col: 0 });
  });

  it('moves right one column', () => {
    expect(moveFocus({ row: 0, col: 0 }, 'ArrowRight', 3, 3)).toEqual({ row: 0, col: 1 });
  });

  it('moves left one column', () => {
    expect(moveFocus({ row: 0, col: 2 }, 'ArrowLeft', 3, 3)).toEqual({ row: 0, col: 1 });
  });
});

describe('moveFocus — boundary clamping', () => {
  it('does not move past the last row on ArrowDown', () => {
    expect(moveFocus({ row: 2, col: 0 }, 'ArrowDown', 3, 3)).toEqual({ row: 2, col: 0 });
  });

  it('does not move past row 0 on ArrowUp', () => {
    expect(moveFocus({ row: 0, col: 0 }, 'ArrowUp', 3, 3)).toEqual({ row: 0, col: 0 });
  });

  it('does not move past the last column on ArrowRight', () => {
    expect(moveFocus({ row: 0, col: 2 }, 'ArrowRight', 3, 3)).toEqual({ row: 0, col: 2 });
  });

  it('does not move past column 0 on ArrowLeft', () => {
    expect(moveFocus({ row: 0, col: 0 }, 'ArrowLeft', 3, 3)).toEqual({ row: 0, col: 0 });
  });
});

describe('moveFocus — unrecognised keys', () => {
  it('returns the current position unchanged for unrecognised keys', () => {
    const pos = { row: 1, col: 1 };
    expect(moveFocus(pos, 'Home', 3, 3)).toEqual(pos);
    expect(moveFocus(pos, 'PageDown', 3, 3)).toEqual(pos);
  });
});
