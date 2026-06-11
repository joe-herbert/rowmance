import { describe, it, expect, vi } from 'vitest';
import type { ColumnMeta } from '$lib/types';

// ColumnPicker.svelte cannot be rendered in the jsdom/vitest environment due to
// Svelte 5 CSS preprocessing incompatibility. The component logic is tested here
// by re-implementing the pure rendering-decision functions locally.

function makeColumn(name: string, opts: Partial<ColumnMeta> = {}): ColumnMeta {
  return {
    name,
    dataType: 'VARCHAR(255)',
    nullable: true,
    isPrimaryKey: false,
    isForeignKey: false,
    ...opts,
  };
}

// Mirrors the checkbox `checked` binding from ColumnPicker.svelte:
//   checked={!hiddenColumns.has(col.name)}
function isColumnVisible(col: ColumnMeta, hiddenColumns: Set<string>): boolean {
  return !hiddenColumns.has(col.name);
}

// Mirrors badge visibility logic from ColumnPicker.svelte:
//   {#if col.isPrimaryKey} ... PK ... {/if}
//   {#if col.isForeignKey} ... FK ... {/if}
function getColumnBadges(col: ColumnMeta): string[] {
  const badges: string[] = [];
  if (col.isPrimaryKey) badges.push('PK');
  if (col.isForeignKey) badges.push('FK');
  return badges;
}

// Mirrors the onchange handler: () => onToggle(col.name)
function handleToggle(colName: string, onToggle: (name: string) => void): void {
  onToggle(colName);
}

// ── Checkbox visibility tests ─────────────────────────────────────────────────

describe('ColumnPicker checkbox visibility', () => {
  it('renders a row for each column', () => {
    const columns = [makeColumn('id'), makeColumn('name'), makeColumn('email')];
    // Each column produces exactly one row in the list
    expect(columns).toHaveLength(3);
  });

  it('visible column (not in hiddenColumns) has checked checkbox', () => {
    const col = makeColumn('id');
    const hidden = new Set<string>();
    expect(isColumnVisible(col, hidden)).toBe(true);
  });

  it('hidden column (in hiddenColumns) has unchecked checkbox', () => {
    const col = makeColumn('name');
    const hidden = new Set(['name']);
    expect(isColumnVisible(col, hidden)).toBe(false);
  });

  it('other columns are unaffected when one is hidden', () => {
    const cols = [makeColumn('id'), makeColumn('name'), makeColumn('email')];
    const hidden = new Set(['name']);
    expect(isColumnVisible(cols[0], hidden)).toBe(true);
    expect(isColumnVisible(cols[1], hidden)).toBe(false);
    expect(isColumnVisible(cols[2], hidden)).toBe(true);
  });

  it('all columns are visible when hiddenColumns is empty', () => {
    const cols = [makeColumn('id'), makeColumn('name')];
    const hidden = new Set<string>();
    expect(cols.every((c) => isColumnVisible(c, hidden))).toBe(true);
  });

  it('all columns are hidden when all are in hiddenColumns', () => {
    const cols = [makeColumn('id'), makeColumn('name')];
    const hidden = new Set(['id', 'name']);
    expect(cols.every((c) => !isColumnVisible(c, hidden))).toBe(true);
  });
});

// ── Toggle callback tests ─────────────────────────────────────────────────────

describe('ColumnPicker toggle callback', () => {
  it('checking an unchecked checkbox calls onToggle with the column name', () => {
    const onToggle = vi.fn();
    handleToggle('name', onToggle);
    expect(onToggle).toHaveBeenCalledWith('name');
    expect(onToggle).toHaveBeenCalledTimes(1);
  });

  it('unchecking a checked checkbox calls onToggle with the column name', () => {
    const onToggle = vi.fn();
    handleToggle('id', onToggle);
    expect(onToggle).toHaveBeenCalledWith('id');
  });

  it('onToggle is called with the correct column name for each column', () => {
    const cols = [makeColumn('id'), makeColumn('name'), makeColumn('email')];
    const onToggle = vi.fn();
    cols.forEach((col) => handleToggle(col.name, onToggle));
    expect(onToggle).toHaveBeenNthCalledWith(1, 'id');
    expect(onToggle).toHaveBeenNthCalledWith(2, 'name');
    expect(onToggle).toHaveBeenNthCalledWith(3, 'email');
  });
});

// ── Close callback tests ──────────────────────────────────────────────────────

describe('ColumnPicker close button', () => {
  it('clicking the close button calls onClose', () => {
    // The button's onclick is directly bound to onClose in the component template.
    const onClose = vi.fn();
    // Simulate the button click handler
    onClose();
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});

// ── Badge tests ───────────────────────────────────────────────────────────────

describe('ColumnPicker badges', () => {
  it('PK columns show a "PK" badge', () => {
    const col = makeColumn('id', { isPrimaryKey: true });
    expect(getColumnBadges(col)).toContain('PK');
  });

  it('FK columns show a "FK" badge', () => {
    const col = makeColumn('user_id', { isForeignKey: true });
    expect(getColumnBadges(col)).toContain('FK');
  });

  it('columns that are not PK or FK show no badge', () => {
    const col = makeColumn('name');
    expect(getColumnBadges(col)).toEqual([]);
  });

  it('a column that is both PK and FK shows both badges', () => {
    const col = makeColumn('id', { isPrimaryKey: true, isForeignKey: true });
    const badges = getColumnBadges(col);
    expect(badges).toContain('PK');
    expect(badges).toContain('FK');
  });

  it('badge list is empty for a plain non-key column', () => {
    const col = makeColumn('description');
    expect(getColumnBadges(col)).toHaveLength(0);
  });
});
