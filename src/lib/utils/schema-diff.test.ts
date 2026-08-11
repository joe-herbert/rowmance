import { describe, it, expect } from 'vitest';
import type {
  ColumnInfo,
  IndexInfo,
  ForeignKeyInfo,
  TableInfo,
  ViewInfo,
  CheckConstraintInfo,
  TriggerInfo,
} from '$lib/types';
import {
  diffColumns,
  diffIndexes,
  diffForeignKeys,
  diffCheckConstraints,
  diffTriggers,
  diffViews,
  diffTables,
  diffTable,
  diffDatabase,
  reorientObjectDiff,
  reorientTableDiff,
  reorientSchemaDiff,
} from './schema-diff';

function col(overrides: Partial<ColumnInfo> = {}): ColumnInfo {
  return {
    name: 'id',
    dataType: 'INT',
    nullable: false,
    defaultValue: null,
    isPrimaryKey: false,
    isAutoIncrement: false,
    isForeignKey: false,
    comment: null,
    ...overrides,
  };
}

function idx(overrides: Partial<IndexInfo> = {}): IndexInfo {
  return {
    name: 'idx_email',
    columns: ['email'],
    unique: false,
    indexType: 'btree',
    ...overrides,
  };
}

function fk(overrides: Partial<ForeignKeyInfo> = {}): ForeignKeyInfo {
  return {
    constraintName: 'fk_user',
    columns: ['user_id'],
    referencedTable: 'users',
    referencedColumns: ['id'],
    onDelete: 'CASCADE',
    onUpdate: 'NO ACTION',
    ...overrides,
  };
}

function checkConstraint(overrides: Partial<CheckConstraintInfo> = {}): CheckConstraintInfo {
  return {
    constraintName: 'chk_age',
    tableName: 'users',
    expression: 'age >= 0',
    ...overrides,
  };
}

function trigger(overrides: Partial<TriggerInfo> = {}): TriggerInfo {
  return {
    name: 'trg_users_bi',
    tableName: 'users',
    timing: 'BEFORE',
    event: 'INSERT',
    definition: 'CREATE TRIGGER trg_users_bi BEFORE INSERT ON users FOR EACH ROW BEGIN END',
    ...overrides,
  };
}

function view(overrides: Partial<ViewInfo> = {}): ViewInfo {
  return { name: 'active_users', definition: 'SELECT * FROM users', ...overrides };
}

function tableInfo(name: string): TableInfo {
  return { name, tableType: 'table', rowCount: null };
}

describe('diffColumns', () => {
  it('returns empty for empty-vs-empty', () => {
    expect(diffColumns([], [], false)).toEqual([]);
  });

  it('marks a column present only on the left as removed', () => {
    const [d] = diffColumns([col({ name: 'id' })], [], false);
    expect(d.status).toBe('removed');
    expect(d.left).not.toBeNull();
    expect(d.right).toBeNull();
  });

  it('marks a column present only on the right as added', () => {
    const [d] = diffColumns([], [col({ name: 'id' })], false);
    expect(d.status).toBe('added');
    expect(d.left).toBeNull();
    expect(d.right).not.toBeNull();
  });

  it('marks identical columns as unchanged', () => {
    const c = col();
    const [d] = diffColumns([c], [{ ...c }], false);
    expect(d.status).toBe('unchanged');
    expect(d.fieldDiffs).toEqual([]);
  });

  it('marks columns with differing fields as changed, listing the field diffs', () => {
    const left = col({ nullable: false });
    const right = col({ nullable: true });
    const [d] = diffColumns([left], [right], false);
    expect(d.status).toBe('changed');
    expect(d.fieldDiffs).toEqual([{ field: 'nullable', left: false, right: true }]);
  });

  it('matches column names case-insensitively while preserving original casing', () => {
    const left = col({ name: 'Users_Id' });
    const right = col({ name: 'users_id' });
    const [d] = diffColumns([left], [right], false);
    expect(d.status).toBe('unchanged');
    expect((d.left as ColumnInfo).name).toBe('Users_Id');
    expect((d.right as ColumnInfo).name).toBe('users_id');
  });

  it('does not set crossEngineCaveat when crossEngine is false, even for a dataType-only diff', () => {
    const left = col({ dataType: 'INT' });
    const right = col({ dataType: 'INTEGER' });
    const [d] = diffColumns([left], [right], false);
    expect(d.status).toBe('changed');
    expect(d.crossEngineCaveat).toBeUndefined();
  });

  it('sets crossEngineCaveat when crossEngine is true and dataType is the only differing field', () => {
    const left = col({ dataType: 'INT' });
    const right = col({ dataType: 'INTEGER' });
    const [d] = diffColumns([left], [right], true);
    expect(d.status).toBe('changed');
    expect(d.crossEngineCaveat).toBe(true);
  });

  it('does not set crossEngineCaveat when other fields also differ', () => {
    const left = col({ dataType: 'INT', nullable: false });
    const right = col({ dataType: 'INTEGER', nullable: true });
    const [d] = diffColumns([left], [right], true);
    expect(d.status).toBe('changed');
    expect(d.crossEngineCaveat).toBeFalsy();
    expect(d.fieldDiffs).toHaveLength(2);
  });
});

