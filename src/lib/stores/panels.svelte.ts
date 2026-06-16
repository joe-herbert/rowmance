/**
 * Panel layout store.
 * Manages the split-panel state for the main area of the application.
 * Up to four panels can be open in a 2×2 grid.
 *
 * Open items are tracked independently of the panel layout — closing a panel
 * does not remove items from the open list. Items are removed only via closeOpenItem().
 */
import type { PanelState, PanelKind, SplitMode } from '$lib/types';

export interface OpenItem {
  id: string;
  content: PanelKind;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

let nextId = 0;

function createId(): string {
  return `panel-${++nextId}`;
}

function makePanel(content: PanelKind): PanelState {
  return { id: createId(), content };
}

/**
 * Returns true when two PanelKind values represent the same logical item.
 * query_editor instances are always treated as distinct.
 */
export function sameContent(a: PanelKind, b: PanelKind): boolean {
  if (a.kind !== b.kind) return false;
  switch (a.kind) {
    case 'table_browser':
      return b.kind === 'table_browser' && a.connectionId === b.connectionId && a.database === b.database && a.table === b.table;
    case 'ddl_viewer':
      return b.kind === 'ddl_viewer' && a.connectionId === b.connectionId && a.database === b.database && a.objectName === b.objectName;
    case 'erd':
      return b.kind === 'erd' && a.connectionId === b.connectionId && a.database === b.database;
    case 'explain':
      return b.kind === 'explain' && a.connectionId === b.connectionId && a.sql === b.sql;
    case 'settings':
      return true;
    case 'query_editor':
      return false;
    case 'empty':
      return true;
  }
}

// ── State ─────────────────────────────────────────────────────────────────────

let panels = $state<PanelState[]>([makePanel({ kind: 'empty' })]);
let splitMode = $state<SplitMode>('none');
let focusedIndex = $state(0);
let openItems = $state<OpenItem[]>([]);

// ── Split mode transitions ────────────────────────────────────────────────────

/**
 * Determines the new SplitMode after adding a panel in the given direction.
 * Follows the state machine defined in the plan:
 *   none + right  → horizontal
 *   none + down   → vertical
 *   horizontal + down → quad
 *   vertical + right  → quad
 */
function nextSplitMode(current: SplitMode, direction: 'right' | 'down'): SplitMode {
  if (current === 'none') return direction === 'right' ? 'horizontal' : 'vertical';
  if (current === 'horizontal' && direction === 'down') return 'quad';
  if (current === 'vertical' && direction === 'right') return 'quad';
  return current;
}

/**
 * Determines the SplitMode after removing a panel.
 *   quad - 1 → horizontal or vertical (depends on remaining layout)
 *   horizontal/vertical - 1 → none
 */
function previousSplitMode(current: SplitMode): SplitMode {
  if (current === 'quad') return 'horizontal'; // caller adjusts as needed
  return 'none';
}

// ── Public interface ──────────────────────────────────────────────────────────

export function usePanels() {
  return {
    get panels() {
      return panels;
    },
    get splitMode() {
      return splitMode;
    },
    get focusedIndex() {
      return focusedIndex;
    },
    get panelCount() {
      return panels.length;
    },
    get focusedPanel(): PanelState {
      return panels[focusedIndex];
    },
    get openItems() {
      return openItems;
    },

    /**
     * Open content in the currently focused panel.
     * - If the content is already shown in another panel, focuses that panel instead.
     * - Adds the item to the open list (deduplicated by sameContent).
     * - query_editor always adds a new entry (each session is distinct).
     */
    openInFocused(content: PanelKind) {
      if (content.kind !== 'empty') {
        // If already visible in some panel, just focus it
        const existingPanelIdx = panels.findIndex(p => sameContent(p.content, content));
        if (existingPanelIdx !== -1) {
          focusedIndex = existingPanelIdx;
          return;
        }
        // Add to open items if not already tracked
        if (!openItems.find(item => sameContent(item.content, content))) {
          openItems = [...openItems, { id: createId(), content }];
        }
      }
      panels = panels.map((p, i) => (i === focusedIndex ? { ...p, content } : p));
    },

    /** Show an already-tracked open item in the focused panel. */
    showItem(item: OpenItem) {
      // If item is already visible in some panel, focus that panel
      const existingPanelIdx = panels.findIndex(p => sameContent(p.content, item.content));
      if (existingPanelIdx !== -1) {
        focusedIndex = existingPanelIdx;
        return;
      }
      panels = panels.map((p, i) => (i === focusedIndex ? { ...p, content: item.content } : p));
    },

    /** Remove an item from the open list and reset any panel showing it to empty. */
    closeOpenItem(itemId: string) {
      const item = openItems.find(i => i.id === itemId);
      if (!item) return;
      openItems = openItems.filter(i => i.id !== itemId);
      panels = panels.map(p =>
        sameContent(p.content, item.content) ? { ...p, content: { kind: 'empty' } } : p
      );
      // Clamp focused index in case the layout visually changes
      focusedIndex = Math.min(focusedIndex, panels.length - 1);
    },

    /** Split the layout by adding a new panel in the given direction. */
    split(direction: 'right' | 'down', content: PanelKind = { kind: 'empty' }) {
      if (panels.length >= 4) return;
      splitMode = nextSplitMode(splitMode, direction);
      panels = [...panels, makePanel(content)];
      focusedIndex = panels.length - 1;
    },

    /** Close the panel at the given index. */
    closePanel(index: number) {
      if (panels.length === 1) {
        // Always keep at least one panel; reset it to empty instead.
        panels = [makePanel({ kind: 'empty' })];
        splitMode = 'none';
        focusedIndex = 0;
        return;
      }

      panels = panels.filter((_, i) => i !== index);
      splitMode = previousSplitMode(splitMode);
      focusedIndex = Math.min(focusedIndex, panels.length - 1);
    },

    /** Move keyboard focus to the next panel (wraps around). */
    focusNext() {
      focusedIndex = (focusedIndex + 1) % panels.length;
    },

    /** Move keyboard focus to the previous panel (wraps around). */
    focusPrev() {
      focusedIndex = (focusedIndex - 1 + panels.length) % panels.length;
    },

    /** Set focus to a specific panel by index. */
    focus(index: number) {
      if (index >= 0 && index < panels.length) {
        focusedIndex = index;
      }
    },
  };
}
