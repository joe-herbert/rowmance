/**
 * Pure DDL-generation helpers for table structure editing (add/edit/drop
 * columns, indexes, and foreign keys) and schema-sync statement generation.
 *
 * These functions take `dialect`/`database`/`table` explicitly instead of
 * closing over component state, so they can be shared between
 * `TableStructure.svelte` and the schema-compare feature.
 */
import type {
  ColumnInfo,
  DialectInfo,
  IndexInfo,
  ForeignKeyInfo,
  ViewInfo,
  CheckConstraintInfo,
  TriggerInfo,
} from '$lib/types';
import { qi, tableRef } from '$lib/utils/dialect';
import type { SchemaDiff, ObjectDiff } from '$lib/utils/schema-diff';

/** Escape and quote a string literal for use in DEFAULT/COMMENT clauses. */
function escStr(s: string): string {
  return "'" + s.replace(/'/g, "''") + "'";
}

// ── Column SQL ───────────────────────────────────────────────────────────────

export interface ColForm {
  mode: 'add' | 'edit';
  original: ColumnInfo | null;
  name: string;
  dataType: string;
  nullable: boolean;
  defaultValue: string;
  autoIncrement: boolean;
  comment: string;
}

/** Render the `name TYPE [NOT NULL] [AUTO_INCREMENT] [DEFAULT ...] [COMMENT ...]` fragment. */
export function colDef(dialect: DialectInfo, form: ColForm): string {
  let s = `${qi(form.name, dialect)} ${form.dataType}`;
  if (!form.nullable) s += ' NOT NULL';
  if (dialect.supportsAutoIncrement && form.autoIncrement) s += ' AUTO_INCREMENT';
  if (form.defaultValue.trim()) s += ` DEFAULT ${form.defaultValue.trim()}`;
  if (dialect.supportsColumnComment && form.comment.trim())
    s += ` COMMENT ${escStr(form.comment.trim())}`;
  return s;
}

export function buildAddColSqls(
  dialect: DialectInfo,
  database: string,
  table: string,
  form: ColForm,
): string[] {
  return [`ALTER TABLE ${tableRef(database, table, dialect)} ADD COLUMN ${colDef(dialect, form)}`];
}

export function buildEditColSqls(
  dialect: DialectInfo,
  database: string,
  table: string,
  orig: ColumnInfo,
  form: ColForm,
): string[] {
  const t = tableRef(database, table, dialect);
  if (dialect.supportsChangeColumn) {
    return [`ALTER TABLE ${t} CHANGE COLUMN ${qi(orig.name, dialect)} ${colDef(dialect, form)}`];
  }
  const stmts: string[] = [];
  const oq = qi(orig.name, dialect);
  if (form.dataType !== orig.dataType) {
    stmts.push(`ALTER TABLE ${t} ALTER COLUMN ${oq} TYPE ${form.dataType}`);
  }
  if (form.nullable !== orig.nullable) {
    stmts.push(
      `ALTER TABLE ${t} ALTER COLUMN ${oq} ${form.nullable ? 'DROP NOT NULL' : 'SET NOT NULL'}`,
    );
  }
  const origDef = orig.defaultValue ?? '';
  const newDef = form.defaultValue.trim();
  if (newDef !== origDef) {
    stmts.push(
      newDef
        ? `ALTER TABLE ${t} ALTER COLUMN ${oq} SET DEFAULT ${newDef}`
        : `ALTER TABLE ${t} ALTER COLUMN ${oq} DROP DEFAULT`,
    );
  }
  if (form.name !== orig.name && dialect.supportsRenameColumn) {
    stmts.push(`ALTER TABLE ${t} RENAME COLUMN ${oq} TO ${qi(form.name, dialect)}`);
  }
  return stmts;
}

export function buildDropColSql(
  dialect: DialectInfo,
  database: string,
  table: string,
  name: string,
): string {
  return `ALTER TABLE ${tableRef(database, table, dialect)} DROP COLUMN ${qi(name, dialect)}`;
}

// ── Index SQL ────────────────────────────────────────────────────────────────

export interface IdxForm {
  name: string;
  selectedColumns: string[];
  unique: boolean;
  isPrimary: boolean;
}

export function buildAddIdxSql(
  dialect: DialectInfo,
  database: string,
  table: string,
  form: IdxForm,
): string {
  const t = tableRef(database, table, dialect);
  const cols = form.selectedColumns.map((c) => qi(c, dialect)).join(', ');
  if (form.isPrimary) {
    return `ALTER TABLE ${t} ADD PRIMARY KEY (${cols})`;
  }
  const name = form.name.trim() || `idx_${table}_${form.selectedColumns.join('_')}`;
  return `CREATE ${form.unique ? 'UNIQUE ' : ''}INDEX ${qi(name, dialect)} ON ${t} (${cols})`;
}

export function buildDropIdxSql(
  dialect: DialectInfo,
  database: string,
  table: string,
  name: string,
): string {
  const t = tableRef(database, table, dialect);
  const syntax = dialect.dropIndexSyntax ?? 'simple';
  if (syntax === 'on_table') {
    if (name === 'PRIMARY') return `ALTER TABLE ${t} DROP PRIMARY KEY`;
    return `DROP INDEX ${qi(name, dialect)} ON ${t}`;
  }
  if (syntax === 'schema_qualified')
    return `DROP INDEX ${qi(database, dialect)}.${qi(name, dialect)}`;
  if (syntax === 'on_table_no_schema') return `DROP INDEX ${qi(name, dialect)} ON ${t}`;
  return `DROP INDEX ${qi(name, dialect)}`;
}

// ── FK SQL ───────────────────────────────────────────────────────────────────

export interface FkForm {
  constraintName: string;
  selectedColumns: string[];
  referencedTable: string;
  referencedColumns: string;
  onDelete: string;
  onUpdate: string;
}

export function buildAddFkSql(
  dialect: DialectInfo,
  database: string,
  table: string,
  form: FkForm,
): string {
  const t = tableRef(database, table, dialect);
  const local = form.selectedColumns.map((c) => qi(c, dialect)).join(', ');
  const refCols = form.referencedColumns
    .split(',')
    .map((s) => qi(s.trim(), dialect))
    .join(', ');
  const name = form.constraintName.trim() || `fk_${table}_${form.selectedColumns.join('_')}`;
  return `ALTER TABLE ${t} ADD CONSTRAINT ${qi(name, dialect)} FOREIGN KEY (${local}) REFERENCES ${qi(form.referencedTable.trim(), dialect)} (${refCols}) ON DELETE ${form.onDelete} ON UPDATE ${form.onUpdate}`;
}

export function buildDropFkSql(
  dialect: DialectInfo,
  database: string,
  table: string,
  name: string,
): string {
  const t = tableRef(database, table, dialect);
  if (dialect.usesForeignKeyKeyword)
    return `ALTER TABLE ${t} DROP FOREIGN KEY ${qi(name, dialect)}`;
  return `ALTER TABLE ${t} DROP CONSTRAINT ${qi(name, dialect)}`;
}

// ── Check constraint SQL ─────────────────────────────────────────────────────

/**
 * MySQL/MariaDB use `ALTER TABLE ... DROP CHECK name`, while every other
 * engine modeled here uses the standard `ALTER TABLE ... DROP CONSTRAINT
 * name`. Ideally this would be modeled as a new `DialectInfo` capability
 * flag (the way `usesForeignKeyKeyword`/`dropIndexSyntax` already model
 * FK/index syntax variance), but `DialectInfo` values are constructed on the
 * Rust side (`src-tauri/src/connections/types.rs`) and this task is
 * frontend-only — adding a new field here without a corresponding Rust
 * change would either fail to compile against what's actually serialized,
 * or (if defaulted) silently be wrong for MySQL until Rust is updated. So,
 * as a narrow and deliberate exception to this codebase's "never branch on
 * engine-name strings" convention, we reuse `displayName` — already an
 * always-populated, per-engine field relied on elsewhere in this same file
 * (see the `sameEngine` check in `generateSyncSql`) — to special-case this
 * one bit of syntax. Follow-up: add a proper `dropCheckUsesCheckKeyword` (or
 * similar) `DialectInfo` flag next time the Rust side is touched, and drop
 * this string comparison in favor of it.
 */
function usesMySqlCheckDropSyntax(dialect: DialectInfo): boolean {
  return dialect.displayName === 'MySQL' || dialect.displayName === 'MariaDB';
}

export function buildAddCheckSql(
  dialect: DialectInfo,
  database: string,
  table: string,
  constraintName: string,
  expression: string,
): string {
  const t = tableRef(database, table, dialect);
  return `ALTER TABLE ${t} ADD CONSTRAINT ${qi(constraintName, dialect)} CHECK (${expression})`;
}

export function buildDropCheckSql(
  dialect: DialectInfo,
  database: string,
  table: string,
  constraintName: string,
): string {
  const t = tableRef(database, table, dialect);
  if (usesMySqlCheckDropSyntax(dialect)) {
    return `ALTER TABLE ${t} DROP CHECK ${qi(constraintName, dialect)}`;
  }
  return `ALTER TABLE ${t} DROP CONSTRAINT ${qi(constraintName, dialect)}`;
}

// ── Trigger SQL ──────────────────────────────────────────────────────────────

/**
 * `DROP TRIGGER <name>`, with no `IF EXISTS`/error-tolerance wrapper.
 *
 * This follows the same precedent already set by `buildDropIdxSql`,
 * `buildDropFkSql`, and `buildDropCheckSql` in this file: none of them emit
 * an `IF EXISTS` guard or any error-tolerant wrapper either — they assume the
 * object being dropped actually exists on the target (which the diff engine
 * has already confirmed) and emit the plain unconditional DROP. Some engines
 * (Oracle, SQL Server) don't even support `IF EXISTS` on `DROP TRIGGER`, so
 * inventing dialect branching here to add one would be new complexity this
 * codebase's other drop builders deliberately don't carry.
 */
function buildDropTriggerSql(dialect: DialectInfo, name: string): string {
  return `DROP TRIGGER ${qi(name, dialect)}`;
}

/**
 * Build the sync SQL for a trigger that needs to be (re)created on the
 * target side.
 *
 * Same-engine: a trigger's `definition` is already valid, engine-native DDL
 * as returned by the source connection, so it is copied through verbatim
 * (preceded by a drop, in case a same-named trigger already exists on the
 * target from a prior sync run) — no attempt is made to parse or rewrite it.
 *
 * Cross-engine: trigger bodies are procedural, engine-specific SQL (syntax
 * for referencing OLD/NEW rows, timing semantics, supported statement types
 * all vary widely across engines) with no reliable automated translation.
 * Rather than emit something that looks like a real DROP/CREATE pair but is
 * silently wrong, this emits only an inert `-- REVIEW` comment block quoting
 * the source definition for a human to translate and apply manually.
 */
export function buildTriggerSyncSql(
  trigger: TriggerInfo,
  sameEngine: boolean,
  dialect: DialectInfo,
): string {
  if (sameEngine) {
    return `${buildDropTriggerSql(dialect, trigger.name)};\n${trigger.definition}`;
  }
  const quotedDefinition = trigger.definition
    .split('\n')
    .map((line) => `-- ${line}`)
    .join('\n');
  return (
    `-- REVIEW: trigger "${trigger.name}" exists on the source side but trigger syntax is ` +
    `engine-specific and cannot be auto-translated across engines. Review and recreate ` +
    `manually if needed.\n-- Source definition:\n${quotedDefinition}`
  );
}

// ── Missing-object builders (schema compare / sync) ───────────────────────────

/**
 * Same-engine "table missing on target side" case: the caller already has a
 * `CREATE TABLE ...` DDL string fetched via `schemaApi.getDdl`, and can just
 * use it directly. This helper is a trivial passthrough kept only so call
 * sites read consistently alongside the other `build*` functions.
 */
export function buildCreateTableSql(sourceDdl: string): string {
  return sourceDdl;
}

/** Build a ColForm-shaped object from a ColumnInfo, for feeding into buildAddColSqls/buildEditColSqls. */
function columnInfoToForm(c: ColumnInfo): ColForm {
  return {
    mode: 'add',
    original: null,
    name: c.name,
    dataType: c.dataType,
    nullable: c.nullable,
    defaultValue: c.defaultValue ?? '',
    autoIncrement: c.isAutoIncrement,
    comment: c.comment ?? '',
  };
}

/**
 * Fallback CREATE TABLE builder from column info alone, used when a table is
 * entirely missing on the sync target and no real DDL string is available to
 * this module (it has no `invoke()` access by design). Callers targeting the
 * SAME engine on both sides should prefer fetching real DDL via the existing
 * `schemaApi.getDdl` and using it as-is (via `buildCreateTableSql`) instead of
 * this reconstruction, since it will be more faithful (default expressions,
 * collations, storage options, etc. are all lost here).
 */
function buildFallbackCreateTableSql(
  dialect: DialectInfo,
  database: string,
  table: string,
  columns: ColumnInfo[],
): string {
  const t = tableRef(database, table, dialect);
  const lines = columns.map((c) => colDef(dialect, columnInfoToForm(c)));
  const pkCols = columns.filter((c) => c.isPrimaryKey).map((c) => c.name);
  if (pkCols.length > 0) {
    lines.push(`PRIMARY KEY (${pkCols.map((n) => qi(n, dialect)).join(', ')})`);
  }
  return `CREATE TABLE ${t} (\n  ${lines.join(',\n  ')}\n)`;
}

function idxInfoToForm(idx: IndexInfo): IdxForm {
  return {
    name: idx.name,
    selectedColumns: idx.columns,
    unique: idx.unique,
    isPrimary: idx.name.toUpperCase() === 'PRIMARY',
  };
}

function fkInfoToForm(fk: ForeignKeyInfo): FkForm {
  return {
    constraintName: fk.constraintName,
    selectedColumns: fk.columns,
    referencedTable: fk.referencedTable,
    referencedColumns: fk.referencedColumns.join(', '),
    onDelete: fk.onDelete,
    onUpdate: fk.onUpdate,
  };
}

/**
 * Walk a SchemaDiff and produce the ordered list of SQL statements needed to
 * make the "target" side (the side NOT named first in `direction`) match the
 * "source" side. E.g. `direction: 'leftToRight'` makes the right side match
 * the left side: things only on the left get added to the right, things only
 * on the right get dropped from the right, and columns/indexes/foreign keys
 * that differ get altered on the right to match the left.
 *
 * Statement ordering is a fixed phase order (not a full dependency graph):
 * trigger drops, table drops, FK drops, check-constraint drops, index drops,
 * column drops, table creates, column adds, index adds, FK adds,
 * check-constraint adds, column alters, trigger creates. Check-constraint
 * drops sit alongside FK drops (both must happen before column drops, since
 * either kind of constraint could reference a column about to be dropped),
 * and check-constraint adds sit alongside FK adds (both after column adds,
 * so the columns they reference already exist). Trigger drops run first
 * (dropping a trigger has no dependents, so it's always safe early), while
 * trigger creates run last of all — after every table/column/index/FK/check-
 * constraint change — since a trigger body may reference columns that were
 * just added or constraints that were just created. This mirrors how
 * `TableStructure.svelte`'s `execSqls` already just runs statements in the
 * order it builds them; there is no attempt at topological sorting beyond
 * this fixed phase order.
 */
export function generateSyncSql(
  diff: SchemaDiff,
  direction: 'leftToRight' | 'rightToLeft',
  leftDialect: DialectInfo,
  rightDialect: DialectInfo,
  leftDatabase: string,
  rightDatabase: string,
): string[] {
  const targetIsRight = direction === 'leftToRight';
  const targetDialect = targetIsRight ? rightDialect : leftDialect;
  const targetDatabase = targetIsRight ? rightDatabase : leftDatabase;
  const sameEngine = leftDialect.displayName === rightDialect.displayName;

  const tableDrops: string[] = [];
  const tableCreates: string[] = [];
  const viewDrops: string[] = [];
  const viewCreates: string[] = [];
  const fkDrops: string[] = [];
  const fkAdds: string[] = [];
  const checkDrops: string[] = [];
  const checkAdds: string[] = [];
  const triggerDrops: string[] = [];
  const triggerCreates: string[] = [];
  const idxDrops: string[] = [];
  const idxAdds: string[] = [];
  const colDrops: string[] = [];
  const colAdds: string[] = [];
  const colAlters: string[] = [];

  /** For a diff entry, return the object that exists on the "source" side (never null when status matches). */
  function sourceOf<T>(d: ObjectDiff<T>): T {
    return (targetIsRight ? d.left : d.right) as T;
  }
  function targetOf<T>(d: ObjectDiff<T>): T {
    return (targetIsRight ? d.right : d.left) as T;
  }

  for (const tableDiff of diff.tables) {
    // A table wholly missing on one side: 'removed' = present on left only,
    // 'added' = present on right only.
    const wholeTableAction: 'create' | 'drop' | null =
      tableDiff.status === 'removed'
        ? targetIsRight
          ? 'create'
          : 'drop'
        : tableDiff.status === 'added'
          ? targetIsRight
            ? 'drop'
            : 'create'
          : null;

    if (wholeTableAction === 'drop') {
      tableDrops.push(`DROP TABLE ${tableRef(targetDatabase, tableDiff.tableName, targetDialect)}`);
      continue;
    }

    if (wholeTableAction === 'create') {
      const sourceColumns = tableDiff.columns.map((c) => sourceOf(c));
      // TODO: same-engine callers may prefer to fetch real DDL via
      // schemaApi.getDdl(...) and pass it through buildCreateTableSql()
      // instead of this column-based reconstruction, since it preserves
      // engine-specific details (defaults, collations, storage options)
      // that this fallback cannot see. This module has no invoke() access
      // by design, so it always falls back to reconstruction here.
      let createSql = buildFallbackCreateTableSql(
        targetDialect,
        targetDatabase,
        tableDiff.tableName,
        sourceColumns,
      );
      if (!sameEngine) {
        createSql = `-- REVIEW: types approximated across engines\n${createSql}`;
      }
      tableCreates.push(createSql);
      // Fall through to also emit ADD statements for indexes/foreign keys
      // below (they are not part of the fallback CREATE TABLE body), but
      // skip column-level statements since the CREATE TABLE already
      // includes every column.
    } else {
      // Only process column-level add/drop/alter statements when we did NOT
      // just emit a whole-table CREATE (which already contains every column).
      for (const c of tableDiff.columns) {
        if (c.status === 'unchanged') continue;
        if (c.status === 'changed') {
          const orig = targetOf(c);
          const form = columnInfoToForm(sourceOf(c));
          colAlters.push(
            ...buildEditColSqls(targetDialect, targetDatabase, tableDiff.tableName, orig, form),
          );
          continue;
        }
        // 'removed' = present on left only, 'added' = present on right only.
        const presentOnSourceOnly = (c.status === 'removed') === targetIsRight;
        if (presentOnSourceOnly) {
          colAdds.push(
            ...buildAddColSqls(
              targetDialect,
              targetDatabase,
              tableDiff.tableName,
              columnInfoToForm(sourceOf(c)),
            ),
          );
        } else {
          colDrops.push(
            buildDropColSql(targetDialect, targetDatabase, tableDiff.tableName, targetOf(c).name),
          );
        }
      }
    }

    for (const i of tableDiff.indexes) {
      if (i.status === 'unchanged') continue;
      if (i.status === 'changed') {
        idxDrops.push(
          buildDropIdxSql(targetDialect, targetDatabase, tableDiff.tableName, targetOf(i).name),
        );
        idxAdds.push(
          buildAddIdxSql(
            targetDialect,
            targetDatabase,
            tableDiff.tableName,
            idxInfoToForm(sourceOf(i)),
          ),
        );
        continue;
      }
      const presentOnSourceOnly = (i.status === 'removed') === targetIsRight;
      if (presentOnSourceOnly) {
        idxAdds.push(
          buildAddIdxSql(
            targetDialect,
            targetDatabase,
            tableDiff.tableName,
            idxInfoToForm(sourceOf(i)),
          ),
        );
      } else {
        idxDrops.push(
          buildDropIdxSql(targetDialect, targetDatabase, tableDiff.tableName, targetOf(i).name),
        );
      }
    }

    for (const f of tableDiff.foreignKeys) {
      if (f.status === 'unchanged') continue;
      if (f.status === 'changed') {
        fkDrops.push(
          buildDropFkSql(
            targetDialect,
            targetDatabase,
            tableDiff.tableName,
            targetOf(f).constraintName,
          ),
        );
        fkAdds.push(
          buildAddFkSql(
            targetDialect,
            targetDatabase,
            tableDiff.tableName,
            fkInfoToForm(sourceOf(f)),
          ),
        );
        continue;
      }
      const presentOnSourceOnly = (f.status === 'removed') === targetIsRight;
      if (presentOnSourceOnly) {
        fkAdds.push(
          buildAddFkSql(
            targetDialect,
            targetDatabase,
            tableDiff.tableName,
            fkInfoToForm(sourceOf(f)),
          ),
        );
      } else {
        fkDrops.push(
          buildDropFkSql(
            targetDialect,
            targetDatabase,
            tableDiff.tableName,
            targetOf(f).constraintName,
          ),
        );
      }
    }

    for (const cc of tableDiff.checkConstraints) {
      if (cc.status === 'unchanged') continue;
      if (cc.status === 'changed') {
        checkDrops.push(
          buildDropCheckSql(
            targetDialect,
            targetDatabase,
            tableDiff.tableName,
            targetOf(cc).constraintName,
          ),
        );
        const source = sourceOf<CheckConstraintInfo>(cc);
        checkAdds.push(
          buildAddCheckSql(
            targetDialect,
            targetDatabase,
            tableDiff.tableName,
            source.constraintName,
            source.expression,
          ),
        );
        continue;
      }
      const presentOnSourceOnly = (cc.status === 'removed') === targetIsRight;
      if (presentOnSourceOnly) {
        const source = sourceOf<CheckConstraintInfo>(cc);
        checkAdds.push(
          buildAddCheckSql(
            targetDialect,
            targetDatabase,
            tableDiff.tableName,
            source.constraintName,
            source.expression,
          ),
        );
      } else {
        checkDrops.push(
          buildDropCheckSql(
            targetDialect,
            targetDatabase,
            tableDiff.tableName,
            targetOf(cc).constraintName,
          ),
        );
      }
    }

    for (const t of tableDiff.triggers) {
      if (t.status === 'unchanged') continue;
      if (t.status === 'changed') {
        triggerCreates.push(
          buildTriggerSyncSql(sourceOf<TriggerInfo>(t), sameEngine, targetDialect),
        );
        continue;
      }
      const presentOnSourceOnly = (t.status === 'removed') === targetIsRight;
      if (presentOnSourceOnly) {
        triggerCreates.push(
          buildTriggerSyncSql(sourceOf<TriggerInfo>(t), sameEngine, targetDialect),
        );
      } else {
        triggerDrops.push(buildDropTriggerSql(targetDialect, targetOf(t).name));
      }
    }
  }

  for (const v of diff.views) {
    if (v.status === 'unchanged') continue;
    const viewRef = tableRef(targetDatabase, v.left?.name ?? v.right?.name ?? '', targetDialect);
    if (v.status === 'changed') {
      const source = sourceOf<ViewInfo>(v);
      viewDrops.push(`DROP VIEW ${viewRef}`);
      viewCreates.push(`CREATE VIEW ${viewRef} AS ${source.definition}`);
      continue;
    }
    const presentOnSourceOnly = (v.status === 'removed') === targetIsRight;
    if (presentOnSourceOnly) {
      const source = sourceOf<ViewInfo>(v);
      let createSql = `CREATE VIEW ${viewRef} AS ${source.definition}`;
      if (!sameEngine) createSql = `-- REVIEW: types approximated across engines\n${createSql}`;
      viewCreates.push(createSql);
    } else {
      viewDrops.push(`DROP VIEW ${viewRef}`);
    }
  }

  return [
    ...triggerDrops,
    ...viewDrops,
    ...tableDrops,
    ...fkDrops,
    ...checkDrops,
    ...idxDrops,
    ...colDrops,
    ...tableCreates,
    ...colAdds,
    ...idxAdds,
    ...fkAdds,
    ...checkAdds,
    ...colAlters,
    ...viewCreates,
    ...triggerCreates,
  ];
}