describe('diffIndexes', () => {
  it('returns empty for empty-vs-empty', () => {
    expect(diffIndexes([], [])).toEqual([]);
  });

  it('detects added/removed/unchanged/changed indexes', () => {
    const removed = idx({ name: 'idx_old' });
    const unchanged = idx({ name: 'idx_same' });
    const changedLeft = idx({ name: 'idx_changed', unique: false });
    const changedRight = idx({ name: 'idx_changed', unique: true });
    const added = idx({ name: 'idx_new' });

    const diffs = diffIndexes([removed, unchanged, changedLeft], [unchanged, changedRight, added]);
    const byName = Object.fromEntries(diffs.map((d) => [d.key, d]));

    expect(byName['idx_old'].status).toBe('removed');
    expect(byName['idx_same'].status).toBe('unchanged');
    expect(byName['idx_changed'].status).toBe('changed');
    expect(byName['idx_changed'].fieldDiffs).toEqual([
      { field: 'unique', left: false, right: true },
    ]);
    expect(byName['idx_new'].status).toBe('added');
  });

  it('matches names case-insensitively', () => {
    const [d] = diffIndexes([idx({ name: 'IDX_Email' })], [idx({ name: 'idx_email' })]);
    expect(d.status).toBe('unchanged');
  });

  it('detects a diff in the columns array', () => {
    const [d] = diffIndexes(
      [idx({ columns: ['email'] })],
      [idx({ columns: ['email', 'tenant_id'] })],
    );
    expect(d.status).toBe('changed');
    expect(d.fieldDiffs[0].field).toBe('columns');
  });
});

describe('diffForeignKeys', () => {
  it('returns empty for empty-vs-empty', () => {
    expect(diffForeignKeys([], [])).toEqual([]);
  });

  it('detects added/removed/unchanged/changed foreign keys', () => {
    const removed = fk({ constraintName: 'fk_old' });
    const unchanged = fk({ constraintName: 'fk_same' });
    const changedLeft = fk({ constraintName: 'fk_changed', onDelete: 'CASCADE' });
    const changedRight = fk({ constraintName: 'fk_changed', onDelete: 'SET NULL' });
    const added = fk({ constraintName: 'fk_new' });

    const diffs = diffForeignKeys(
      [removed, unchanged, changedLeft],
      [unchanged, changedRight, added],
    );
    const byName = Object.fromEntries(diffs.map((d) => [d.key, d]));

    expect(byName['fk_old'].status).toBe('removed');
    expect(byName['fk_same'].status).toBe('unchanged');
    expect(byName['fk_changed'].status).toBe('changed');
    expect(byName['fk_new'].status).toBe('added');
  });

  it('matches names case-insensitively', () => {
    const [d] = diffForeignKeys(
      [fk({ constraintName: 'FK_User' })],
      [fk({ constraintName: 'fk_user' })],
    );
    expect(d.status).toBe('unchanged');
  });
});

