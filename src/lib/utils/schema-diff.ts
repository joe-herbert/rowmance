/**
 * Pure schema-diff engine for the database/table structure compare feature.
 *
 * No `invoke()` calls and no Svelte/store imports — everything here operates
 * on plain data passed in by the caller, so it is trivially unit-testable.
 * The UI layer is responsible for fetching the raw per-side data (via the
 * `$lib/tauri/schema` wrappers) and assembling it into the shapes below.
 */
import type {
  ColumnInfo,
  IndexInfo,
  ForeignKeyInfo,
  TableInfo,
  ViewInfo,
  CheckConstraintInfo,
  TriggerInfo,
} from '$lib/types';

export type DiffStatus = 'added' | 'removed' | 'changed' | 'unchanged';

export interface FieldDiff {
  field: string;
  left: unknown;
  right: unknown;
}

export interface ObjectDiff<T> {
  key: string;
  status: DiffStatus;
  left: T | null;
  right: T | null;
  /** Populated only when status === 'changed'. */
  fieldDiffs: FieldDiff[];
  /** True when the only diff is a type-text mismatch AND crossEngine is true. */
  crossEngineCaveat?: boolean;
}

export interface TableDiff {
  tableName: string;
  status: DiffStatus;
  columns: ObjectDiff<ColumnInfo>[];
  indexes: ObjectDiff<IndexInfo>[];
  foreignKeys: ObjectDiff<ForeignKeyInfo>[];
  checkConstraints: ObjectDiff<CheckConstraintInfo>[];
  triggers: ObjectDiff<TriggerInfo>[];
}

export interface SchemaDiff {
  tables: TableDiff[];
  views: ObjectDiff<ViewInfo>[];
  crossEngine: boolean;
}

// ── Generic name-matching diff helper ─────────────────────────────────────────

/**
 * Diff two arrays of named objects, matching by name case-insensitively but
 * preserving original casing in the returned `left`/`right` values.
 */
function diffByName<T>(
  left: T[],
  right: T[],
  getName: (item: T) => string,
  fields: (keyof T)[],
  crossEngineCaveatField?: keyof T,
  crossEngine?: boolean,
): ObjectDiff<T>[] {
  const leftByKey = new Map<string, T>();
  for (const item of left) leftByKey.set(getName(item).toLowerCase(), item);
  const rightByKey = new Map<string, T>();
  for (const item of right) rightByKey.set(getName(item).toLowerCase(), item);

  const allKeys = new Set<string>([...leftByKey.keys(), ...rightByKey.keys()]);
  const result: ObjectDiff<T>[] = [];

  for (const key of allKeys) {
    const l = leftByKey.get(key) ?? null;
    const r = rightByKey.get(key) ?? null;

    if (l && !r) {
      result.push({ key, status: 'removed', left: l, right: null, fieldDiffs: [] });
      continue;
    }
    if (!l && r) {
      result.push({ key, status: 'added', left: null, right: r, fieldDiffs: [] });
      continue;
    }
    if (!l || !r) continue; // unreachable, satisfies TS

    const fieldDiffs: FieldDiff[] = [];
    for (const field of fields) {
      const lv = l[field];
      const rv = r[field];
      if (!deepEqual(lv, rv)) {
        fieldDiffs.push({ field: String(field), left: lv, right: rv });
      }
    }

    if (fieldDiffs.length === 0) {
      result.push({ key, status: 'unchanged', left: l, right: r, fieldDiffs: [] });
      continue;
    }

    const isSoleCaveatField =
      crossEngine === true &&
      crossEngineCaveatField !== undefined &&
      fieldDiffs.length === 1 &&
      fieldDiffs[0].field === String(crossEngineCaveatField);

    result.push({
      key,
      status: 'changed',
      left: l,
      right: r,
      fieldDiffs,
      ...(isSoleCaveatField ? { crossEngineCaveat: true } : {}),
    });
  }

  return result;
}

function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) return false;
    return a.every((v, i) => deepEqual(v, b[i]));
  }
  return false;
}

// ── Public diff functions ─────────────────────────────────────────────────────

const COLUMN_FIELDS: (keyof ColumnInfo)[] = [
  'dataType',
  'nullable',
  'defaultValue',
  'isPrimaryKey',
  'isAutoIncrement',
  'isForeignKey',
  'comment',
];

