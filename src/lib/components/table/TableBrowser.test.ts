import { describe, it, expect } from 'vitest';

// Pure functions redefined locally to avoid rune compilation issues in test context.
// These mirror the inline functions in TableBrowser.svelte, adapted to take
// dbType as a parameter instead of reading it from component state.

const _PAGE_SIZE = 50;

function quoteIdentifier(name: string, dbType: string): string {
  return dbType === 'postgres' ? `"${name}"` : `\`${name}\``;
}

// ── FilterEditorState types (mirrored from FilterEditor.svelte) ───────────────

type FilterOperator =
  | '='
  | '!='
  | '>'
  | '<'
  | '>='
  | '<='
  | 'LIKE'
  | 'NOT LIKE'
  | 'IS NULL'
  | 'IS NOT NULL'
  | 'IN';

interface FilterRule {
  id: string;
  rawSql?: string;
  column: string;
  operator: FilterOperator;
  value: string;
}

interface FilterGroup {
  id: string;
  conjunction: 'AND' | 'OR';
  rules: FilterRule[];
}

interface FilterEditorState {
  mode: 'builder' | 'sql';
  groupJunction: 'AND' | 'OR';
  groups: FilterGroup[];
  sql: string;
}

function emptyFilterState(): FilterEditorState {
  return { mode: 'builder', groupJunction: 'AND', groups: [], sql: '' };
}

function isActiveRule(r: FilterRule): boolean {
  if (r.rawSql !== undefined) return r.rawSql.trim() !== '';
  return (
    r.column !== '' &&
    (r.operator === 'IS NULL' || r.operator === 'IS NOT NULL' || r.value.trim() !== '')
  );
}

function buildRuleSql(r: FilterRule, quoteId: (n: string) => string): string {
  if (r.rawSql !== undefined) return r.rawSql.trim();
  const col = quoteId(r.column);
  if (r.operator === 'IS NULL') return `${col} IS NULL`;
  if (r.operator === 'IS NOT NULL') return `${col} IS NOT NULL`;
  if (r.operator === 'IN') return `${col} IN (${r.value})`;
  const escaped = r.value.replaceAll("'", "''");
  return `${col} ${r.operator} '${escaped}'`;
}

function buildWhereClause(state: FilterEditorState, quoteId: (n: string) => string): string {
  if (state.mode === 'sql') return state.sql.trim();

  const activeGroups = state.groups
    .map((g) => ({
      conjunction: g.conjunction,
      parts: g.rules.filter(isActiveRule).map((r) => buildRuleSql(r, quoteId)),
    }))
    .filter((g) => g.parts.length > 0);

  if (activeGroups.length === 0) return '';

  const groupStrings = activeGroups.map((g) => {
    const joined = g.parts.join(` ${g.conjunction} `);
    return activeGroups.length > 1 && g.parts.length > 1 ? `(${joined})` : joined;
  });

  return groupStrings.join(` ${state.groupJunction} `);
}

function buildSql(
  database: string,
  table: string,
  filterState: FilterEditorState,
  dbType: string,
): string {
  const qi = (n: string) => quoteIdentifier(n, dbType);
  const quotedDb = qi(database);
  const quotedTable = qi(table);
  let base = `SELECT * FROM ${quotedDb}.${quotedTable}`;
  const where = buildWhereClause(filterState, qi);
  if (where) {
    base += ` WHERE ${where}`;
  }
  return base;
}

