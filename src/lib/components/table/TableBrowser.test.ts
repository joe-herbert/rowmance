import { describe, it, expect } from 'vitest';

// Pure functions redefined locally to avoid rune compilation issues in test context.
// These mirror the inline functions in TableBrowser.svelte, adapted to take
// dbType as a parameter instead of reading it from component state.

const PAGE_SIZE = 50;

function quoteIdentifier(name: string, dbType: string): string {
  return dbType === 'postgres' ? `"${name}"` : `\`${name}\``;
}

function buildSql(
  database: string,
  table: string,
  page: number,
  pageSize: number,
  filterValue: string,
  dbType: string,
): string {
  const quotedDb = quoteIdentifier(database, dbType);
  const quotedTable = quoteIdentifier(table, dbType);
  let base = `SELECT * FROM ${quotedDb}.${quotedTable}`;
  if (filterValue.trim()) {
    base += ` WHERE ${filterValue.trim()}`;
  }
  return base;
}

// ── quoteIdentifier tests ─────────────────────────────────────────────────────

describe('quoteIdentifier', () => {
  it('wraps identifier in double quotes for postgres', () => {
    expect(quoteIdentifier('myTable', 'postgres')).toBe('"myTable"');
  });

  it('wraps identifier in backticks for mysql', () => {
    expect(quoteIdentifier('myTable', 'mysql')).toBe('`myTable`');
  });

  it('wraps identifier in backticks for mariadb', () => {
    expect(quoteIdentifier('myTable', 'mariadb')).toBe('`myTable`');
  });

  it('wraps database name in double quotes for postgres', () => {
    expect(quoteIdentifier('my_database', 'postgres')).toBe('"my_database"');
  });

  it('wraps database name in backticks for mysql', () => {
    expect(quoteIdentifier('my_database', 'mysql')).toBe('`my_database`');
  });
});

// ── buildSql tests ────────────────────────────────────────────────────────────

describe('buildSql', () => {
  it('produces a basic SELECT with no filter for mysql', () => {
    const sql = buildSql('mydb', 'users', 1, PAGE_SIZE, '', 'mysql');
    expect(sql).toBe('SELECT * FROM `mydb`.`users`');
  });

  it('produces a basic SELECT with no filter for postgres', () => {
    const sql = buildSql('mydb', 'users', 1, PAGE_SIZE, '', 'postgres');
    expect(sql).toBe('SELECT * FROM "mydb"."users"');
  });

  it('includes a WHERE clause when filterValue is non-empty', () => {
    const sql = buildSql('mydb', 'users', 1, PAGE_SIZE, 'age > 18', 'mysql');
    expect(sql).toBe('SELECT * FROM `mydb`.`users` WHERE age > 18');
  });

  it('trims whitespace from filterValue in the WHERE clause', () => {
    const sql = buildSql('mydb', 'users', 1, PAGE_SIZE, '  age > 18  ', 'mysql');
    expect(sql).toBe('SELECT * FROM `mydb`.`users` WHERE age > 18');
  });

  it('does not include WHERE clause when filterValue is only whitespace', () => {
    const sql = buildSql('mydb', 'users', 1, PAGE_SIZE, '   ', 'mysql');
    expect(sql).not.toContain('WHERE');
  });

  it('does not include LIMIT or OFFSET (pagination is handled by the backend)', () => {
    const sql1 = buildSql('mydb', 'users', 1, PAGE_SIZE, '', 'mysql');
    const sql2 = buildSql('mydb', 'users', 2, PAGE_SIZE, '', 'mysql');
    const sql3 = buildSql('mydb', 'users', 3, PAGE_SIZE, '', 'mysql');
    expect(sql1).not.toContain('LIMIT');
    expect(sql1).not.toContain('OFFSET');
    expect(sql2).not.toContain('LIMIT');
    expect(sql3).not.toContain('LIMIT');
  });

  it('uses double-quote identifiers for postgres', () => {
    const sql = buildSql('prod', 'orders', 1, PAGE_SIZE, '', 'postgres');
    expect(sql).toContain('"prod"."orders"');
  });

  it('uses backtick identifiers for mysql', () => {
    const sql = buildSql('prod', 'orders', 1, PAGE_SIZE, '', 'mysql');
    expect(sql).toContain('`prod`.`orders`');
  });

  it('uses backtick identifiers for mariadb', () => {
    const sql = buildSql('prod', 'orders', 1, PAGE_SIZE, '', 'mariadb');
    expect(sql).toContain('`prod`.`orders`');
  });

  it('does not include LIMIT (pagination handled by backend)', () => {
    const sql = buildSql('mydb', 'users', 1, PAGE_SIZE, '', 'mysql');
    expect(sql).not.toContain('LIMIT');
  });
});
