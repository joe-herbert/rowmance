/**
 * Panel layout store.
 * Manages the split-panel state for the main area of the application.
 * Up to four panels can be open in a 2×2 grid.
 *
 * Open items are tracked independently of the panel layout — closing a panel
 * does not remove items from the open list. Items are removed only via closeOpenItem().
 */
import { untrack } from 'svelte';
import type { PanelState, PanelKind, SplitMode } from '$lib/types';
import { queryEditorCache } from './queryEditorState';
import { clearTableFilterCache } from '$lib/stores/tableBrowserFilterCache';

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
      return (
        b.kind === 'table_browser' &&
        a.connectionId === b.connectionId &&
        a.database === b.database &&
        a.table === b.table
      );
    case 'table_structure':
      return (
        b.kind === 'table_structure' &&
        a.connectionId === b.connectionId &&
        a.database === b.database &&
        a.table === b.table
      );
    case 'ddl_viewer':
      return (
        b.kind === 'ddl_viewer' &&
        a.connectionId === b.connectionId &&
        a.database === b.database &&
        a.objectName === b.objectName
      );
    case 'erd':
      return b.kind === 'erd' && a.connectionId === b.connectionId && a.database === b.database;
    case 'explain':
      return b.kind === 'explain' && a.connectionId === b.connectionId && a.sql === b.sql;
    case 'settings':
      return true;
    case 'query_editor':
      return b.kind === 'query_editor' && !!a.editorId && a.editorId === b.editorId;
    case 'user_manager':
      return b.kind === 'user_manager' && a.connectionId === b.connectionId;
    case 'speed_analysis':
      return true;
    case 'empty':
      return true;
  }
}

// ── State ─────────────────────────────────────────────────────────────────────

let panels = $state<PanelState[]>([makePanel({ kind: 'empty' })]);
let splitMode = $state<SplitMode>('none');
let focusedIndex = $state(0);
let openItems = $state<OpenItem[]>([]);
let dirtyItemKeys = $state<Set<string>>(new Set());

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