export function diffColumns(
  left: ColumnInfo[],
  right: ColumnInfo[],
  crossEngine: boolean,
): ObjectDiff<ColumnInfo>[] {
  return diffByName(left, right, (c) => c.name, COLUMN_FIELDS, 'dataType', crossEngine);
}

const INDEX_FIELDS: (keyof IndexInfo)[] = ['columns', 'unique', 'indexType'];

export function diffIndexes(left: IndexInfo[], right: IndexInfo[]): ObjectDiff<IndexInfo>[] {
  return diffByName(left, right, (i) => i.name, INDEX_FIELDS);
}

const FK_FIELDS: (keyof ForeignKeyInfo)[] = [
  'columns',
  'referencedTable',
  'referencedColumns',
  'onDelete',
  'onUpdate',
];

export function diffForeignKeys(
  left: ForeignKeyInfo[],
  right: ForeignKeyInfo[],
): ObjectDiff<ForeignKeyInfo>[] {
  return diffByName(left, right, (fk) => fk.constraintName, FK_FIELDS);
}

const CHECK_CONSTRAINT_FIELDS: (keyof CheckConstraintInfo)[] = ['expression'];

/**
 * Diff check constraints by name.
 *
 * Deliberately does NOT apply the `crossEngineCaveat` softening to a
 * differing `expression`, even when `crossEngine` is true. That caveat
 * exists for `diffColumns`' `dataType` field to signal "this is merely a
 * type-name spelling difference between engines (e.g. INT vs INTEGER); the
 * generated sync SQL is still safe/meaningful as-is." A check constraint's
 * `expression` is arbitrary engine-specific SQL (functions, operators,
 * casts, quoting) — a mismatch there is not a spelling variant of the same
 * rule, it can be a genuinely different constraint. Surfacing it as a
 * normal, un-softened `changed` diff (full text shown, no "safe to
 * auto-apply" implication) is the more honest default.
 */
export function diffCheckConstraints(
  left: CheckConstraintInfo[],
  right: CheckConstraintInfo[],
): ObjectDiff<CheckConstraintInfo>[] {
  return diffByName(left, right, (c) => c.constraintName, CHECK_CONSTRAINT_FIELDS);
}

const TRIGGER_FIELDS: (keyof TriggerInfo)[] = ['timing', 'event', 'definition'];

/**
 * Diff triggers by name.
 *
 * Deliberately does NOT apply the `crossEngineCaveat` softening to a
 * differing `definition`, for the same reason `diffCheckConstraints` doesn't
 * soften `expression`: the caveat exists for `diffColumns`' `dataType` field
 * to signal "this is merely a type-name spelling difference between engines;
 * the generated sync SQL is still safe/meaningful as-is." A trigger's
 * `definition` is arbitrary, fully engine-specific procedural SQL — a
 * mismatch there is never a harmless spelling variant, and `generateSyncSql`
 * refuses to translate it across engines at all (see `buildTriggerSyncSql`).
 * Surfacing it as a normal, un-softened `changed` diff is the honest default.
 */
export function diffTriggers(left: TriggerInfo[], right: TriggerInfo[]): ObjectDiff<TriggerInfo>[] {
  return diffByName(left, right, (t) => t.name, TRIGGER_FIELDS);
}

const VIEW_FIELDS: (keyof ViewInfo)[] = ['definition'];

export function diffViews(left: ViewInfo[], right: ViewInfo[]): ObjectDiff<ViewInfo>[] {
  return diffByName(left, right, (v) => v.name, VIEW_FIELDS);
}

export function diffTables(
  left: TableInfo[],
  right: TableInfo[],
): { added: string[]; removed: string[]; common: string[] } {
  const leftByKey = new Map<string, string>();
  for (const t of left) leftByKey.set(t.name.toLowerCase(), t.name);
  const rightByKey = new Map<string, string>();
  for (const t of right) rightByKey.set(t.name.toLowerCase(), t.name);

  const added: string[] = [];
  const removed: string[] = [];
  const common: string[] = [];

  const allKeys = new Set<string>([...leftByKey.keys(), ...rightByKey.keys()]);
  for (const key of allKeys) {
    const l = leftByKey.get(key);
    const r = rightByKey.get(key);
    if (l && r) common.push(l);
    else if (l && !r) removed.push(l);
    else if (!l && r) added.push(r);
  }

  return { added, removed, common };
}