describe('diffCheckConstraints', () => {
  it('returns empty for empty-vs-empty', () => {
    expect(diffCheckConstraints([], [])).toEqual([]);
  });

  it('detects added/removed/unchanged/changed check constraints', () => {
    const removed = checkConstraint({ constraintName: 'chk_old' });
    const unchanged = checkConstraint({ constraintName: 'chk_same' });
    const changedLeft = checkConstraint({ constraintName: 'chk_changed', expression: 'age >= 0' });
    const changedRight = checkConstraint({
      constraintName: 'chk_changed',
      expression: 'age >= 18',
    });
    const added = checkConstraint({ constraintName: 'chk_new' });

    const diffs = diffCheckConstraints(
      [removed, unchanged, changedLeft],
      [unchanged, changedRight, added],
    );
    const byName = Object.fromEntries(diffs.map((d) => [d.key, d]));

    expect(byName['chk_old'].status).toBe('removed');
    expect(byName['chk_same'].status).toBe('unchanged');
    expect(byName['chk_changed'].status).toBe('changed');
    expect(byName['chk_changed'].fieldDiffs).toEqual([
      { field: 'expression', left: 'age >= 0', right: 'age >= 18' },
    ]);
    expect(byName['chk_new'].status).toBe('added');
  });

  it('matches constraint names case-insensitively while preserving original casing', () => {
    const left = checkConstraint({ constraintName: 'CHK_Age' });
    const right = checkConstraint({ constraintName: 'chk_age' });
    const [d] = diffCheckConstraints([left], [right]);
    expect(d.status).toBe('unchanged');
    expect((d.left as CheckConstraintInfo).constraintName).toBe('CHK_Age');
    expect((d.right as CheckConstraintInfo).constraintName).toBe('chk_age');
  });
});

describe('diffTriggers', () => {
  it('returns empty for empty-vs-empty', () => {
    expect(diffTriggers([], [])).toEqual([]);
  });

  it('detects added/removed/unchanged/changed triggers', () => {
    const removed = trigger({ name: 'trg_old' });
    const unchanged = trigger({ name: 'trg_same' });
    const changedLeft = trigger({ name: 'trg_changed', event: 'INSERT' });
    const changedRight = trigger({ name: 'trg_changed', event: 'UPDATE' });
    const added = trigger({ name: 'trg_new' });

    const diffs = diffTriggers(
      [removed, unchanged, changedLeft],
      [unchanged, changedRight, added],
    );
    const byName = Object.fromEntries(diffs.map((d) => [d.key, d]));

    expect(byName['trg_old'].status).toBe('removed');
    expect(byName['trg_same'].status).toBe('unchanged');
    expect(byName['trg_changed'].status).toBe('changed');
    expect(byName['trg_changed'].fieldDiffs).toEqual([
      { field: 'event', left: 'INSERT', right: 'UPDATE' },
    ]);
    expect(byName['trg_new'].status).toBe('added');
  });

  it('matches trigger names case-insensitively while preserving original casing', () => {
    const left = trigger({ name: 'TRG_Users_BI' });
    const right = trigger({ name: 'trg_users_bi' });
    const [d] = diffTriggers([left], [right]);
    expect(d.status).toBe('unchanged');
    expect((d.left as TriggerInfo).name).toBe('TRG_Users_BI');
    expect((d.right as TriggerInfo).name).toBe('trg_users_bi');
  });
});