export function dirtyKeyForContent(content: PanelKind): string | null {
  if (content.kind === 'table_browser')
    return `${content.connectionId}:${content.database}:${content.table}`;
  if (content.kind === 'table_structure')
    return `${content.connectionId}:${content.database}:${content.table}`;
  if (content.kind === 'ddl_viewer')
    return `${content.connectionId}:${content.database}:${content.objectName}`;
  if (content.kind === 'query_editor' && content.editorId) return `query:${content.editorId}`;
  return null;
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
      // Stamp a unique editorId on query_editor content so sameContent can match instances.
      if (content.kind === 'query_editor' && !content.editorId) {
        content = { ...content, editorId: createId() };
      }
      // For explicit filter navigation (FK click, relations panel), clear the cached filter
      // so the initialFilter prop takes effect instead of the stale cached state.
      if (content.kind === 'table_browser' && content.initialFilter) {
        clearTableFilterCache(`${content.connectionId}:${content.database}:${content.table}`);
      }
      if (content.kind !== 'empty') {
        // If already visible in some panel, just focus it
        const existingPanelIdx = panels.findIndex((p) => sameContent(p.content, content));
        if (existingPanelIdx !== -1) {
          // Update the panel content so the initialFilter prop propagates to the mounted component.
          if (content.kind === 'table_browser' && content.initialFilter) {
            panels = panels.map((p, i) => (i === existingPanelIdx ? { ...p, content } : p));
          }
          focusedIndex = existingPanelIdx;
          return;
        }
        // Add to open items if not already tracked
        if (!openItems.find((item) => sameContent(item.content, content))) {
          openItems = [...openItems, { id: createId(), content }];
        }
      }
      panels = panels.map((p, i) => (i === focusedIndex ? { ...p, content } : p));
    },

    /**
     * Replace the focused panel's content and update its open item entry in-place.
     * Used when switching view modes (Data / Structure / SQL) so the open list
     * shows one item for the table rather than accumulating entries.
     */
    replaceInFocused(content: PanelKind) {
      const currentContent = panels[focusedIndex].content;
      const itemIndex = openItems.findIndex((item) => sameContent(item.content, currentContent));
      if (itemIndex !== -1) {
        openItems = openItems.map((item, i) => (i === itemIndex ? { ...item, content } : item));
      } else {
        openItems = [...openItems, { id: createId(), content }];
      }
      panels = panels.map((p, i) => (i === focusedIndex ? { ...p, content } : p));
    },

    /** Show an already-tracked open item in the focused panel. */
    showItem(item: OpenItem) {
      // If item is already visible in some panel, focus that panel
      const existingPanelIdx = panels.findIndex((p) => sameContent(p.content, item.content));
      if (existingPanelIdx !== -1) {
        focusedIndex = existingPanelIdx;
        return;
      }
      panels = panels.map((p, i) => (i === focusedIndex ? { ...p, content: item.content } : p));
    },

    /** Close all open items associated with a specific connection, optionally skipping dirty items. */
    closeItemsForConnection(connectionId: string, { skipDirty = false } = {}) {
      const toClose = openItems.filter((item) => {
        const c = item.content;
        if (!('connectionId' in c) || c.connectionId !== connectionId) return false;
        if (skipDirty) {
          const key = dirtyKeyForContent(c);
          if (key && dirtyItemKeys.has(key)) return false;
        }
        return true;
      });
      for (const item of toClose) {
        if (item.content.kind === 'query_editor' && item.content.editorId) {
          queryEditorCache.delete(item.content.editorId);
        }
        openItems = openItems.filter((i) => i.id !== item.id);
        panels = panels.map((p) =>
          sameContent(p.content, item.content) ? { ...p, content: { kind: 'empty' } } : p,
        );
      }
      focusedIndex = Math.min(focusedIndex, panels.length - 1);
    },

    /** Close all open items associated with a specific table. */
    closeItemsForTable(connectionId: string, database: string, table: string) {
      const toClose = openItems.filter((item) => {
        const c = item.content;
        if (c.kind === 'table_browser')
          return c.connectionId === connectionId && c.database === database && c.table === table;
        if (c.kind === 'table_structure')
          return c.connectionId === connectionId && c.database === database && c.table === table;
        if (c.kind === 'ddl_viewer')
          return (
            c.connectionId === connectionId && c.database === database && c.objectName === table
          );
        return false;
      });
      for (const item of toClose) {
        openItems = openItems.filter((i) => i.id !== item.id);
        panels = panels.map((p) =>
          sameContent(p.content, item.content) ? { ...p, content: { kind: 'empty' } } : p,
        );
      }
      focusedIndex = Math.min(focusedIndex, panels.length - 1);
    },

    /** Close all open items associated with a specific database. */
    closeItemsForDatabase(connectionId: string, database: string) {
      const toClose = openItems.filter((item) => {
        const c = item.content;
        if (c.kind === 'table_browser')
          return c.connectionId === connectionId && c.database === database;
        if (c.kind === 'table_structure')
          return c.connectionId === connectionId && c.database === database;
        if (c.kind === 'ddl_viewer')
          return c.connectionId === connectionId && c.database === database;
        if (c.kind === 'erd') return c.connectionId === connectionId && c.database === database;
        return false;
      });
      for (const item of toClose) {
        openItems = openItems.filter((i) => i.id !== item.id);
        panels = panels.map((p) =>
          sameContent(p.content, item.content) ? { ...p, content: { kind: 'empty' } } : p,
        );
      }
      focusedIndex = Math.min(focusedIndex, panels.length - 1);
    },

    /** Remove an item from the open list and reset any panel showing it to empty. */
    closeOpenItem(itemId: string) {
      const item = openItems.find((i) => i.id === itemId);
      if (!item) return;
      if (item.content.kind === 'query_editor' && item.content.editorId) {
        queryEditorCache.delete(item.content.editorId);
      }
      openItems = openItems.filter((i) => i.id !== itemId);
      panels = panels.map((p) =>
        sameContent(p.content, item.content) ? { ...p, content: { kind: 'empty' } } : p,
      );
      // Clamp focused index in case the layout visually changes
      focusedIndex = Math.min(focusedIndex, panels.length - 1);
    },

    /** Close all open items except the one with the given id. */
    closeOtherItems(keepItemId: string) {
      const toClose = openItems.filter((i) => i.id !== keepItemId);
      for (const item of toClose) {
        if (item.content.kind === 'query_editor' && item.content.editorId) {
          queryEditorCache.delete(item.content.editorId);
        }
        openItems = openItems.filter((i) => i.id !== item.id);
        panels = panels.map((p) =>
          sameContent(p.content, item.content) ? { ...p, content: { kind: 'empty' } } : p,
        );
      }
      focusedIndex = Math.min(focusedIndex, panels.length - 1);
    },

    /**
     * Close the open item currently shown in the focused panel.
     * If other open items exist, the next one (or previous if at the end) is shown
     * in its place — matching the behaviour of clicking × in the sidebar.
     */
    closeFocusedItem() {
      const focused = panels[focusedIndex];
      if (!focused || focused.content.kind === 'empty') return;

      if (focused.content.kind === 'query_editor' && focused.content.editorId) {
        queryEditorCache.delete(focused.content.editorId);
      }
      const itemIndex = openItems.findIndex((i) => sameContent(i.content, focused.content));
      openItems = openItems.filter((_, i) => i !== itemIndex);

      const next = openItems[itemIndex] ?? openItems[itemIndex - 1] ?? null;
      const nextContent: PanelKind = next ? next.content : { kind: 'empty' };
      panels = panels.map((p, i) => (i === focusedIndex ? { ...p, content: nextContent } : p));
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

    setItemDirty(key: string, dirty: boolean) {
      untrack(() => {
        if (dirtyItemKeys.has(key) === dirty) return;
        const next = new Set(dirtyItemKeys);
        if (dirty) next.add(key);
        else next.delete(key);
        dirtyItemKeys = next;
      });
    },
    isItemDirty(key: string): boolean {
      return dirtyItemKeys.has(key);
    },

    /** Update saved query metadata on a query_editor open item and its panel. */
    updateQueryEditorMeta(
      editorId: string,
      updates: { savedQueryId?: string; savedQueryName?: string },
    ) {
      const applyUpdate = (content: PanelKind): PanelKind => {
        if (content.kind === 'query_editor' && content.editorId === editorId) {
          return { ...content, ...updates };
        }
        return content;
      };
      openItems = openItems.map((item) => ({ ...item, content: applyUpdate(item.content) }));
      panels = panels.map((p) => ({ ...p, content: applyUpdate(p.content) }));
    },

    /** Reorder open items by moving fromId to before/after toId. */
    reorderOpenItems(fromId: string, toId: string, position: 'before' | 'after') {
      if (fromId === toId) return;
      const fromIndex = openItems.findIndex((i) => i.id === fromId);
      if (fromIndex === -1) return;
      const items = [...openItems];
      const [item] = items.splice(fromIndex, 1);
      const insertAt = items.findIndex((i) => i.id === toId);
      if (insertAt === -1) return;
      items.splice(position === 'before' ? insertAt : insertAt + 1, 0, item);
      openItems = items;
    },
  };
}