// ── Higher-level composition ───────────────────────────────────────────────────

export interface TableSideData {
  columns: ColumnInfo[];
  indexes: IndexInfo[];
  foreignKeys: ForeignKeyInfo[];
  /** Optional and defaulted to `[]` so existing callers/tests that predate
   *  check-constraint support keep compiling without changes. */
  checkConstraints?: CheckConstraintInfo[];
  /** Optional and defaulted to `[]` so existing callers/tests that predate
   *  trigger support keep compiling without changes. */
  triggers?: TriggerInfo[];
}

export function diffTable(
  tableName: string,
  left: TableSideData,
  right: TableSideData,
  crossEngine: boolean,
): TableDiff {
  const columns = diffColumns(left.columns, right.columns, crossEngine);
  const indexes = diffIndexes(left.indexes, right.indexes);
  const foreignKeys = diffForeignKeys(left.foreignKeys, right.foreignKeys);
  const checkConstraints = diffCheckConstraints(
    left.checkConstraints ?? [],
    right.checkConstraints ?? [],
  );
  const triggers = diffTriggers(left.triggers ?? [], right.triggers ?? []);

  const hasChildDiffs = [
    ...columns,
    ...indexes,
    ...foreignKeys,
    ...checkConstraints,
    ...triggers,
  ].some((d) => d.status !== 'unchanged');

  return {
    tableName,
    status: hasChildDiffs ? 'changed' : 'unchanged',
    columns,
    indexes,
    foreignKeys,
    checkConstraints,
    triggers,
  };
}

export interface DatabaseSideData {
  tables: TableInfo[];
  tableDetails: Map<string, TableSideData>;
  views: ViewInfo[];
}

export function diffDatabase(
  left: DatabaseSideData,
  right: DatabaseSideData,
  crossEngine: boolean,
): SchemaDiff {
  const { added, removed, common } = diffTables(left.tables, right.tables);
  const emptySide: TableSideData = {
    columns: [],
    indexes: [],
    foreignKeys: [],
    checkConstraints: [],
    triggers: [],
  };

  const tables: TableDiff[] = [];

  for (const name of removed) {
    const leftData = left.tableDetails.get(name) ?? emptySide;
    tables.push({
      tableName: name,
      status: 'removed',
      columns: leftData.columns.map((c) => ({
        key: c.name.toLowerCase(),
        status: 'removed' as DiffStatus,
        left: c,
        right: null,
        fieldDiffs: [],
      })),
      indexes: leftData.indexes.map((i) => ({
        key: i.name.toLowerCase(),
        status: 'removed' as DiffStatus,
        left: i,
        right: null,
        fieldDiffs: [],
      })),
      foreignKeys: leftData.foreignKeys.map((fk) => ({
        key: fk.constraintName.toLowerCase(),
        status: 'removed' as DiffStatus,
        left: fk,
        right: null,
        fieldDiffs: [],
      })),
      checkConstraints: (leftData.checkConstraints ?? []).map((cc) => ({
        key: cc.constraintName.toLowerCase(),
        status: 'removed' as DiffStatus,
        left: cc,
        right: null,
        fieldDiffs: [],
      })),
      triggers: (leftData.triggers ?? []).map((t) => ({
        key: t.name.toLowerCase(),
        status: 'removed' as DiffStatus,
        left: t,
        right: null,
        fieldDiffs: [],
      })),
    });
  }

  for (const name of added) {
    const rightData = right.tableDetails.get(name) ?? emptySide;
    tables.push({
      tableName: name,
      status: 'added',
      columns: rightData.columns.map((c) => ({
        key: c.name.toLowerCase(),
        status: 'added' as DiffStatus,
        left: null,
        right: c,
        fieldDiffs: [],
      })),
      indexes: rightData.indexes.map((i) => ({
        key: i.name.toLowerCase(),
        status: 'added' as DiffStatus,
        left: null,
        right: i,
        fieldDiffs: [],
      })),
      foreignKeys: rightData.foreignKeys.map((fk) => ({
        key: fk.constraintName.toLowerCase(),
        status: 'added' as DiffStatus,
        left: null,
        right: fk,
        fieldDiffs: [],
      })),
      checkConstraints: (rightData.checkConstraints ?? []).map((cc) => ({
        key: cc.constraintName.toLowerCase(),
        status: 'added' as DiffStatus,
        left: null,
        right: cc,
        fieldDiffs: [],
      })),
      triggers: (rightData.triggers ?? []).map((t) => ({
        key: t.name.toLowerCase(),
        status: 'added' as DiffStatus,
        left: null,
        right: t,
        fieldDiffs: [],
      })),
    });
  }

  for (const name of common) {
    const leftData = left.tableDetails.get(name) ?? emptySide;
    const rightData = right.tableDetails.get(name) ?? emptySide;
    tables.push(diffTable(name, leftData, rightData, crossEngine));
  }

  return {
    tables,
    views: diffViews(left.views, right.views),
    crossEngine,
  };
}

