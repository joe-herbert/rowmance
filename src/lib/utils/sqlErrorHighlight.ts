import type { Diagnostic } from '@codemirror/lint';

const SQL_KEYWORDS = [
  'SELECT', 'FROM', 'WHERE', 'JOIN', 'INNER', 'LEFT', 'RIGHT', 'OUTER', 'FULL', 'CROSS',
  'ON', 'GROUP', 'ORDER', 'HAVING', 'LIMIT', 'OFFSET', 'DISTINCT', 'ALL',
  'INSERT', 'INTO', 'VALUES', 'UPDATE', 'SET', 'DELETE',
  'CREATE', 'TABLE', 'ALTER', 'DROP', 'INDEX', 'VIEW', 'DATABASE', 'SCHEMA',
  'AND', 'OR', 'NOT', 'IN', 'EXISTS', 'BETWEEN', 'LIKE', 'IS', 'NULL',
  'AS', 'WITH', 'UNION', 'INTERSECT', 'EXCEPT', 'CASE', 'WHEN', 'THEN', 'ELSE', 'END',
  'BEGIN', 'COMMIT', 'ROLLBACK', 'TRANSACTION',
  'SHOW', 'DESCRIBE', 'EXPLAIN', 'USE', 'TRUNCATE', 'RETURNING',
];

function levenshtein(a: string, b: string): number {
  const m = a.length;
  const n = b.length;
  const dp: number[] = Array.from({ length: n + 1 }, (_, j) => j);
  for (let i = 1; i <= m; i++) {
    let prev = dp[0];
    dp[0] = i;
    for (let j = 1; j <= n; j++) {
      const tmp = dp[j];
      dp[j] = a[i - 1] === b[j - 1] ? prev : 1 + Math.min(prev, dp[j], dp[j - 1]);
      prev = tmp;
    }
  }
  return dp[n];
}

function suggestKeyword(token: string): string | null {
  const upper = token.toUpperCase();
  if (SQL_KEYWORDS.includes(upper)) return null;
  if (upper.length < 3) return null;

  let best: string | null = null;
  let bestDist = Infinity;
  const maxDist = Math.max(1, Math.floor(upper.length * 0.4));
  for (const kw of SQL_KEYWORDS) {
    const dist = levenshtein(upper, kw);
    if (dist < bestDist && dist <= maxDist) {
      bestDist = dist;
      best = kw;
    }
  }
  return best;
}

function lineToOffset(text: string, line: number): number {
  let current = 1;
  let i = 0;
  while (i < text.length && current < line) {
    if (text[i] === '\n') current++;
    i++;
  }
  return i;
}

function findTokenInStatement(
  statement: string,
  token: string,
  lineHint?: number,
): { from: number; to: number } | null {
  if (!token) return null;

  const startSearch = lineHint !== undefined && lineHint > 1
    ? Math.max(0, lineToOffset(statement, lineHint) - 5)
    : 0;

  let idx = statement.indexOf(token, startSearch);
  if (idx !== -1) return { from: idx, to: idx + token.length };

  const lowerStmt = statement.toLowerCase();
  idx = lowerStmt.indexOf(token.toLowerCase(), startSearch);
  if (idx !== -1) return { from: idx, to: idx + token.length };

  return null;
}

interface ParsedError {
  token?: string;
  line?: number;
  suggestion?: string;
}

function parseMysqlError(msg: string): ParsedError {
  const nearLine = msg.match(/near ['"](.+?)['"]\s+at line (\d+)/i);
  if (nearLine) {
    const token = nearLine[1];
    const line = parseInt(nearLine[2], 10);
    const firstWord = token.split(/\s+/)[0];
    const kw = suggestKeyword(firstWord);
    return { token, line, suggestion: kw ? `Did you mean "${kw}"?` : undefined };
  }

  const nearOnly = msg.match(/near ['"](.+?)['"]/i);
  if (nearOnly) {
    const token = nearOnly[1];
    const kw = suggestKeyword(token.split(/\s+/)[0]);
    return { token, suggestion: kw ? `Did you mean "${kw}"?` : undefined };
  }

  const unknownCol = msg.match(/Unknown column ['"](.+?)['"]/i);
  if (unknownCol) return { token: unknownCol[1] };

  const tableNotExist = msg.match(/Table '(.+?)' doesn't exist/i);
  if (tableNotExist) {
    const parts = tableNotExist[1].split('.');
    return { token: parts[parts.length - 1] };
  }

  return {};
}

function parsePostgresError(msg: string): ParsedError {
  const syntaxAt = msg.match(/syntax error at or near "(.+?)"/i);
  if (syntaxAt) {
    const token = syntaxAt[1];
    const lineMatch = msg.match(/LINE (\d+):/);
    const line = lineMatch ? parseInt(lineMatch[1], 10) : undefined;
    const kw = suggestKeyword(token);
    return { token, line, suggestion: kw ? `Did you mean "${kw}"?` : undefined };
  }

  const colNotExist = msg.match(/column "(.+?)" does not exist/i);
  if (colNotExist) return { token: colNotExist[1] };

  const relNotExist = msg.match(/relation "(.+?)" does not exist/i);
  if (relNotExist) return { token: relNotExist[1] };

  const fnNotExist = msg.match(/function (.+?) does not exist/i);
  if (fnNotExist) return { token: fnNotExist[1].split('(')[0] };

  return {};
}

function parseSqliteError(msg: string): ParsedError {
  const near = msg.match(/near "(.+?)":\s*syntax error/i);
  if (near) {
    const token = near[1];
    const kw = suggestKeyword(token);
    return { token, suggestion: kw ? `Did you mean "${kw}"?` : undefined };
  }
  return {};
}

function parseError(msg: string): ParsedError {
  if (/at line \d+|Unknown column|doesn't exist/.test(msg)) return parseMysqlError(msg);
  if (/LINE \d+:|does not exist|syntax error at or near/.test(msg)) return parsePostgresError(msg);
  if (/: syntax error/.test(msg)) return parseSqliteError(msg);
  // Try all parsers in order
  return parseMysqlError(msg) || parsePostgresError(msg) || parseSqliteError(msg);
}

export interface ErrorInput {
  error: string;
  statement: string;
  statementOffset: number;
}

export function buildDiagnosticsFromErrors(errors: ErrorInput[]): Diagnostic[] {
  const diagnostics: Diagnostic[] = [];

  for (const { error, statement, statementOffset } of errors) {
    const parsed = parseError(error);
    let from: number;
    let to: number;

    if (parsed.token) {
      const pos = findTokenInStatement(statement, parsed.token, parsed.line);
      if (pos) {
        from = statementOffset + pos.from;
        to = statementOffset + pos.to;
      } else if (parsed.line !== undefined) {
        const lineStart = lineToOffset(statement, parsed.line);
        const lineEnd = statement.indexOf('\n', lineStart);
        from = statementOffset + lineStart;
        to = statementOffset + (lineEnd === -1 ? statement.length : lineEnd);
      } else {
        from = statementOffset;
        to = statementOffset + statement.trimEnd().length;
      }
    } else if (parsed.line !== undefined) {
      const lineStart = lineToOffset(statement, parsed.line);
      const lineEnd = statement.indexOf('\n', lineStart);
      from = statementOffset + lineStart;
      to = statementOffset + (lineEnd === -1 ? statement.length : lineEnd);
    } else {
      from = statementOffset;
      to = statementOffset + statement.trimEnd().length;
    }

    const message = parsed.suggestion ? `${error}\n\nSuggestion: ${parsed.suggestion}` : error;

    diagnostics.push({ from, to, severity: 'error', message });
  }

  return diagnostics;
}
