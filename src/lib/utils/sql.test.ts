import { describe, it, expect } from 'vitest';
import { splitStatements, statementAtCursor, isMutatingStatement } from './sql';

describe('splitStatements', () => {
  it('splits two simple statements', () => {
    const result = splitStatements('SELECT 1; SELECT 2;');
    expect(result).toEqual(['SELECT 1', 'SELECT 2']);
  });

  it('handles a trailing statement with no semicolon', () => {
    expect(splitStatements('SELECT 1')).toEqual(['SELECT 1']);
  });

  it('ignores semicolons inside single-quoted strings', () => {
    const result = splitStatements("SELECT 'a;b'; SELECT 2;");
    expect(result).toEqual(["SELECT 'a;b'", 'SELECT 2']);
  });

  it('ignores semicolons inside double-quoted identifiers', () => {
    const result = splitStatements('SELECT "col;name"; SELECT 2;');
    expect(result).toEqual(['SELECT "col;name"', 'SELECT 2']);
  });

  it('ignores semicolons inside backtick identifiers', () => {
    const result = splitStatements('SELECT `col;name`; SELECT 2;');
    expect(result).toEqual(['SELECT `col;name`', 'SELECT 2']);
  });

  it('ignores semicolons in single-line comments', () => {
    const result = splitStatements('-- SELECT 1;\nSELECT 2;');
    expect(result).toEqual(['-- SELECT 1;\nSELECT 2']);
  });

  it('ignores semicolons in block comments', () => {
    const result = splitStatements('/* SELECT 1; */ SELECT 2;');
    expect(result).toEqual(['/* SELECT 1; */ SELECT 2']);
  });

  it('handles escaped single quotes inside strings', () => {
    const result = splitStatements("SELECT 'it''s a test'; SELECT 2;");
    expect(result).toEqual(["SELECT 'it''s a test'", 'SELECT 2']);
  });

  it('returns an empty array for blank input', () => {
    expect(splitStatements('   ')).toEqual([]);
    expect(splitStatements('')).toEqual([]);
  });

  it('strips empty statements between consecutive semicolons', () => {
    expect(splitStatements('SELECT 1;;SELECT 2;')).toEqual(['SELECT 1', 'SELECT 2']);
  });
});

describe('statementAtCursor', () => {
  const sql = 'SELECT 1; SELECT 2; SELECT 3';

  it('finds the first statement when cursor is at the start', () => {
    expect(statementAtCursor(sql, 0)).toBe('SELECT 1');
  });

  it('finds the second statement when cursor is inside it', () => {
    // Cursor at position 12, inside 'SELECT 2'
    expect(statementAtCursor(sql, 12)).toBe('SELECT 2');
  });

  it('finds the last statement with no trailing semicolon', () => {
    expect(statementAtCursor(sql, sql.length)).toBe('SELECT 3');
  });
});

describe('isMutatingStatement', () => {
  it.each([
    ['INSERT INTO t VALUES (1)', true],
    ['UPDATE t SET a = 1', true],
    ['DELETE FROM t', true],
    ['DROP TABLE t', true],
    ['CREATE TABLE t (id INT)', true],
    ['ALTER TABLE t ADD COLUMN a INT', true],
    ['TRUNCATE TABLE t', true],
    ['select * from t', false],
    ['  SELECT id FROM users', false],
    ['EXPLAIN SELECT 1', false],
  ])('"%s" → %s', (sql, expected) => {
    expect(isMutatingStatement(sql)).toBe(expected);
  });
});