// ── Direction-aware display re-orientation ─────────────────────────────────────
//
// diffTable/diffDatabase are always computed against a fixed (left, right)
// pair: `status: 'added'` means "present in right only", `'removed'` means
// "present in left only", and every `left`/`right` value is physical-side-
// relative. The sync-direction toggle in the UI lets either physical side be
// the "source of truth" (the other is the "target" that gets modified), and
// the diff tree should always read consistently from that point of view:
//   - 'added'/'removed' should always mean "will be created on" / "will be
//     dropped from" the target — regardless of which physical side that is.
//   - A `changed` row's left/right values should always show source first,
//     target second, matching the header's source → target ordering.
//
// These two adjustments are NOT the same transform. Direction 'leftToRight'
// (source = left, target = right) already has left/right values in the
// desired source-first/target-second order — physical left IS the source —
// so only the status labels need swapping (raw 'removed' = present in left/
// source only = needs to be *created* on the target, i.e. displayed as
// 'added'; raw 'added' = present in right/target only = needs to be
// *dropped*, i.e. displayed as 'removed'). Direction 'rightToLeft' (source =
// right, target = left) has the opposite problem: the raw status labels are
// already correct (raw 'added' = present in right/source only = needs
// creating = 'added'), but the left/right VALUES are in target-first/
// source-second order and need swapping to read source-first. Conflating
// these into a single "swap everything" transform (as an earlier version of
// this function did) produces a tree whose per-row values run in the
// opposite direction from the header for one of the two directions — exactly
// the "comparisons run backwards" bug this split avoids.
//
// SQL generation is unaffected by any of this: it derives create/drop from
// the raw (unreoriented) diff plus its own `direction` parameter — see
// `generateSyncSql` in ddl-generation.ts.

type Direction = 'leftToRight' | 'rightToLeft';

function flipStatus(status: DiffStatus): DiffStatus {
  if (status === 'added') return 'removed';
  if (status === 'removed') return 'added';
  return status;
}

export function reorientObjectDiff<T>(d: ObjectDiff<T>, direction: Direction): ObjectDiff<T> {
  if (direction === 'leftToRight') {
    return { ...d, status: flipStatus(d.status) };
  }
  return {
    ...d,
    left: d.right,
    right: d.left,
    fieldDiffs: d.fieldDiffs.map((fd) => ({ ...fd, left: fd.right, right: fd.left })),
  };
}

export function reorientTableDiff(t: TableDiff, direction: Direction): TableDiff {
  return {
    ...t,
    status: direction === 'leftToRight' ? flipStatus(t.status) : t.status,
    columns: t.columns.map((c) => reorientObjectDiff(c, direction)),
    indexes: t.indexes.map((i) => reorientObjectDiff(i, direction)),
    foreignKeys: t.foreignKeys.map((fk) => reorientObjectDiff(fk, direction)),
    checkConstraints: t.checkConstraints.map((cc) => reorientObjectDiff(cc, direction)),
    triggers: t.triggers.map((tr) => reorientObjectDiff(tr, direction)),
  };
}

export function reorientSchemaDiff(s: SchemaDiff, direction: Direction): SchemaDiff {
  return {
    tables: s.tables.map((t) => reorientTableDiff(t, direction)),
    views: s.views.map((v) => reorientObjectDiff(v, direction)),
    crossEngine: s.crossEngine,
  };
}