describe('diffViews', () => {
  it('returns empty for empty-vs-empty', () => {
    expect(diffViews([], [])).toEqual([]);
  });

  it('detects added/removed/unchanged/changed views', () => {
    const unchanged = view({ name: 'v_same' });
    const changedLeft = view({ name: 'v_changed', definition: 'SELECT 1' });
    const changedRight = view({ name: 'v_changed', definition: 'SELECT 2' });

    const diffs = diffViews(
      [view({ name: 'v_removed' }), unchanged, changedLeft],
      [unchanged, changedRight, view({ name: 'v_added' })],
    );
    const byName = Object.fromEntries(diffs.map((d) => [d.key, d]));

    expect(byName['v_removed'].status).toBe('removed');
    expect(byName['v_same'].status).toBe('unchanged');
    expect(byName['v_changed'].status).toBe('changed');
    expect(byName['v_added'].status).toBe('added');
  });
});

describe('diffTables', () => {
  it('returns empty arrays for empty-vs-empty', () => {
    expect(diffTables([], [])).toEqual({ added: [], removed: [], common: [] });
  });

  it('partitions tables into added/removed/common', () => {
    const result = diffTables(
      [tableInfo('users'), tableInfo('orders')],
      [tableInfo('orders'), tableInfo('products')],
    );
    expect(result.removed).toEqual(['users']);
    expect(result.added).toEqual(['products']);
    expect(result.common).toEqual(['orders']);
  });

  it('matches table names case-insensitively', () => {
    const result = diffTables([tableInfo('Users')], [tableInfo('users')]);
    expect(result.common).toEqual(['Users']);
    expect(result.added).toEqual([]);
    expect(result.removed).toEqual([]);
  });
});

describe('diffTable', () => {
  it('is unchanged when both sides have identical columns/indexes/fks', () => {
    const side = { columns: [col()], indexes: [idx()], foreignKeys: [fk()] };
    const result = diffTable('users', side, { ...side }, false);
    expect(result.status).toBe('unchanged');
    expect(result.checkConstraints).toEqual([]);
    expect(result.triggers).toEqual([]);
  });

  it('is changed when any child collection has a diff', () => {
    const left = { columns: [col({ nullable: false })], indexes: [], foreignKeys: [] };
    const right = { columns: [col({ nullable: true })], indexes: [], foreignKeys: [] };
    const result = diffTable('users', left, right, false);
    expect(result.status).toBe('changed');
  });

  it('treats missing checkConstraints as an empty array on either side', () => {
    const left = { columns: [col()], indexes: [idx()], foreignKeys: [fk()] };
    const right = {
      columns: [col()],
      indexes: [idx()],
      foreignKeys: [fk()],
      checkConstraints: [],
    };
    const result = diffTable('users', left, right, false);
    expect(result.status).toBe('unchanged');
    expect(result.checkConstraints).toEqual([]);
  });

  it('is changed when check constraints differ, and populates checkConstraints diffs', () => {
    const side = { columns: [col()], indexes: [idx()], foreignKeys: [fk()] };
    const left = { ...side, checkConstraints: [checkConstraint({ expression: 'age >= 0' })] };
    const right = { ...side, checkConstraints: [checkConstraint({ expression: 'age >= 18' })] };
    const result = diffTable('users', left, right, false);
    expect(result.status).toBe('changed');
    expect(result.checkConstraints).toHaveLength(1);
    expect(result.checkConstraints[0].status).toBe('changed');
  });

  it('treats missing triggers as an empty array on either side', () => {
    const left = { columns: [col()], indexes: [idx()], foreignKeys: [fk()] };
    const right = {
      columns: [col()],
      indexes: [idx()],
      foreignKeys: [fk()],
      triggers: [],
    };
    const result = diffTable('users', left, right, false);
    expect(result.status).toBe('unchanged');
    expect(result.triggers).toEqual([]);
  });

  it('is changed when triggers differ, and populates triggers diffs', () => {
    const side = { columns: [col()], indexes: [idx()], foreignKeys: [fk()] };
    const left = { ...side, triggers: [trigger({ event: 'INSERT' })] };
    const right = { ...side, triggers: [trigger({ event: 'UPDATE' })] };
    const result = diffTable('users', left, right, false);
    expect(result.status).toBe('changed');
    expect(result.triggers).toHaveLength(1);
    expect(result.triggers[0].status).toBe('changed');
  });
});

