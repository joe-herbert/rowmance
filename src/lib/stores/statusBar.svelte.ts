/**
 * Status bar store.
 * The focused panel writes its transient state here so the AppShell status bar
 * can display it without prop-drilling through the layout tree.
 *
 * Each field is a separate reactive variable so callers can write individual
 * fields without reading (and therefore tracking) the others — avoiding the
 * effect_update_depth_exceeded infinite loop that a single spread-object store
 * would cause.
 */

let pendingCellCount = $state(0);
let pendingRowCount = $state(0);
let rowCount = $state<number | null>(null);
let totalRowCount = $state<number | null>(null);
let lastQueryMs = $state<number | null>(null);
let isSaving = $state(false);
let onSave = $state<(() => Promise<void>) | null>(null);
let onDiscard = $state<(() => void) | null>(null);

export function useStatusBar() {
  return {
    get pendingCellCount() { return pendingCellCount; },
    get pendingRowCount() { return pendingRowCount; },
    get rowCount() { return rowCount; },
    get totalRowCount() { return totalRowCount; },
    get lastQueryMs() { return lastQueryMs; },
    get isSaving() { return isSaving; },
    get onSave() { return onSave; },
    get onDiscard() { return onDiscard; },

    update(patch: {
      pendingCellCount?: number;
      pendingRowCount?: number;
      rowCount?: number | null;
      totalRowCount?: number | null;
      lastQueryMs?: number | null;
      isSaving?: boolean;
      onSave?: (() => Promise<void>) | null;
      onDiscard?: (() => void) | null;
    }) {
      if (patch.pendingCellCount !== undefined) pendingCellCount = patch.pendingCellCount;
      if (patch.pendingRowCount !== undefined) pendingRowCount = patch.pendingRowCount;
      if (patch.rowCount !== undefined) rowCount = patch.rowCount;
      if (patch.totalRowCount !== undefined) totalRowCount = patch.totalRowCount;
      if (patch.lastQueryMs !== undefined) lastQueryMs = patch.lastQueryMs;
      if (patch.isSaving !== undefined) isSaving = patch.isSaving;
      if (patch.onSave !== undefined) onSave = patch.onSave;
      if (patch.onDiscard !== undefined) onDiscard = patch.onDiscard;
    },

    clear() {
      pendingCellCount = 0;
      pendingRowCount = 0;
      rowCount = null;
      totalRowCount = null;
      lastQueryMs = null;
      isSaving = false;
      onSave = null;
      onDiscard = null;
    },
  };
}