// Helper: build a single-group state with given rules
function singleGroup(
  rules: Omit<FilterRule, 'id'>[],
  conjunction: 'AND' | 'OR' = 'AND',
): FilterEditorState {
  return {
    mode: 'builder',
    groupJunction: 'AND',
    groups: [
      {
        id: '1',
        conjunction,
        rules: rules.map((r, i) => ({ id: String(i + 1), ...r })),
      },
    ],
    sql: '',
  };
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

// ── buildWhereClause tests ────────────────────────────────────────────────────

describe('buildWhereClause', () => {
  const qi = (n: string) => quoteIdentifier(n, 'mysql');
  const qiPg = (n: string) => quoteIdentifier(n, 'postgres');

  it('returns empty string for empty builder state', () => {
    expect(buildWhereClause(emptyFilterState(), qi)).toBe('');
  });

  it('returns empty string for SQL mode with empty sql', () => {
    expect(buildWhereClause({ mode: 'sql', groupJunction: 'AND', groups: [], sql: '' }, qi)).toBe(
      '',
    );
  });

  it('returns trimmed sql for SQL mode', () => {
    const state: FilterEditorState = {
      mode: 'sql',
      groupJunction: 'AND',
      groups: [],
      sql: '  age > 18  ',
    };
    expect(buildWhereClause(state, qi)).toBe('age > 18');
  });

  it('returns verbatim sql for SQL mode (no escaping)', () => {
    const state: FilterEditorState = {
      mode: 'sql',
      groupJunction: 'AND',
      groups: [],
      sql: "name = 'alice'",
    };
    expect(buildWhereClause(state, qi)).toBe("name = 'alice'");
  });

  it('builds a single equality rule for mysql', () => {
    expect(
      buildWhereClause(singleGroup([{ column: 'status', operator: '=', value: 'active' }]), qi),
    ).toBe("`status` = 'active'");
  });

  it('builds a single equality rule for postgres', () => {
    expect(
      buildWhereClause(singleGroup([{ column: 'status', operator: '=', value: 'active' }]), qiPg),
    ).toBe(`"status" = 'active'`);
  });

  it('handles IS NULL operator without value', () => {
    expect(
      buildWhereClause(singleGroup([{ column: 'deleted_at', operator: 'IS NULL', value: '' }]), qi),
    ).toBe('`deleted_at` IS NULL');
  });

  it('handles IS NOT NULL operator without value', () => {
    expect(
      buildWhereClause(singleGroup([{ column: 'email', operator: 'IS NOT NULL', value: '' }]), qi),
    ).toBe('`email` IS NOT NULL');
  });

  it('handles IN operator by wrapping value in parens', () => {
    expect(
      buildWhereClause(
        singleGroup([{ column: 'role', operator: 'IN', value: "'admin','user'" }]),
        qi,
      ),
    ).toBe("`role` IN ('admin','user')");
  });

  it('escapes single quotes in values', () => {
    expect(
      buildWhereClause(singleGroup([{ column: 'name', operator: '=', value: "O'Brien" }]), qi),
    ).toBe("`name` = 'O''Brien'");
  });

  it('combines two rules with AND in a single group', () => {
    expect(
      buildWhereClause(
        singleGroup(
          [
            { column: 'age', operator: '>', value: '18' },
            { column: 'status', operator: '=', value: 'active' },
          ],
          'AND',
        ),
        qi,
      ),
    ).toBe("`age` > '18' AND `status` = 'active'");
  });

  it('combines two rules with OR in a single group', () => {
    expect(
      buildWhereClause(
        singleGroup(
          [
            { column: 'age', operator: '<', value: '18' },
            { column: 'age', operator: '>', value: '65' },
          ],
          'OR',
        ),
        qi,
      ),
    ).toBe("`age` < '18' OR `age` > '65'");
  });

  it('skips rules with empty column', () => {
    expect(
      buildWhereClause(
        singleGroup([
          { column: '', operator: '=', value: 'something' },
          { column: 'status', operator: '=', value: 'active' },
        ]),
        qi,
      ),
    ).toBe("`status` = 'active'");
  });

  it('skips rules with empty value (except IS NULL / IS NOT NULL)', () => {
    expect(
      buildWhereClause(
        singleGroup([
          { column: 'name', operator: '=', value: '' },
          { column: 'status', operator: '=', value: 'active' },
        ]),
        qi,
      ),
    ).toBe("`status` = 'active'");
  });

  it('returns empty string when all rules have empty columns', () => {
    expect(buildWhereClause(singleGroup([{ column: '', operator: '=', value: 'foo' }]), qi)).toBe(
      '',
    );
  });

  it('handles LIKE operator', () => {
    expect(
      buildWhereClause(
        singleGroup([{ column: 'email', operator: 'LIKE', value: '%@example.com' }]),
        qi,
      ),
    ).toBe("`email` LIKE '%@example.com'");
  });

  it('handles NOT LIKE operator', () => {
    expect(
      buildWhereClause(
        singleGroup([{ column: 'email', operator: 'NOT LIKE', value: '%@spam.com' }]),
        qi,
      ),
    ).toBe("`email` NOT LIKE '%@spam.com'");
  });

  it('joins two single-rule groups with AND', () => {
    const state: FilterEditorState = {
      mode: 'builder',
      groupJunction: 'AND',
      groups: [
        {
          id: '1',
          conjunction: 'AND',
          rules: [{ id: 'r1', column: 'status', operator: '=', value: 'active' }],
        },
        {
          id: '2',
          conjunction: 'AND',
          rules: [{ id: 'r2', column: 'age', operator: '>', value: '18' }],
        },
      ],
      sql: '',
    };
    expect(buildWhereClause(state, qi)).toBe("`status` = 'active' AND `age` > '18'");
  });

  it('joins two multi-rule groups with AND and wraps each in parens', () => {
    const state: FilterEditorState = {
      mode: 'builder',
      groupJunction: 'AND',
      groups: [
        {
          id: '1',
          conjunction: 'OR',
          rules: [
            { id: 'r1', column: 'client_id', operator: '=', value: 'crm' },
            { id: 'r2', column: 'client_id', operator: '=', value: 'dms' },
          ],
        },
        {
          id: '2',
          conjunction: 'AND',
          rules: [{ id: 'r3', column: 'name', operator: '=', value: 'CRM' }],
        },
      ],
      sql: '',
    };
    expect(buildWhereClause(state, qi)).toBe(
      "(`client_id` = 'crm' OR `client_id` = 'dms') AND `name` = 'CRM'",
    );
  });

  it('joins two multi-rule groups with OR', () => {
    const state: FilterEditorState = {
      mode: 'builder',
      groupJunction: 'OR',
      groups: [
        {
          id: '1',
          conjunction: 'AND',
          rules: [
            { id: 'r1', column: 'type', operator: '=', value: 'admin' },
            { id: 'r2', column: 'active', operator: '=', value: '1' },
          ],
        },
        {
          id: '2',
          conjunction: 'AND',
          rules: [
            { id: 'r3', column: 'type', operator: '=', value: 'owner' },
            { id: 'r4', column: 'active', operator: '=', value: '1' },
          ],
        },
      ],
      sql: '',
    };
    expect(buildWhereClause(state, qi)).toBe(
      "(`type` = 'admin' AND `active` = '1') OR (`type` = 'owner' AND `active` = '1')",
    );
  });

  it('does not wrap a single-rule group in parens even when there are multiple groups', () => {
    const state: FilterEditorState = {
      mode: 'builder',
      groupJunction: 'AND',
      groups: [
        {
          id: '1',
          conjunction: 'AND',
          rules: [{ id: 'r1', column: 'a', operator: '=', value: '1' }],
        },
        {
          id: '2',
          conjunction: 'AND',
          rules: [{ id: 'r2', column: 'b', operator: '=', value: '2' }],
        },
      ],
      sql: '',
    };
    expect(buildWhereClause(state, qi)).toBe("`a` = '1' AND `b` = '2'");
  });
});

// ── buildSql tests ────────────────────────────────────────────────────────────

describe('buildSql', () => {
  it('produces a basic SELECT with no filter for mysql', () => {
    const sql = buildSql('mydb', 'users', emptyFilterState(), 'mysql');
    expect(sql).toBe('SELECT * FROM `mydb`.`users`');
  });

  it('produces a basic SELECT with no filter for postgres', () => {
    const sql = buildSql('mydb', 'users', emptyFilterState(), 'postgres');
    expect(sql).toBe('SELECT * FROM "mydb"."users"');
  });

  it('includes a WHERE clause when SQL filter is non-empty', () => {
    const state: FilterEditorState = {
      mode: 'sql',
      groupJunction: 'AND',
      groups: [],
      sql: 'age > 18',
    };
    const sql = buildSql('mydb', 'users', state, 'mysql');
    expect(sql).toBe('SELECT * FROM `mydb`.`users` WHERE age > 18');
  });

  it('trims whitespace from SQL filter', () => {
    const state: FilterEditorState = {
      mode: 'sql',
      groupJunction: 'AND',
      groups: [],
      sql: '  age > 18  ',
    };
    const sql = buildSql('mydb', 'users', state, 'mysql');
    expect(sql).toBe('SELECT * FROM `mydb`.`users` WHERE age > 18');
  });

  it('does not include WHERE clause when SQL filter is only whitespace', () => {
    const state: FilterEditorState = { mode: 'sql', groupJunction: 'AND', groups: [], sql: '   ' };
    const sql = buildSql('mydb', 'users', state, 'mysql');
    expect(sql).not.toContain('WHERE');
  });

  it('includes WHERE clause from builder rules', () => {
    const sql = buildSql(
      'mydb',
      'users',
      singleGroup([{ column: 'status', operator: '=', value: 'active' }]),
      'mysql',
    );
    expect(sql).toBe("SELECT * FROM `mydb`.`users` WHERE `status` = 'active'");
  });

  it('does not include LIMIT or OFFSET (pagination is handled by the backend)', () => {
    const sql = buildSql('mydb', 'users', emptyFilterState(), 'mysql');
    expect(sql).not.toContain('LIMIT');
    expect(sql).not.toContain('OFFSET');
  });

  it('uses double-quote identifiers for postgres', () => {
    const sql = buildSql('prod', 'orders', emptyFilterState(), 'postgres');
    expect(sql).toContain('"prod"."orders"');
  });

  it('uses backtick identifiers for mysql', () => {
    const sql = buildSql('prod', 'orders', emptyFilterState(), 'mysql');
    expect(sql).toContain('`prod`.`orders`');
  });

  it('uses backtick identifiers for mariadb', () => {
    const sql = buildSql('prod', 'orders', emptyFilterState(), 'mariadb');
    expect(sql).toContain('`prod`.`orders`');
  });
});
