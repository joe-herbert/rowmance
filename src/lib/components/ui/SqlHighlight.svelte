<script lang="ts">
  type TokenType = 'keyword' | 'string' | 'comment' | 'number' | 'operator' | 'default';

  interface Token {
    text: string;
    type: TokenType;
  }

  interface Props {
    sql: string;
    class?: string;
  }

  let { sql, class: className = '' }: Props = $props();

  const KEYWORDS = new Set([
    'SELECT', 'FROM', 'WHERE', 'JOIN', 'LEFT', 'RIGHT', 'INNER', 'OUTER', 'CROSS', 'FULL',
    'ON', 'AND', 'OR', 'NOT', 'IN', 'LIKE', 'ILIKE', 'BETWEEN', 'IS', 'NULL', 'AS',
    'GROUP', 'ORDER', 'BY', 'HAVING', 'LIMIT', 'OFFSET', 'DISTINCT', 'ALL', 'TOP',
    'INSERT', 'INTO', 'VALUES', 'UPDATE', 'SET', 'DELETE',
    'CREATE', 'TABLE', 'VIEW', 'DROP', 'ALTER', 'ADD', 'COLUMN', 'INDEX', 'REPLACE',
    'BEGIN', 'COMMIT', 'ROLLBACK', 'TRANSACTION', 'SAVEPOINT',
    'CASE', 'WHEN', 'THEN', 'ELSE', 'END',
    'EXISTS', 'ANY', 'SOME', 'WITH', 'UNION', 'INTERSECT', 'EXCEPT',
    'RETURNING', 'EXPLAIN', 'TRUNCATE', 'DESCRIBE', 'SHOW', 'USE',
    'PRIMARY', 'KEY', 'FOREIGN', 'REFERENCES', 'UNIQUE', 'CONSTRAINT',
    'ASC', 'DESC', 'NULLS', 'FIRST', 'LAST',
    'CAST', 'COALESCE', 'NULLIF', 'IF', 'IFNULL', 'OVER', 'PARTITION',
    'TRUE', 'FALSE', 'DEFAULT', 'AUTO_INCREMENT', 'AUTOINCREMENT',
    'INNER', 'NATURAL', 'USING', 'LATERAL',
  ]);

  function tokenize(input: string): Token[] {
    const out: Token[] = [];
    let i = 0;

    while (i < input.length) {
      const ch = input[i];
      const ch2 = input[i + 1];

      // Line comment
      if (ch === '-' && ch2 === '-') {
        const end = input.indexOf('\n', i);
        const text = end === -1 ? input.slice(i) : input.slice(i, end + 1);
        out.push({ text, type: 'comment' });
        i += text.length;
        continue;
      }

      // Block comment
      if (ch === '/' && ch2 === '*') {
        const end = input.indexOf('*/', i + 2);
        const text = end === -1 ? input.slice(i) : input.slice(i, end + 2);
        out.push({ text, type: 'comment' });
        i += text.length;
        continue;
      }

      // Single-quoted string (with '' escaping)
      if (ch === "'") {
        let j = i + 1;
        while (j < input.length) {
          if (input[j] === "'" && input[j + 1] === "'") { j += 2; continue; }
          if (input[j] === "'") { j++; break; }
          j++;
        }
        out.push({ text: input.slice(i, j), type: 'string' });
        i = j;
        continue;
      }

      // Double-quoted string/identifier
      if (ch === '"') {
        let j = i + 1;
        while (j < input.length) {
          if (input[j] === '"' && input[j + 1] === '"') { j += 2; continue; }
          if (input[j] === '"') { j++; break; }
          j++;
        }
        out.push({ text: input.slice(i, j), type: 'string' });
        i = j;
        continue;
      }

      // Backtick identifier — not a keyword, render as default
      if (ch === '`') {
        let j = i + 1;
        while (j < input.length && input[j] !== '`') j++;
        if (j < input.length) j++;
        out.push({ text: input.slice(i, j), type: 'default' });
        i = j;
        continue;
      }

      // Number (integer or decimal; don't consume leading - as that's an operator)
      if (/\d/.test(ch) || (ch === '.' && /\d/.test(input[i + 1] ?? ''))) {
        let j = i;
        while (j < input.length && /[\d.eE]/.test(input[j])) j++;
        out.push({ text: input.slice(i, j), type: 'number' });
        i = j;
        continue;
      }

      // Word → keyword or identifier
      if (/[a-zA-Z_$]/.test(ch)) {
        let j = i;
        while (j < input.length && /[\w$]/.test(input[j])) j++;
        const word = input.slice(i, j);
        out.push({ text: word, type: KEYWORDS.has(word.toUpperCase()) ? 'keyword' : 'default' });
        i = j;
        continue;
      }

      // Operators (multi-char first)
      if (/[=<>!|+\-*/%~&^]/.test(ch)) {
        let len = 1;
        const pair = ch + ch2;
        if (['!=', '<>', '<=', '>=', '||', '->'].includes(pair)) len = 2;
        if (input.slice(i, i + 3) === '->>') len = 3;
        out.push({ text: input.slice(i, i + len), type: 'operator' });
        i += len;
        continue;
      }

      // Everything else: whitespace, punctuation
      out.push({ text: ch, type: 'default' });
      i++;
    }

    return out;
  }

  let tokens = $derived(tokenize(sql));
</script>

<span class="sql-hl {className}"
  >{#each tokens as token}<span class="tok tok-{token.type}">{token.text}</span>{/each}</span
>

<style>
  .sql-hl {
    font-family: var(--font-family-mono);
  }

  .tok-keyword {
    color: var(--color-editor-keyword);
  }

  .tok-string {
    color: var(--color-editor-string);
  }

  .tok-comment {
    color: var(--color-editor-comment);
    font-style: italic;
  }

  .tok-number {
    color: var(--color-editor-number);
  }

  .tok-operator {
    color: var(--color-editor-operator);
  }

  .tok-default {
    color: inherit;
  }
</style>
