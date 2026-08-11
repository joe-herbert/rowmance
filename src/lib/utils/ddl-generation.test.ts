import { describe, it, expect } from 'vitest';
import { defaultDialectInfo } from '$lib/utils/dialect';
import type {
  DialectInfo,
  ColumnInfo,
  IndexInfo,
  CheckConstraintInfo,
  TriggerInfo,
} from '$lib/types';
import {
  colDef,
  buildAddColSqls,
  buildEditColSqls,
  buildDropColSql,
  buildAddIdxSql,
  buildDropIdxSql,
  buildAddFkSql,
  buildDropFkSql,
  buildAddCheckSql,
  buildDropCheckSql,
  buildTriggerSyncSql,
  buildCreateTableSql,
  generateSyncSql,
  type ColForm,
  type IdxForm,
  type FkForm,
} from './ddl-generation';
import type { SchemaDiff, ObjectDiff } from '$lib/utils/schema-diff';

const mysqlDialect: DialectInfo = {
  ...defaultDialectInfo,
  displayName: 'MySQL',
  identifierOpen: '`',
  identifierClose: '`',
  identifierEscape: '``',
  usesSchema: true,
  supportsAutoIncrement: true,
  supportsColumnComment: true,
  supportsChangeColumn: true,
  supportsRenameColumn: false,
  usesForeignKeyKeyword: true,
  dropIndexSyntax: 'on_table',
};

const postgresDialect: DialectInfo = {
  ...defaultDialectInfo,
  displayName: 'PostgreSQL',
  identifierOpen: '"',
  identifierClose: '"',
  identifierEscape: '""',
  usesSchema: true,
  supportsAutoIncrement: false,
  supportsColumnComment: false,
  supportsChangeColumn: false,
  supportsRenameColumn: true,
  usesForeignKeyKeyword: false,
  dropIndexSyntax: 'schema_qualified',
};

const sqliteDialect: DialectInfo = {
  ...defaultDialectInfo,
  displayName: 'SQLite',
  identifierOpen: '"',
  identifierClose: '"',
  identifierEscape: '""',
  usesSchema: false,
  supportsAutoIncrement: false,
  supportsColumnComment: false,
  supportsChangeColumn: false,
  supportsRenameColumn: true,
  usesForeignKeyKeyword: false,
  dropIndexSyntax: 'simple',
};

const sqlServerDialect: DialectInfo = {
  ...defaultDialectInfo,
  displayName: 'SQL Server',
  identifierOpen: '[',
  identifierClose: ']',
  identifierEscape: ']]',
  usesSchema: true,
  supportsAutoIncrement: false,
  supportsColumnComment: false,
  supportsChangeColumn: false,
  supportsRenameColumn: false,
  usesForeignKeyKeyword: false,
  dropIndexSyntax: 'on_table_no_schema',
};

function makeColForm(overrides: Partial<ColForm> = {}): ColForm {
  return {
    mode: 'add',
    original: null,
    name: 'age',
    dataType: 'INT',
    nullable: true,
    defaultValue: '',
    autoIncrement: false,
    comment: '',
    ...overrides,
  };
}

function makeColumnInfo(overrides: Partial<ColumnInfo> = {}): ColumnInfo {
  return {
    name: 'age',
    dataType: 'INT',
    nullable: true,
    defaultValue: null,
    isPrimaryKey: false,
    isAutoIncrement: false,
    isForeignKey: false,
    comment: null,
    ...overrides,
  };
}

describe('colDef', () => {
  it('renders a minimal nullable column', () => {
    expect(colDef(mysqlDialect, makeColForm())).toBe('`age` INT');
  });

  it('adds NOT NULL when not nullable', () => {
    expect(colDef(mysqlDialect, makeColForm({ nullable: false }))).toBe('`age` INT NOT NULL');
  });

  it('adds AUTO_INCREMENT when dialect and form both support it', () => {
    expect(colDef(mysqlDialect, makeColForm({ autoIncrement: true }))).toBe(
      '`age` INT AUTO_INCREMENT',
    );
  });

  it('omits AUTO_INCREMENT when dialect does not support it', () => {
    expect(colDef(postgresDialect, makeColForm({ autoIncrement: true }))).toBe('"age" INT');
  });

  it('adds DEFAULT when provided', () => {
    expect(colDef(mysqlDialect, makeColForm({ defaultValue: '0' }))).toBe('`age` INT DEFAULT 0');
  });

  it('adds COMMENT when dialect supports it', () => {
    expect(colDef(mysqlDialect, makeColForm({ comment: "it's a note" }))).toBe(
      "`age` INT COMMENT 'it''s a note'",
    );
  });

  it('omits COMMENT when dialect does not support it', () => {
    expect(colDef(postgresDialect, makeColForm({ comment: 'note' }))).toBe('"age" INT');
  });
});

