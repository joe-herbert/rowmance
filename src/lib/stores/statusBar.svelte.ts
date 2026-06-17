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

let pendingCount = $state(0);
let rowCount = $state<number | null>(null);
let lastQueryMs = $state<number | null>(null);
let isSaving = $state(false);
let onSave = $state<(() => Promise<void>) | null>(null);
let onDiscard = $state<(() => void) | null>(null);

export function useStatusBar() {
  return {
    get pendingCount() { return pendingCount; },
    get rowCount() { return rowCount; },
    get lastQueryMs() { return lastQueryMs; },
    get isSaving() { return isSaving; },
    get onSave() { return onSave; },
    get onDiscard() { return onDiscard; },

    update(patch: {
      pendingCount?: number;
      rowCount?: number | null;
      lastQueryMs?: number | null;
      isSaving?: boolean;
      onSave?: (() => Promise<void>) | null;
      onDiscard?: (() => void) | null;
    }) {
      if (patch.pendingCount !== undefined) pendingCount = patch.pendingCount;
      if (patch.rowCount !== undefined) rowCount = patch.rowCount;
      if (patch.lastQueryMs !== undefined) lastQueryMs = patch.lastQueryMs;
      if (patch.isSaving !== undefined) isSaving = patch.isSaving;
      if (patch.onSave !== undefined) onSave = patch.onSave;
      if (patch.onDiscard !== undefined) onDiscard = patch.onDiscard;
    },

    clear() {
      pendingCount = 0;
      rowCount = null;
      lastQueryMs = null;
      isSaving = false;
      onSave = null;
      onDiscard = null;
    },
  };
}
