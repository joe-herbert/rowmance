import { describe, it, expect } from 'vitest';

// Mirrors of the pure grouping helpers from commands/erd.rs, implemented in TS
// so we can test the same invariants on the frontend side.

interface ErdColumn {
  name: string;
  dataType: string;
  isPrimaryKey: boolean;
}

interface ErdTable {
  name: string;
  columns: ErdColumn[];
}

interface ErdRelation {
  fromTable: string;
  fromColumns: string[];
  toTable: string;
  toColumns: string[];
  constraintName: string;
}

// Mirrors group_into_tables from erd.rs.
function groupIntoTables(rows: Array<[string, ErdColumn]>): ErdTable[] {
  const tables: ErdTable[] = [];
  for (const [tableName, col] of rows) {
    const last = tables[tables.length - 1];
    if (last && last.name === tableName) {
      last.columns.push(col);
    } else {
      tables.push({ name: tableName, columns: [col] });
    }
  }
  return tables;
}

// Mirrors build_edges_from_fk_rows from erd.rs.
interface FkRow {
  constraintName: string;
  tableName: string;
  columnName: string;
  referencedTableName: string;
  referencedColumnName: string;
}

function buildEdgesFromFkRows(rows: FkRow[]): ErdRelation[] {
  const edges: ErdRelation[] = [];
  for (const row of rows) {
    const last = edges[edges.length - 1];
    if (last && last.constraintName === row.constraintName) {
      last.fromColumns.push(row.columnName);
      last.toColumns.push(row.referencedColumnName);
    } else {
      edges.push({
        fromTable: row.tableName,
        fromColumns: [row.columnName],
        toTable: row.referencedTableName,
        toColumns: [row.referencedColumnName],
        constraintName: row.constraintName,
      });
    }
  }
  return edges;
}

// ── groupIntoTables ───────────────────────────────────────────────────────────

describe('groupIntoTables', () => {
  it('returns empty array for empty input', () => {
    expect(groupIntoTables([])).toEqual([]);
  });

  it('groups all columns under the same table', () => {
    const rows: Array<[string, ErdColumn]> = [
      ['users', { name: 'id', dataType: 'int', isPrimaryKey: true }],
      ['users', { name: 'name', dataType: 'varchar', isPrimaryKey: false }],
    ];
    const tables = groupIntoTables(rows);
    expect(tables).toHaveLength(1);
    expect(tables[0].name).toBe('users');
    expect(tables[0].columns).toHaveLength(2);
    expect(tables[0].columns[0].name).toBe('id');
    expect(tables[0].columns[0].isPrimaryKey).toBe(true);
  });

  it('creates separate tables for different table names', () => {
    const rows: Array<[string, ErdColumn]> = [
      ['users', { name: 'id', dataType: 'int', isPrimaryKey: true }],
      ['orders', { name: 'id', dataType: 'int', isPrimaryKey: true }],
      ['orders', { name: 'user_id', dataType: 'int', isPrimaryKey: false }],
    ];
    const tables = groupIntoTables(rows);
    expect(tables).toHaveLength(2);
    expect(tables[0].name).toBe('users');
    expect(tables[1].name).toBe('orders');
    expect(tables[1].columns).toHaveLength(2);
  });

  it('preserves column order within a table', () => {
    const rows: Array<[string, ErdColumn]> = [
      ['t', { name: 'a', dataType: 'int', isPrimaryKey: false }],
      ['t', { name: 'b', dataType: 'int', isPrimaryKey: false }],
      ['t', { name: 'c', dataType: 'int', isPrimaryKey: false }],
    ];
    const tables = groupIntoTables(rows);
    expect(tables[0].columns.map((c) => c.name)).toEqual(['a', 'b', 'c']);
  });

  it('handles a single column single table', () => {
    const rows: Array<[string, ErdColumn]> = [
      ['settings', { name: 'key', dataType: 'text', isPrimaryKey: true }],
    ];
    const tables = groupIntoTables(rows);
    expect(tables).toHaveLength(1);
    expect(tables[0].columns).toHaveLength(1);
  });
});

// ── buildEdgesFromFkRows ──────────────────────────────────────────────────────

describe('buildEdgesFromFkRows', () => {
  it('returns empty array for empty input', () => {
    expect(buildEdgesFromFkRows([])).toEqual([]);
  });

  it('creates one edge for a single-column FK', () => {
    const rows: FkRow[] = [{
      constraintName: 'fk_order_user',
      tableName: 'orders',
      columnName: 'user_id',
      referencedTableName: 'users',
      referencedColumnName: 'id',
    }];
    const edges = buildEdgesFromFkRows(rows);
    expect(edges).toHaveLength(1);
    expect(edges[0].constraintName).toBe('fk_order_user');
    expect(edges[0].fromTable).toBe('orders');
    expect(edges[0].fromColumns).toEqual(['user_id']);
    expect(edges[0].toTable).toBe('users');
    expect(edges[0].toColumns).toEqual(['id']);
  });

  it('merges multiple rows with the same constraint into one edge', () => {
    const rows: FkRow[] = [
      {
        constraintName: 'fk_composite',
        tableName: 'order_items',
        columnName: 'order_id',
        referencedTableName: 'orders',
        referencedColumnName: 'id',
      },
      {
        constraintName: 'fk_composite',
        tableName: 'order_items',
        columnName: 'product_id',
        referencedTableName: 'orders',
        referencedColumnName: 'product_id',
      },
    ];
    const edges = buildEdgesFromFkRows(rows);
    expect(edges).toHaveLength(1);
    expect(edges[0].fromColumns).toEqual(['order_id', 'product_id']);
    expect(edges[0].toColumns).toEqual(['id', 'product_id']);
  });

  it('creates separate edges for different constraints', () => {
    const rows: FkRow[] = [
      {
        constraintName: 'fk_a',
        tableName: 'child',
        columnName: 'parent_a_id',
        referencedTableName: 'parent_a',
        referencedColumnName: 'id',
      },
      {
        constraintName: 'fk_b',
        tableName: 'child',
        columnName: 'parent_b_id',
        referencedTableName: 'parent_b',
        referencedColumnName: 'id',
      },
    ];
    const edges = buildEdgesFromFkRows(rows);
    expect(edges).toHaveLength(2);
    expect(edges[0].constraintName).toBe('fk_a');
    expect(edges[1].constraintName).toBe('fk_b');
  });
});