describe('buildAddColSqls', () => {
  it('builds ALTER TABLE ADD COLUMN with schema-qualified table ref', () => {
    expect(buildAddColSqls(mysqlDialect, 'mydb', 'users', makeColForm())).toEqual([
      'ALTER TABLE `mydb`.`users` ADD COLUMN `age` INT',
    ]);
  });

  it('omits schema prefix for schemaless dialects (SQLite)', () => {
    expect(buildAddColSqls(sqliteDialect, 'mydb', 'users', makeColForm())).toEqual([
      'ALTER TABLE "users" ADD COLUMN "age" INT',
    ]);
  });
});

describe('buildEditColSqls', () => {
  it('uses CHANGE COLUMN when dialect supports it (MySQL)', () => {
    const orig = makeColumnInfo({ name: 'age', dataType: 'INT' });
    const form = makeColForm({ mode: 'edit', original: orig, dataType: 'BIGINT' });
    expect(buildEditColSqls(mysqlDialect, 'mydb', 'users', orig, form)).toEqual([
      'ALTER TABLE `mydb`.`users` CHANGE COLUMN `age` `age` BIGINT',
    ]);
  });

  it('builds per-field ALTER statements when supportsChangeColumn is false (Postgres)', () => {
    const orig = makeColumnInfo({
      name: 'age',
      dataType: 'INT',
      nullable: true,
      defaultValue: null,
    });
    const form = makeColForm({
      mode: 'edit',
      original: orig,
      dataType: 'BIGINT',
      nullable: false,
      defaultValue: '0',
    });
    expect(buildEditColSqls(postgresDialect, 'mydb', 'users', orig, form)).toEqual([
      'ALTER TABLE "mydb"."users" ALTER COLUMN "age" TYPE BIGINT',
      'ALTER TABLE "mydb"."users" ALTER COLUMN "age" SET NOT NULL',
      'ALTER TABLE "mydb"."users" ALTER COLUMN "age" SET DEFAULT 0',
    ]);
  });

  it('drops the default when cleared', () => {
    const orig = makeColumnInfo({ name: 'age', defaultValue: '0' });
    const form = makeColForm({ mode: 'edit', original: orig, defaultValue: '' });
    expect(buildEditColSqls(postgresDialect, 'mydb', 'users', orig, form)).toEqual([
      'ALTER TABLE "mydb"."users" ALTER COLUMN "age" DROP DEFAULT',
    ]);
  });

  it('renames the column only when dialect supports rename', () => {
    const orig = makeColumnInfo({ name: 'age' });
    const form = makeColForm({ mode: 'edit', original: orig, name: 'years' });
    expect(buildEditColSqls(postgresDialect, 'mydb', 'users', orig, form)).toEqual([
      'ALTER TABLE "mydb"."users" RENAME COLUMN "age" TO "years"',
    ]);
  });

  it('omits the rename statement when dialect does not support it (MySQL, non-changecolumn branch is unreachable, verify no rename in supportsRenameColumn=false dialect)', () => {
    const noRenameDialect: DialectInfo = {
      ...postgresDialect,
      supportsChangeColumn: false,
      supportsRenameColumn: false,
    };
    const orig = makeColumnInfo({ name: 'age' });
    const form = makeColForm({ mode: 'edit', original: orig, name: 'years' });
    expect(buildEditColSqls(noRenameDialect, 'mydb', 'users', orig, form)).toEqual([]);
  });
});

describe('buildDropColSql', () => {
  it('builds a DROP COLUMN statement', () => {
    expect(buildDropColSql(mysqlDialect, 'mydb', 'users', 'age')).toBe(
      'ALTER TABLE `mydb`.`users` DROP COLUMN `age`',
    );
  });
});