describe('diffDatabase', () => {
  it('produces no diffs for empty-vs-empty', () => {
    const empty = { tables: [], tableDetails: new Map(), views: [] };
    const result = diffDatabase(empty, empty, false);
    expect(result.tables).toEqual([]);
    expect(result.views).toEqual([]);
    expect(result.crossEngine).toBe(false);
  });

  it('marks a table missing on the right as removed, with all its columns removed', () => {
    const left = {
      tables: [tableInfo('users')],
      tableDetails: new Map([
        [
          'users',
          {
            columns: [col()],
            indexes: [],
            foreignKeys: [],
            checkConstraints: [checkConstraint()],
            triggers: [trigger()],
          },
        ],
      ]),
      views: [],
    };
    const right = { tables: [], tableDetails: new Map(), views: [] };
    const result = diffDatabase(left, right, false);
    expect(result.tables).toHaveLength(1);
    expect(result.tables[0].status).toBe('removed');
    expect(result.tables[0].columns[0].status).toBe('removed');
    expect(result.tables[0].checkConstraints).toHaveLength(1);
    expect(result.tables[0].checkConstraints[0].status).toBe('removed');
    expect(result.tables[0].triggers).toHaveLength(1);
    expect(result.tables[0].triggers[0].status).toBe('removed');
  });

  it('marks a table missing on the left as added, with all its columns added', () => {
    const right = {
      tables: [tableInfo('users')],
      tableDetails: new Map([
        [
          'users',
          {
            columns: [col()],
            indexes: [],
            foreignKeys: [],
            checkConstraints: [checkConstraint()],
            triggers: [trigger()],
          },
        ],
      ]),
      views: [],
    };
    const left = { tables: [], tableDetails: new Map(), views: [] };
    const result = diffDatabase(left, right, false);
    expect(result.tables).toHaveLength(1);
    expect(result.tables[0].status).toBe('added');
    expect(result.tables[0].columns[0].status).toBe('added');
    expect(result.tables[0].checkConstraints).toHaveLength(1);
    expect(result.tables[0].checkConstraints[0].status).toBe('added');
    expect(result.tables[0].triggers).toHaveLength(1);
    expect(result.tables[0].triggers[0].status).toBe('added');
  });

  it('diffs common tables and propagates crossEngine to column diffs', () => {
    const left = {
      tables: [tableInfo('users')],
      tableDetails: new Map([
        ['users', { columns: [col({ dataType: 'INT' })], indexes: [], foreignKeys: [] }],
      ]),
      views: [],
    };
    const right = {
      tables: [tableInfo('users')],
      tableDetails: new Map([
        ['users', { columns: [col({ dataType: 'INTEGER' })], indexes: [], foreignKeys: [] }],
      ]),
      views: [],
    };
    const result = diffDatabase(left, right, true);
    expect(result.crossEngine).toBe(true);
    expect(result.tables[0].status).toBe('changed');
    expect(result.tables[0].columns[0].crossEngineCaveat).toBe(true);
  });

  it('diffs views alongside tables', () => {
    const left = { tables: [], tableDetails: new Map(), views: [view({ name: 'v1' })] };
    const right = { tables: [], tableDetails: new Map(), views: [] };
    const result = diffDatabase(left, right, false);
    expect(result.views).toHaveLength(1);
    expect(result.views[0].status).toBe('removed');
  });
});

