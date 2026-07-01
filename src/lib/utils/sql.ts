/**
 * SQL text utilities used by the query editor.
 * These are pure functions with no side effects, making them straightforward to test.
 */

/**
 * Split a SQL document into individual statements by splitting on semicolons,
 * while respecting single-quoted strings, double-quoted identifiers,
 * backtick-quoted identifiers, and both single-line (--) and block comments (/ * ... * /).
 *
 * Returns an array of trimmed, non-empty statement strings.
 */
export function splitStatements(sql: string): string[] {
  const statements: string[] = [];
  let current = '';
  let i = 0;

  while (i < sql.length) {
    const char = sql[i];
    const next = sql[i + 1];

    // Block comment: /* ... */
    if (char === '/' && next === '*') {
      const end = sql.indexOf('*/', i + 2);
      if (end === -1) {
        current += sql.slice(i);
        break;
      }
      current += sql.slice(i, end + 2);
      i = end + 2;
      continue;
    }

    // Single-line comment: -- ... \n
    if (char === '-' && next === '-') {
      const end = sql.indexOf('\n', i);
      if (end === -1) {
        current += sql.slice(i);
        break;
      }
      current += sql.slice(i, end + 1);
      i = end + 1;
      continue;
    }

    // Quoted strings: ' ... ' (with '' escape)
    if (char === "'") {
      let j = i + 1;
      while (j < sql.length) {
        if (sql[j] === "'" && sql[j + 1] === "'") {
          j += 2; // escaped quote
        } else if (sql[j] === "'") {
          j++;
          break;
        } else {
          j++;
        }
      }
      current += sql.slice(i, j);
      i = j;
      continue;
    }

    // Double-quoted identifiers: " ... "
    if (char === '"') {
      let j = i + 1;
      while (j < sql.length && sql[j] !== '"') j++;
      current += sql.slice(i, j + 1);
      i = j + 1;
      continue;
    }

    // Backtick-quoted identifiers: ` ... `
    if (char === '`') {
      let j = i + 1;
      while (j < sql.length && sql[j] !== '`') j++;
      current += sql.slice(i, j + 1);
      i = j + 1;
      continue;
    }

    // Statement delimiter
    if (char === ';') {
      const trimmed = current.trim();
      if (trimmed.length > 0) {
        statements.push(trimmed);
      }
      current = '';
      i++;
      continue;
    }

    current += char;
    i++;
  }

  // Capture any trailing statement without a trailing semicolon
  const trailing = current.trim();
  if (trailing.length > 0) {
    statements.push(trailing);
  }

  return statements;
}

/**
 * Find the SQL statement that contains the given cursor position (character offset).
 * Returns the statement string, or the full document if it cannot be determined.
 *
 * Used by the "run under cursor" editor action.
 */
export function statementAtCursor(sql: string, cursorPos: number): string {
  // Find statement boundaries by locating semicolons outside quoted regions.
  const boundaries: number[] = [-1]; // start sentinel
  let i = 0;

  while (i < sql.length) {
    const char = sql[i];
    const next = sql[i + 1];

    if (char === '/' && next === '*') {
      const end = sql.indexOf('*/', i + 2);
      i = end === -1 ? sql.length : end + 2;
      continue;
    }
    if (char === '-' && next === '-') {
      const end = sql.indexOf('\n', i);
      i = end === -1 ? sql.length : end + 1;
      continue;
    }
    if (char === "'") {
      let j = i + 1;
      while (j < sql.length) {
        if (sql[j] === "'" && sql[j + 1] === "'") j += 2;
        else if (sql[j] === "'") {
          j++;
          break;
        } else j++;
      }
      i = j;
      continue;
    }
    if (char === '"' || char === '`') {
      const close = char;
      let j = i + 1;
      while (j < sql.length && sql[j] !== close) j++;
      i = j + 1;
      continue;
    }
    if (char === ';') {
      boundaries.push(i);
    }
    i++;
  }
  boundaries.push(sql.length); // end sentinel

  // Find the segment whose range contains cursorPos.
  for (let b = 0; b < boundaries.length - 1; b++) {
    const start = boundaries[b] + 1;
    const end = boundaries[b + 1];
    if (cursorPos >= start && cursorPos <= end) {
      return sql.slice(start, end).trim();
    }
  }

  return sql.trim();
}

/**
 * Return whether the first keyword of a SQL statement is a write operation.
 * Used to show an early error in the UI before sending to the backend.
 * The backend enforces read-only at the database connection level, which
 * covers cases this heuristic misses (CTEs, CALL, EXEC, etc.).
 */
export function isMutatingStatement(sql: string): boolean {
  const keyword = sql.trim().split(/\s+/)[0]?.toUpperCase();
  return [
    'INSERT',
    'UPDATE',
    'DELETE',
    'DROP',
    'CREATE',
    'ALTER',
    'TRUNCATE',
    'REPLACE',
    'MERGE',
    'RENAME',
  ].includes(keyword ?? '');
}