describe('buildAddIdxSql', () => {
  const form: IdxForm = { name: '', selectedColumns: ['email'], unique: false, isPrimary: false };

  it('builds a CREATE INDEX statement with an auto-generated name', () => {
    expect(buildAddIdxSql(mysqlDialect, 'mydb', 'users', form)).toBe(
      'CREATE INDEX `idx_users_email` ON `mydb`.`users` (`email`)',
    );
  });

  it('builds a CREATE UNIQUE INDEX statement when unique is set', () => {
    expect(buildAddIdxSql(mysqlDialect, 'mydb', 'users', { ...form, unique: true })).toBe(
      'CREATE UNIQUE INDEX `idx_users_email` ON `mydb`.`users` (`email`)',
    );
  });

  it('uses a custom index name when provided', () => {
    expect(buildAddIdxSql(mysqlDialect, 'mydb', 'users', { ...form, name: 'my_idx' })).toBe(
      'CREATE INDEX `my_idx` ON `mydb`.`users` (`email`)',
    );
  });

  it('builds ADD PRIMARY KEY when isPrimary is set', () => {
    expect(buildAddIdxSql(mysqlDialect, 'mydb', 'users', { ...form, isPrimary: true })).toBe(
      'ALTER TABLE `mydb`.`users` ADD PRIMARY KEY (`email`)',
    );
  });
});

describe('buildDropIdxSql', () => {
  it('handles "on_table" syntax (MySQL)', () => {
    expect(buildDropIdxSql(mysqlDialect, 'mydb', 'users', 'idx_email')).toBe(
      'DROP INDEX `idx_email` ON `mydb`.`users`',
    );
  });

  it('handles PRIMARY specially for "on_table" syntax', () => {
    expect(buildDropIdxSql(mysqlDialect, 'mydb', 'users', 'PRIMARY')).toBe(
      'ALTER TABLE `mydb`.`users` DROP PRIMARY KEY',
    );
  });

  it('handles "schema_qualified" syntax (Postgres)', () => {
    expect(buildDropIdxSql(postgresDialect, 'mydb', 'users', 'idx_email')).toBe(
      'DROP INDEX "mydb"."idx_email"',
    );
  });

  it('handles "on_table_no_schema" syntax (SQL Server)', () => {
    expect(buildDropIdxSql(sqlServerDialect, 'mydb', 'users', 'idx_email')).toBe(
      'DROP INDEX [idx_email] ON [mydb].[users]',
    );
  });

  it('handles "simple" syntax (SQLite)', () => {
    expect(buildDropIdxSql(sqliteDialect, 'mydb', 'users', 'idx_email')).toBe(
      'DROP INDEX "idx_email"',
    );
  });
});

describe('buildAddFkSql', () => {
  const form: FkForm = {
    constraintName: '',
    selectedColumns: ['user_id'],
    referencedTable: 'users',
    referencedColumns: 'id',
    onDelete: 'CASCADE',
    onUpdate: 'NO ACTION',
  };

  it('builds an ADD CONSTRAINT ... FOREIGN KEY statement', () => {
    expect(buildAddFkSql(mysqlDialect, 'mydb', 'orders', form)).toBe(
      'ALTER TABLE `mydb`.`orders` ADD CONSTRAINT `fk_orders_user_id` FOREIGN KEY (`user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE ON UPDATE NO ACTION',
    );
  });

  it('supports multiple referenced columns', () => {
    expect(
      buildAddFkSql(mysqlDialect, 'mydb', 'orders', {
        ...form,
        referencedColumns: 'id, tenant_id',
      }),
    ).toContain('REFERENCES `users` (`id`, `tenant_id`)');
  });
});

describe('buildDropFkSql', () => {
  it('uses DROP FOREIGN KEY when usesForeignKeyKeyword is true (MySQL)', () => {
    expect(buildDropFkSql(mysqlDialect, 'mydb', 'orders', 'fk_orders_user_id')).toBe(
      'ALTER TABLE `mydb`.`orders` DROP FOREIGN KEY `fk_orders_user_id`',
    );
  });

  it('uses DROP CONSTRAINT when usesForeignKeyKeyword is false (Postgres)', () => {
    expect(buildDropFkSql(postgresDialect, 'mydb', 'orders', 'fk_orders_user_id')).toBe(
      'ALTER TABLE "mydb"."orders" DROP CONSTRAINT "fk_orders_user_id"',
    );
  });
});

describe('buildAddCheckSql', () => {
  it('builds an ADD CONSTRAINT ... CHECK statement with the raw expression', () => {
    expect(buildAddCheckSql(mysqlDialect, 'mydb', 'users', 'chk_age', 'age >= 0')).toBe(
      'ALTER TABLE `mydb`.`users` ADD CONSTRAINT `chk_age` CHECK (age >= 0)',
    );
  });

  it('does not re-quote the expression', () => {
    expect(
      buildAddCheckSql(postgresDialect, 'mydb', 'users', 'chk_age', "status IN ('a', 'b')"),
    ).toBe('ALTER TABLE "mydb"."users" ADD CONSTRAINT "chk_age" CHECK (status IN (\'a\', \'b\'))');
  });
});