describe('reorientObjectDiff / reorientTableDiff / reorientSchemaDiff', () => {
  // Column "foo" exists only on the physical left side (raw status: 'removed').
  const [removedOnRight] = diffColumns([col({ name: 'foo' })], [], false);
  // Column "bar" exists only on the physical right side (raw status: 'added').
  const [addedOnRight] = diffColumns([], [col({ name: 'bar' })], false);

  it('leftToRight (source=left, target=right): swaps status only, keeps left/right as-is', () => {
    // Present on left/source only -> needs to be created on the target -> displays as 'added'.
    const a = reorientObjectDiff(removedOnRight, 'leftToRight');
    expect(a.status).toBe('added');
    expect(a.left).toEqual(col({ name: 'foo' })); // untouched: left is still the source value
    expect(a.right).toBeNull();

    // Present on right/target only -> needs to be dropped from the target -> displays as 'removed'.
    const b = reorientObjectDiff(addedOnRight, 'leftToRight');
    expect(b.status).toBe('removed');
    expect(b.right).toEqual(col({ name: 'bar' })); // untouched
    expect(b.left).toBeNull();
  });

  it('rightToLeft (source=right, target=left): keeps status, swaps left/right to source-first', () => {
    // Present on left/target only -> raw 'removed' already means "drop from target" here.
    const a = reorientObjectDiff(removedOnRight, 'rightToLeft');
    expect(a.status).toBe('removed');
    expect(a.left).toBeNull(); // source (right) lacks it
    expect(a.right).toEqual(col({ name: 'foo' })); // target (left) has it, swapped into the right slot

    // Present on right/source only -> raw 'added' already means "create on target" here.
    const b = reorientObjectDiff(addedOnRight, 'rightToLeft');
    expect(b.status).toBe('added');
    expect(b.left).toEqual(col({ name: 'bar' })); // source (right), swapped into the left slot
    expect(b.right).toBeNull();
  });

  it('for a changed row, orders field values source-first/target-second in both directions', () => {
    const [changed] = diffColumns([col({ dataType: 'INT' })], [col({ dataType: 'TEXT' })], false);
    expect(changed.fieldDiffs).toEqual([{ field: 'dataType', left: 'INT', right: 'TEXT' }]);

    // leftToRight: source=left already comes first raw -> untouched.
    const ltr = reorientObjectDiff(changed, 'leftToRight');
    expect(ltr.status).toBe('changed');
    expect(ltr.fieldDiffs).toEqual([{ field: 'dataType', left: 'INT', right: 'TEXT' }]);

    // rightToLeft: source=right must come first -> swapped.
    const rtl = reorientObjectDiff(changed, 'rightToLeft');
    expect(rtl.status).toBe('changed');
    expect(rtl.fieldDiffs).toEqual([{ field: 'dataType', left: 'TEXT', right: 'INT' }]);
  });

  it('leaves unchanged rows alone in both directions', () => {
    const [unchanged] = diffColumns([col()], [col()], false);
    expect(reorientObjectDiff(unchanged, 'leftToRight').status).toBe('unchanged');
    expect(reorientObjectDiff(unchanged, 'rightToLeft').status).toBe('unchanged');
  });

  it('reorientTableDiff applies the same rule to the table status and every child group', () => {
    const t = diffTable(
      'users',
      { columns: [col()], indexes: [], foreignKeys: [], checkConstraints: [], triggers: [] },
      { columns: [], indexes: [], foreignKeys: [], checkConstraints: [], triggers: [] },
      false,
    );
    expect(t.columns[0].status).toBe('removed');
    expect(reorientTableDiff(t, 'leftToRight').columns[0].status).toBe('added');
    expect(reorientTableDiff(t, 'rightToLeft').columns[0].status).toBe('removed');
  });

  it('reorientSchemaDiff applies the same rule to tables and views, leaves crossEngine alone', () => {
    const left = { tables: [], tableDetails: new Map(), views: [view({ name: 'v1' })] };
    const right = { tables: [], tableDetails: new Map(), views: [] };
    const diff = diffDatabase(left, right, true);
    expect(diff.views[0].status).toBe('removed');
    expect(reorientSchemaDiff(diff, 'leftToRight').views[0].status).toBe('added');
    expect(reorientSchemaDiff(diff, 'rightToLeft').views[0].status).toBe('removed');
    expect(reorientSchemaDiff(diff, 'leftToRight').crossEngine).toBe(true);
  });
});