describe('buildDropCheckSql', () => {
  it('uses DROP CHECK for MySQL', () => {
    expect(buildDropCheckSql(mysqlDialect, 'mydb', 'users', 'chk_age')).toBe(
      'ALTER TABLE `mydb`.`users` DROP CHECK `chk_age`',
    );
  });

  it('uses DROP CHECK for MariaDB', () => {
    const mariaDbDialect: DialectInfo = { ...mysqlDialect, displayName: 'MariaDB' };
    expect(buildDropCheckSql(mariaDbDialect, 'mydb', 'users', 'chk_age')).toBe(
      'ALTER TABLE `mydb`.`users` DROP CHECK `chk_age`',
    );
  });

  it('uses DROP CONSTRAINT for Postgres', () => {
    expect(buildDropCheckSql(postgresDialect, 'mydb', 'users', 'chk_age')).toBe(
      'ALTER TABLE "mydb"."users" DROP CONSTRAINT "chk_age"',
    );
  });

  it('uses DROP CONSTRAINT for SQL Server', () => {
    expect(buildDropCheckSql(sqlServerDialect, 'mydb', 'users', 'chk_age')).toBe(
      'ALTER TABLE [mydb].[users] DROP CONSTRAINT [chk_age]',
    );
  });
});

function makeTriggerInfo(overrides: Partial<TriggerInfo> = {}): TriggerInfo {
  return {
    name: 'trg_users_bi',
    tableName: 'users',
    timing: 'BEFORE',
    event: 'INSERT',
    definition: 'CREATE TRIGGER trg_users_bi BEFORE INSERT ON users FOR EACH ROW SET NEW.x = 1',
    ...overrides,
  };
}

describe('buildTriggerSyncSql', () => {
  it('builds a drop-then-create pair using the raw definition for same-engine sync', () => {
    const trigger = makeTriggerInfo();
    const sql = buildTriggerSyncSql(trigger, true, mysqlDialect);
    expect(sql).toBe(
      'DROP TRIGGER `trg_users_bi`;\n' +
        'CREATE TRIGGER trg_users_bi BEFORE INSERT ON users FOR EACH ROW SET NEW.x = 1',
    );
  });

  it('emits only a REVIEW comment block for cross-engine sync, with no DROP/CREATE', () => {
    const trigger = makeTriggerInfo();
    const sql = buildTriggerSyncSql(trigger, false, postgresDialect);
    expect(sql).toContain('-- REVIEW');
    expect(sql).toContain('trg_users_bi');
    expect(sql).toContain('-- CREATE TRIGGER trg_users_bi BEFORE INSERT ON users');
    expect(sql).not.toMatch(/^DROP TRIGGER/m);
    expect(sql).not.toMatch(/^CREATE TRIGGER/m);
  });
});

describe('buildCreateTableSql', () => {
  it('passes the source DDL through unchanged', () => {
    const ddl = 'CREATE TABLE users (id INT PRIMARY KEY)';
    expect(buildCreateTableSql(ddl)).toBe(ddl);
  });
});

describe('generateSyncSql', () => {
  function emptyDiff(): SchemaDiff {
    return { tables: [], views: [], crossEngine: false };
  }

  it('produces no statements for an empty diff', () => {
    expect(
      generateSyncSql(emptyDiff(), 'leftToRight', postgresDialect, postgresDialect, 'db', 'db'),
    ).toEqual([]);
  });

  it('adds a column present only on the left when syncing leftToRight', () => {
    const col = makeColumnInfo({ name: 'age', dataType: 'INT' });
    const colDiff: ObjectDiff<ColumnInfo> = {
      key: 'age',
      status: 'removed',
      left: col,
      right: null,
      fieldDiffs: [],
    };
    const diff: SchemaDiff = {
      tables: [
        {
          tableName: 'users',
          status: 'changed',
          columns: [colDiff],
          indexes: [],
          foreignKeys: [],
          checkConstraints: [],
          triggers: [],
        },
      ],
      views: [],
      crossEngine: false,
    };
    const sqls = generateSyncSql(
      diff,
      'leftToRight',
      postgresDialect,
      postgresDialect,
      'left_db',
      'right_db',
    );
    expect(sqls).toEqual(['ALTER TABLE "right_db"."users" ADD COLUMN "age" INT']);
  });

  it('drops a column present only on the left when syncing rightToLeft', () => {
    const col = makeColumnInfo({ name: 'age', dataType: 'INT' });
    const colDiff: ObjectDiff<ColumnInfo> = {
      key: 'age',
      status: 'removed',
      left: col,
      right: null,
      fieldDiffs: [],
    };
    const diff: SchemaDiff = {
      tables: [
        {
          tableName: 'users',
          status: 'changed',
          columns: [colDiff],
          indexes: [],
          foreignKeys: [],
          checkConstraints: [],
          triggers: [],
        },
      ],
      views: [],
      crossEngine: false,
    };
    const sqls = generateSyncSql(
      diff,
      'rightToLeft',
      postgresDialect,
      postgresDialect,
      'left_db',
      'right_db',
    );
    expect(sqls).toEqual(['ALTER TABLE "left_db"."users" DROP COLUMN "age"']);
  });

  it('generates a fallback CREATE TABLE for a table missing on the target side', () => {
    const col = makeColumnInfo({
      name: 'id',
      dataType: 'INT',
      nullable: false,
      isPrimaryKey: true,
    });
    const diff: SchemaDiff = {
      tables: [
        {
          tableName: 'orders',
          status: 'removed',
          columns: [{ key: 'id', status: 'removed', left: col, right: null, fieldDiffs: [] }],
          indexes: [],
          foreignKeys: [],
          checkConstraints: [],
          triggers: [],
        },
      ],
      views: [],
      crossEngine: false,
    };
    const sqls = generateSyncSql(
      diff,
      'leftToRight',
      postgresDialect,
      postgresDialect,
      'left_db',
      'right_db',
    );
    expect(sqls).toHaveLength(1);
    expect(sqls[0]).toContain('CREATE TABLE "right_db"."orders"');
    expect(sqls[0]).toContain('"id" INT NOT NULL');
    expect(sqls[0]).toContain('PRIMARY KEY ("id")');
  });

  it('prefixes cross-engine CREATE TABLE statements with a review comment', () => {
    const col = makeColumnInfo({ name: 'id', dataType: 'INT' });
    const diff: SchemaDiff = {
      tables: [
        {
          tableName: 'orders',
          status: 'removed',
          columns: [{ key: 'id', status: 'removed', left: col, right: null, fieldDiffs: [] }],
          indexes: [],
          foreignKeys: [],
          checkConstraints: [],
          triggers: [],
        },
      ],
      views: [],
      crossEngine: true,
    };
    const sqls = generateSyncSql(
      diff,
      'leftToRight',
      mysqlDialect,
      postgresDialect,
      'left_db',
      'right_db',
    );
    expect(sqls[0].startsWith('-- REVIEW: types approximated across engines')).toBe(true);
  });

  it('drops the whole table when it is missing on the source side', () => {
    const col = makeColumnInfo({ name: 'id' });
    const diff: SchemaDiff = {
      tables: [
        {
          tableName: 'orders',
          status: 'added',
          columns: [{ key: 'id', status: 'added', left: null, right: col, fieldDiffs: [] }],
          indexes: [],
          foreignKeys: [],
          checkConstraints: [],
          triggers: [],
        },
      ],
      views: [],
      crossEngine: false,
    };
    const sqls = generateSyncSql(
      diff,
      'leftToRight',
      postgresDialect,
      postgresDialect,
      'left_db',
      'right_db',
    );
    expect(sqls).toEqual(['DROP TABLE "right_db"."orders"']);
  });

  it('orders statements as drops-of-constraints/indexes/columns before creates before alters', () => {
    const origCol = makeColumnInfo({ name: 'age', dataType: 'INT' });
    const desiredCol = makeColumnInfo({ name: 'age', dataType: 'BIGINT' });
    const changedColDiff: ObjectDiff<ColumnInfo> = {
      key: 'age',
      status: 'changed',
      left: desiredCol,
      right: origCol,
      fieldDiffs: [{ field: 'dataType', left: 'BIGINT', right: 'INT' }],
    };
    const removedIdx: ObjectDiff<IndexInfo> = {
      key: 'idx_old',
      status: 'removed',
      left: { name: 'idx_old', columns: ['age'], unique: false, indexType: 'btree' },
      right: null,
      fieldDiffs: [],
    };
    const addedCol = makeColumnInfo({ name: 'new_col' });
    const addedColDiff: ObjectDiff<ColumnInfo> = {
      key: 'new_col',
      status: 'removed',
      left: addedCol,
      right: null,
      fieldDiffs: [],
    };
    const diff: SchemaDiff = {
      tables: [
        {
          tableName: 'users',
          status: 'changed',
          columns: [changedColDiff, addedColDiff],
          indexes: [removedIdx],
          foreignKeys: [],
          checkConstraints: [],
          triggers: [],
        },
      ],
      views: [],
      crossEngine: false,
    };
    const sqls = generateSyncSql(
      diff,
      'leftToRight',
      postgresDialect,
      postgresDialect,
      'left_db',
      'right_db',
    );
    // removedIdx: status 'removed' means present on left only -> in leftToRight
    // that means it's an index the target (right) is missing -> gets ADDED.
    const idxAddPos = sqls.findIndex((s) => s.startsWith('CREATE INDEX'));
    const colAddPos = sqls.findIndex((s) => s.includes('ADD COLUMN'));
    const alterPos = sqls.findIndex((s) => s.includes('ALTER COLUMN'));
    expect(colAddPos).toBeGreaterThanOrEqual(0);
    expect(idxAddPos).toBeGreaterThanOrEqual(0);
    expect(alterPos).toBeGreaterThanOrEqual(0);
    expect(colAddPos).toBeLessThan(alterPos);
    expect(idxAddPos).toBeLessThan(alterPos);
  });

  function makeCheckConstraintInfo(
    overrides: Partial<CheckConstraintInfo> = {},
  ): CheckConstraintInfo {
    return {
      constraintName: 'chk_age',
      tableName: 'users',
      expression: 'age >= 0',
      ...overrides,
    };
  }

  it('adds a check constraint present only on the left when syncing leftToRight', () => {
    const cc = makeCheckConstraintInfo();
    const ccDiff: ObjectDiff<CheckConstraintInfo> = {
      key: 'chk_age',
      status: 'removed',
      left: cc,
      right: null,
      fieldDiffs: [],
    };
    const diff: SchemaDiff = {
      tables: [
        {
          tableName: 'users',
          status: 'changed',
          columns: [],
          indexes: [],
          foreignKeys: [],
          checkConstraints: [ccDiff],
          triggers: [],
        },
      ],
      views: [],
      crossEngine: false,
    };
    const sqls = generateSyncSql(
      diff,
      'leftToRight',
      postgresDialect,
      postgresDialect,
      'left_db',
      'right_db',
    );
    expect(sqls).toEqual([
      'ALTER TABLE "right_db"."users" ADD CONSTRAINT "chk_age" CHECK (age >= 0)',
    ]);
  });

  it('drops a check constraint present only on the left when syncing rightToLeft', () => {
    const cc = makeCheckConstraintInfo();
    const ccDiff: ObjectDiff<CheckConstraintInfo> = {
      key: 'chk_age',
      status: 'removed',
      left: cc,
      right: null,
      fieldDiffs: [],
    };
    const diff: SchemaDiff = {
      tables: [
        {
          tableName: 'users',
          status: 'changed',
          columns: [],
          indexes: [],
          foreignKeys: [],
          checkConstraints: [ccDiff],
          triggers: [],
        },
      ],
      views: [],
      crossEngine: false,
    };
    const sqls = generateSyncSql(
      diff,
      'rightToLeft',
      postgresDialect,
      postgresDialect,
      'left_db',
      'right_db',
    );
    expect(sqls).toEqual(['ALTER TABLE "left_db"."users" DROP CONSTRAINT "chk_age"']);
  });

  it('drops and re-adds a changed check constraint', () => {
    const leftCc = makeCheckConstraintInfo({ expression: 'age >= 0' });
    const rightCc = makeCheckConstraintInfo({ expression: 'age >= 18' });
    const ccDiff: ObjectDiff<CheckConstraintInfo> = {
      key: 'chk_age',
      status: 'changed',
      left: leftCc,
      right: rightCc,
      fieldDiffs: [{ field: 'expression', left: 'age >= 0', right: 'age >= 18' }],
    };
    const diff: SchemaDiff = {
      tables: [
        {
          tableName: 'users',
          status: 'changed',
          columns: [],
          indexes: [],
          foreignKeys: [],
          checkConstraints: [ccDiff],
          triggers: [],
        },
      ],
      views: [],
      crossEngine: false,
    };
    const sqls = generateSyncSql(
      diff,
      'leftToRight',
      postgresDialect,
      postgresDialect,
      'left_db',
      'right_db',
    );
    expect(sqls).toEqual([
      'ALTER TABLE "right_db"."users" DROP CONSTRAINT "chk_age"',
      'ALTER TABLE "right_db"."users" ADD CONSTRAINT "chk_age" CHECK (age >= 0)',
    ]);
  });

  it('orders check-constraint drops alongside FK drops (before column drops) and adds alongside FK adds (after column adds)', () => {
    // Present on right only -> in leftToRight this is dropped from the target (right).
    const removedColDiff: ObjectDiff<ColumnInfo> = {
      key: 'legacy',
      status: 'added',
      left: null,
      right: makeColumnInfo({ name: 'legacy' }),
      fieldDiffs: [],
    };
    const addedCol = makeColumnInfo({ name: 'new_col' });
    const addedColDiff: ObjectDiff<ColumnInfo> = {
      key: 'new_col',
      status: 'removed',
      left: addedCol,
      right: null,
      fieldDiffs: [],
    };
    const addedCcDiff: ObjectDiff<CheckConstraintInfo> = {
      key: 'chk_age',
      status: 'removed',
      left: makeCheckConstraintInfo(),
      right: null,
      fieldDiffs: [],
    };
    const droppedCcDiff: ObjectDiff<CheckConstraintInfo> = {
      key: 'chk_legacy',
      status: 'added',
      left: null,
      right: makeCheckConstraintInfo({ constraintName: 'chk_legacy' }),
      fieldDiffs: [],
    };
    const diff: SchemaDiff = {
      tables: [
        {
          tableName: 'users',
          status: 'changed',
          columns: [removedColDiff, addedColDiff],
          indexes: [],
          foreignKeys: [],
          checkConstraints: [addedCcDiff, droppedCcDiff],
          triggers: [],
        },
      ],
      views: [],
      crossEngine: false,
    };
    const sqls = generateSyncSql(
      diff,
      'leftToRight',
      postgresDialect,
      postgresDialect,
      'left_db',
      'right_db',
    );
    const checkDropPos = sqls.findIndex((s) => s.includes('DROP CONSTRAINT "chk_legacy"'));
    const checkAddPos = sqls.findIndex((s) => s.includes('ADD CONSTRAINT "chk_age"'));
    const colDropPos = sqls.findIndex((s) => s === 'ALTER TABLE "right_db"."users" DROP COLUMN "legacy"');
    const colAddPos = sqls.findIndex((s) => s.includes('ADD COLUMN "new_col"'));
    expect(checkDropPos).toBeGreaterThanOrEqual(0);
    expect(checkAddPos).toBeGreaterThanOrEqual(0);
    expect(colDropPos).toBeGreaterThanOrEqual(0);
    expect(colAddPos).toBeGreaterThanOrEqual(0);
    expect(checkDropPos).toBeLessThan(colDropPos);
    expect(colAddPos).toBeLessThan(checkAddPos);
  });

  it('adds a trigger present only on the left when syncing leftToRight, using the raw definition (same engine)', () => {
    const trg = makeTriggerInfo();
    const trgDiff: ObjectDiff<TriggerInfo> = {
      key: 'trg_users_bi',
      status: 'removed',
      left: trg,
      right: null,
      fieldDiffs: [],
    };
    const diff: SchemaDiff = {
      tables: [
        {
          tableName: 'users',
          status: 'changed',
          columns: [],
          indexes: [],
          foreignKeys: [],
          checkConstraints: [],
          triggers: [trgDiff],
        },
      ],
      views: [],
      crossEngine: false,
    };
    const sqls = generateSyncSql(
      diff,
      'leftToRight',
      mysqlDialect,
      mysqlDialect,
      'left_db',
      'right_db',
    );
    expect(sqls).toEqual([`DROP TRIGGER \`trg_users_bi\`;\n${trg.definition}`]);
  });

  it('drops a trigger present only on the left when syncing rightToLeft', () => {
    const trg = makeTriggerInfo();
    const trgDiff: ObjectDiff<TriggerInfo> = {
      key: 'trg_users_bi',
      status: 'removed',
      left: trg,
      right: null,
      fieldDiffs: [],
    };
    const diff: SchemaDiff = {
      tables: [
        {
          tableName: 'users',
          status: 'changed',
          columns: [],
          indexes: [],
          foreignKeys: [],
          checkConstraints: [],
          triggers: [trgDiff],
        },
      ],
      views: [],
      crossEngine: false,
    };
    const sqls = generateSyncSql(
      diff,
      'rightToLeft',
      mysqlDialect,
      mysqlDialect,
      'left_db',
      'right_db',
    );
    expect(sqls).toEqual(['DROP TRIGGER `trg_users_bi`']);
  });

  it('emits only a REVIEW comment (no DROP/CREATE) for a cross-engine added trigger', () => {
    const trg = makeTriggerInfo();
    const trgDiff: ObjectDiff<TriggerInfo> = {
      key: 'trg_users_bi',
      status: 'removed',
      left: trg,
      right: null,
      fieldDiffs: [],
    };
    const diff: SchemaDiff = {
      tables: [
        {
          tableName: 'users',
          status: 'changed',
          columns: [],
          indexes: [],
          foreignKeys: [],
          checkConstraints: [],
          triggers: [trgDiff],
        },
      ],
      views: [],
      crossEngine: true,
    };
    const sqls = generateSyncSql(
      diff,
      'leftToRight',
      mysqlDialect,
      postgresDialect,
      'left_db',
      'right_db',
    );
    expect(sqls).toHaveLength(1);
    expect(sqls[0]).toContain('-- REVIEW');
    expect(sqls[0]).not.toMatch(/^DROP TRIGGER/m);
    expect(sqls[0]).not.toMatch(/^CREATE TRIGGER/m);
  });

  it('orders trigger creates after everything else (columns, indexes, FKs, check constraints)', () => {
    const addedColDiff: ObjectDiff<ColumnInfo> = {
      key: 'new_col',
      status: 'removed',
      left: makeColumnInfo({ name: 'new_col' }),
      right: null,
      fieldDiffs: [],
    };
    const addedCcDiff: ObjectDiff<CheckConstraintInfo> = {
      key: 'chk_age',
      status: 'removed',
      left: makeCheckConstraintInfo(),
      right: null,
      fieldDiffs: [],
    };
    const addedTrgDiff: ObjectDiff<TriggerInfo> = {
      key: 'trg_users_bi',
      status: 'removed',
      left: makeTriggerInfo(),
      right: null,
      fieldDiffs: [],
    };
    const removedTrgDiff: ObjectDiff<TriggerInfo> = {
      key: 'trg_legacy',
      status: 'added',
      left: null,
      right: makeTriggerInfo({ name: 'trg_legacy' }),
      fieldDiffs: [],
    };
    const diff: SchemaDiff = {
      tables: [
        {
          tableName: 'users',
          status: 'changed',
          columns: [addedColDiff],
          indexes: [],
          foreignKeys: [],
          checkConstraints: [addedCcDiff],
          triggers: [addedTrgDiff, removedTrgDiff],
        },
      ],
      views: [],
      crossEngine: false,
    };
    const sqls = generateSyncSql(
      diff,
      'leftToRight',
      postgresDialect,
      postgresDialect,
      'left_db',
      'right_db',
    );
    const triggerDropPos = sqls.findIndex((s) => s === 'DROP TRIGGER "trg_legacy"');
    const triggerCreatePos = sqls.findIndex((s) => s.includes('trg_users_bi'));
    const colAddPos = sqls.findIndex((s) => s.includes('ADD COLUMN "new_col"'));
    const checkAddPos = sqls.findIndex((s) => s.includes('ADD CONSTRAINT "chk_age"'));
    expect(triggerDropPos).toBeGreaterThanOrEqual(0);
    expect(triggerCreatePos).toBeGreaterThanOrEqual(0);
    expect(colAddPos).toBeGreaterThanOrEqual(0);
    expect(checkAddPos).toBeGreaterThanOrEqual(0);
    // Trigger drops run near the start, well before column/check adds.
    expect(triggerDropPos).toBeLessThan(colAddPos);
    // Trigger creates run last, after every other change.
    expect(colAddPos).toBeLessThan(triggerCreatePos);
    expect(checkAddPos).toBeLessThan(triggerCreatePos);
  });

  it('generates CREATE VIEW / DROP VIEW statements for view diffs', () => {
    const diff: SchemaDiff = {
      tables: [],
      views: [
        {
          key: 'v1',
          status: 'removed',
          left: { name: 'v1', definition: 'SELECT 1' },
          right: null,
          fieldDiffs: [],
        },
      ],
      crossEngine: false,
    };
    const sqls = generateSyncSql(
      diff,
      'leftToRight',
      postgresDialect,
      postgresDialect,
      'left_db',
      'right_db',
    );
    expect(sqls).toEqual(['CREATE VIEW "right_db"."v1" AS SELECT 1']);
  });
});
