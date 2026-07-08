/**
 * Panel layout store.
 * Manages a tree-based split layout. Each "split" (leaf node) has its own
 * list of open items and a focused item id.
 *
 * The public API maintains backward-compatible getters (panels, focusedIndex,
 * focusedPanel, openItems, panelCount) so existing callers don't break.
 */
import { untrack } from 'svelte';
import type { PanelState, PanelKind, SplitNode, SplitChild } from '$lib/types';
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

/**
 * Returns true when two PanelKind values represent the same logical item.
 * query_editor instances are always treated as distinct unless both have the
 * same editorId.
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
    case 'release_notes':
      return b.kind === 'release_notes' && a.version === b.version;
    case 'empty':
      return true;
  }
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

// ── Tree helpers ──────────────────────────────────────────────────────────────

/** DFS left-to-right traversal: returns all leaf splitIds in order. */
function getAllLeafIds(node: SplitNode): string[] {
  if (node.type === 'leaf') return [node.splitId];
  const result: string[] = [];
  for (const child of node.children) {
    result.push(...getAllLeafIds(child.node));
  }
  return result;
}

function countLeaves(node: SplitNode): number {
  return getAllLeafIds(node).length;
}

/** Replace a leaf node (by splitId) with a new node. */
function replaceNode(root: SplitNode, targetId: string, newNode: SplitNode): SplitNode {
  if (root.type === 'leaf') {
    return root.splitId === targetId ? newNode : root;
  }
  return {
    ...root,
    children: root.children.map((c) => ({
      ...c,
      node: replaceNode(c.node, targetId, newNode),
    })),
  };
}

/**
 * Remove a leaf from the tree.
 * - Returns null if it was the last leaf.
 * - Collapses single-child containers.
 * - Redistributes sizes equally among siblings after removal.
 */
function removeLeaf(root: SplitNode, splitId: string): SplitNode | null {
  if (root.type === 'leaf') {
    return root.splitId === splitId ? null : root;
  }

  // Remove the target from children
  const newChildren: SplitChild[] = [];
  for (const child of root.children) {
    const updated = removeLeaf(child.node, splitId);
    if (updated !== null) {
      newChildren.push({ ...child, node: updated });
    }
  }

  // If all children were removed, this container should collapse
  if (newChildren.length === 0) return null;

  // Collapse single-child container
  if (newChildren.length === 1) return newChildren[0].node;

  // Redistribute sizes equally
  const equalSize = 100 / newChildren.length;
  const redistributed = newChildren.map((c) => ({ ...c, size: equalSize }));

  return { ...root, children: redistributed };
}

/**
 * If the target leaf is a direct child of a container whose direction matches,
 * insert newNode as a sibling (before or after) and split the target's size in half.
 * Returns the updated tree root, or null if no suitable parent was found.
 */
function tryInsertSiblingInParent(
  root: SplitNode,
  targetSplitId: string,
  newNode: SplitNode,
  insertBefore: boolean,
  direction: 'horizontal' | 'vertical',
): SplitNode | null {
  if (root.type === 'leaf') return null;

  if (root.direction === direction) {
    const idx = root.children.findIndex(
      (c) => c.node.type === 'leaf' && c.node.splitId === targetSplitId,
    );
    if (idx !== -1) {
      const half = root.children[idx].size / 2;
      const newChildren = root.children.map((c, i) => (i === idx ? { ...c, size: half } : c));
      newChildren.splice(insertBefore ? idx : idx + 1, 0, { node: newNode, size: half });
      return { ...root, children: newChildren };
    }
  }

  for (let i = 0; i < root.children.length; i++) {
    const result = tryInsertSiblingInParent(
      root.children[i].node,
      targetSplitId,
      newNode,
      insertBefore,
      direction,
    );
    if (result !== null) {
      const newChildren = [...root.children];
      newChildren[i] = { ...newChildren[i], node: result };
      return { ...root, children: newChildren };
    }
  }

  return null;
}

/**
 * Returns the maximum number of direct children found in any container of the
 * given direction anywhere in the tree. Used to check per-direction split limits.
 */
function maxContainerChildren(node: SplitNode, direction: 'horizontal' | 'vertical'): number {
  if (node.type === 'leaf') return 0;
  let best = node.direction === direction ? node.children.length : 0;
  for (const child of node.children) {
    best = Math.max(best, maxContainerChildren(child.node, direction));
  }
  return best;
}

/** Mutate the sizes of children in the container with the given containerId in-place. */
function mutateSizes(root: SplitNode, containerId: string, newSizes: number[]): void {
  if (root.type === 'leaf') return;
  if (root.containerId === containerId) {
    root.children.forEach((c, i) => {
      if (newSizes[i] !== undefined) c.size = newSizes[i];
    });
    return;
  }
  for (const c of root.children) {
    mutateSizes(c.node, containerId, newSizes);
  }
}

// ── Per-split state ───────────────────────────────────────────────────────────

interface SplitState {
  id: string;
  openItems: OpenItem[];
  focusedItemId: string | null;
}

// ── Module-level reactive state ───────────────────────────────────────────────

const initialSplitId = createId();

let layout = $state<SplitNode>({ type: 'leaf', splitId: initialSplitId });
let splitStates = $state<Map<string, SplitState>>(
  new Map([[initialSplitId, { id: initialSplitId, openItems: [], focusedItemId: null }]]),
);
let focusedSplitIdState = $state<string>(initialSplitId);
let dirtyItemKeys = $state<Set<string>>(new Set());

// ── Internal helpers ──────────────────────────────────────────────────────────

function getSplitState(splitId: string): SplitState {
  let s = splitStates.get(splitId);
  if (!s) {
    s = { id: splitId, openItems: [], focusedItemId: null };
    const next = new Map(splitStates);
    next.set(splitId, s);
    splitStates = next;
  }
  return s;
}

function setSplitState(splitId: string, state: SplitState) {
  const next = new Map(splitStates);
  next.set(splitId, state);
  splitStates = next;
}

function deleteSplitState(splitId: string) {
  const next = new Map(splitStates);
  next.delete(splitId);
  splitStates = next;
}

/** Find which splitId contains an item by itemId. */
function findSplitForItem(itemId: string): string | null {
  for (const [splitId, state] of splitStates) {
    if (state.openItems.some((i) => i.id === itemId)) return splitId;
  }
  return null;
}

/** Find which splitId contains an item matching the given content (by sameContent). */
function findSplitForContent(content: PanelKind): string | null {
  for (const [splitId, state] of splitStates) {
    if (state.openItems.some((i) => sameContent(i.content, content))) return splitId;
  }
  return null;
}

/**
 * Auto-close a split if it's empty and not the last split.
 * Returns true if the split was closed.
 */
function autoCloseSplitIfEmpty(splitId: string): boolean {
  const leafIds = getAllLeafIds(layout);
  if (leafIds.length <= 1) return false;
  const state = splitStates.get(splitId);
  if (!state || state.openItems.length > 0) return false;

  const newLayout = removeLeaf(layout, splitId);
  if (!newLayout) return false;
  layout = newLayout;
  deleteSplitState(splitId);

  // Refocus first remaining leaf if needed
  const remaining = getAllLeafIds(layout);
  if (!remaining.includes(focusedSplitIdState)) {
    focusedSplitIdState = remaining[0] ?? '';
  }
  return true;
}

/**
 * Move an item between splits (module-level so addColumnRight/addRowBottom can call it).
 * Returns true if the move was performed.
 */
function moveItemBetweenSplits(itemId: string, targetSplitId: string): boolean {
  const sourceSplitId = findSplitForItem(itemId);
  if (!sourceSplitId || sourceSplitId === targetSplitId) return false;

  const sourceState = getSplitState(sourceSplitId);
  const item = sourceState.openItems.find((i) => i.id === itemId);
  if (!item) return false;

  // Remove from source
  const newSourceItems = sourceState.openItems.filter((i) => i.id !== itemId);
  let newSourceFocusedId = sourceState.focusedItemId;
  if (newSourceFocusedId === itemId) {
    const idx = sourceState.openItems.findIndex((i) => i.id === itemId);
    const next = sourceState.openItems[idx + 1] ?? sourceState.openItems[idx - 1] ?? null;
    newSourceFocusedId = next?.id ?? null;
  }
  setSplitState(sourceSplitId, {
    ...sourceState,
    openItems: newSourceItems,
    focusedItemId: newSourceFocusedId,
  });

  // Add to target
  const targetState = getSplitState(targetSplitId);
  setSplitState(targetSplitId, {
    ...targetState,
    openItems: [...targetState.openItems, item],
    focusedItemId: item.id,
  });

  focusedSplitIdState = targetSplitId;

  // Auto-close source if empty
  if (newSourceItems.length === 0) {
    autoCloseSplitIfEmpty(sourceSplitId);
  }

  return true;
}

/** Clean up query editor cache for all items in a split before closing it. */
function cleanupSplitItems(splitId: string) {
  const state = splitStates.get(splitId);
  if (!state) return;
  for (const item of state.openItems) {
    if (item.content.kind === 'query_editor' && item.content.editorId) {
      queryEditorCache.delete(item.content.editorId);
    }
  }
}

// ── Public interface ──────────────────────────────────────────────────────────

export function usePanels() {
  return {
    // ── Backward-compat getters ─────────────────────────────────────────────

    /** Array of PanelState for each leaf in DFS order (backward compat). */
    get panels(): PanelState[] {
      return getAllLeafIds(layout).map((splitId) => {
        const state = splitStates.get(splitId);
        const focusedId = state?.focusedItemId ?? null;
        const item = state?.openItems.find((i) => i.id === focusedId);
        const content: PanelKind = item?.content ?? { kind: 'empty' };
        return { id: splitId, content };
      });
    },

    get focusedIndex(): number {
      const ids = getAllLeafIds(layout);
      const idx = ids.indexOf(focusedSplitIdState);
      return idx >= 0 ? idx : 0;
    },

    get focusedPanel(): PanelState {
      const splitId = focusedSplitIdState;
      const state = splitStates.get(splitId);
      const focusedId = state?.focusedItemId ?? null;
      const item = state?.openItems.find((i) => i.id === focusedId);
      const content: PanelKind = item?.content ?? { kind: 'empty' };
      return { id: splitId, content };
    },

    /** All items across all splits, flattened (for sidebar mode). */
    get openItems(): OpenItem[] {
      const ids = getAllLeafIds(layout);
      const result: OpenItem[] = [];
      for (const splitId of ids) {
        const state = splitStates.get(splitId);
        if (state) result.push(...state.openItems);
      }
      return result;
    },

    get panelCount(): number {
      return getAllLeafIds(layout).length;
    },

    // ── New split getters ───────────────────────────────────────────────────

    get layout(): SplitNode {
      return layout;
    },

    get focusedSplitId(): string {
      return focusedSplitIdState;
    },

    get splitCount(): number {
      return getAllLeafIds(layout).length;
    },

    get maxHorizontalColumns(): number {
      return maxContainerChildren(layout, 'horizontal');
    },

    get maxVerticalRows(): number {
      return maxContainerChildren(layout, 'vertical');
    },

    getSplitItems(splitId: string): OpenItem[] {
      return splitStates.get(splitId)?.openItems ?? [];
    },

    getSplitFocusedItemId(splitId: string): string | null {
      return splitStates.get(splitId)?.focusedItemId ?? null;
    },

    getSplitLabel(splitId: string): string {
      const ids = getAllLeafIds(layout);
      const idx = ids.indexOf(splitId);
      return idx >= 0 ? `Split ${idx + 1}` : 'Split';
    },

    /** Returns all leaf splitIds in DFS order. Public for TabBar/SidebarTopHalf. */
    getAllLeafIds(): string[] {
      return getAllLeafIds(layout);
    },

    // ── Existing methods ────────────────────────────────────────────────────

    /**
     * Open content in the currently focused split.
     * If already shown in some split, focus that split instead.
     */
    openInFocused(content: PanelKind) {
      if (content.kind === 'query_editor' && !content.editorId) {
        content = { ...content, editorId: createId() };
      }
      if (content.kind === 'table_browser' && content.initialFilter) {
        clearTableFilterCache(`${content.connectionId}:${content.database}:${content.table}`);
      }

      if (content.kind !== 'empty') {
        // If already visible in some split, focus that split and switch to the tab
        const existingSplitId = findSplitForContent(content);
        if (existingSplitId) {
          const state = getSplitState(existingSplitId);
          const itemIdx = state.openItems.findIndex((i) => sameContent(i.content, content));
          if (itemIdx !== -1) {
            const newItems = [...state.openItems];
            // Update the content so initialFilter propagates
            if (content.kind === 'table_browser' && content.initialFilter) {
              newItems[itemIdx] = { ...newItems[itemIdx], content };
            }
            setSplitState(existingSplitId, {
              ...state,
              openItems: newItems,
              focusedItemId: newItems[itemIdx].id,
            });
          }
          focusedSplitIdState = existingSplitId;
          return;
        }

        // Add to focused split's open items if not already tracked there
        const focusedState = getSplitState(focusedSplitIdState);
        if (!focusedState.openItems.find((item) => sameContent(item.content, content))) {
          const newItem: OpenItem = { id: createId(), content };
          setSplitState(focusedSplitIdState, {
            ...focusedState,
            openItems: [...focusedState.openItems, newItem],
            focusedItemId: newItem.id,
          });
        } else {
          // Already in this split — just focus it
          const existing = focusedState.openItems.find((i) => sameContent(i.content, content));
          if (existing) {
            setSplitState(focusedSplitIdState, {
              ...focusedState,
              focusedItemId: existing.id,
            });
          }
        }
      }
    },

    /**
     * Always open a new copy of content in the focused split, regardless of whether
     * the same content is already open elsewhere. Used for "Open Copy" actions.
     */
    openCopyInFocused(content: PanelKind) {
      if (content.kind === 'empty') return;
      if (content.kind === 'query_editor') {
        content = { ...content, editorId: createId() };
      }
      const focusedState = getSplitState(focusedSplitIdState);
      const newItem: OpenItem = { id: createId(), content };
      setSplitState(focusedSplitIdState, {
        ...focusedState,
        openItems: [...focusedState.openItems, newItem],
        focusedItemId: newItem.id,
      });
    },

    /** Like openInFocused but targets a specific split. */
    openInSplit(content: PanelKind, splitId: string) {
      if (content.kind === 'query_editor' && !content.editorId) {
        content = { ...content, editorId: createId() };
      }
      if (content.kind === 'table_browser' && content.initialFilter) {
        clearTableFilterCache(`${content.connectionId}:${content.database}:${content.table}`);
      }

      if (content.kind !== 'empty') {
        const allowDuplicate =
          (content.kind === 'table_browser' && !content.initialFilter) ||
          content.kind === 'table_structure';

        if (!allowDuplicate) {
          const existingSplitId = findSplitForContent(content);
          if (existingSplitId) {
            focusedSplitIdState = existingSplitId;
            return;
          }
        }

        const state = getSplitState(splitId);
        if (allowDuplicate || !state.openItems.find((item) => sameContent(item.content, content))) {
          const newItem: OpenItem = { id: createId(), content };
          setSplitState(splitId, {
            ...state,
            openItems: [...state.openItems, newItem],
            focusedItemId: newItem.id,
          });
        }
      }
      focusedSplitIdState = splitId;
    },

    /**
     * Replace the focused split's active tab in-place (for view mode switching).
     */
    replaceInFocused(content: PanelKind) {
      const state = getSplitState(focusedSplitIdState);
      const currentItem = state.openItems.find((i) => i.id === state.focusedItemId);
      if (currentItem) {
        const itemIndex = state.openItems.indexOf(currentItem);
        const newItems = [...state.openItems];
        newItems[itemIndex] = { ...currentItem, content };
        setSplitState(focusedSplitIdState, { ...state, openItems: newItems });
      } else {
        // No focused item — add it
        const newItem: OpenItem = { id: createId(), content };
        setSplitState(focusedSplitIdState, {
          ...state,
          openItems: [...state.openItems, newItem],
          focusedItemId: newItem.id,
        });
      }
    },

    /** Find which split contains this item and focus it; set that split's focusedItemId. */
    showItem(item: OpenItem) {
      const splitId = findSplitForItem(item.id);
      if (splitId) {
        const state = getSplitState(splitId);
        setSplitState(splitId, { ...state, focusedItemId: item.id });
        focusedSplitIdState = splitId;
      }
    },

    /** Remove item from whichever split contains it. Auto-closes empty splits (except last). */
    closeOpenItem(itemId: string) {
      const splitId = findSplitForItem(itemId);
      if (!splitId) return;

      const state = getSplitState(splitId);
      const item = state.openItems.find((i) => i.id === itemId);
      if (!item) return;

      if (item.content.kind === 'query_editor' && item.content.editorId) {
        queryEditorCache.delete(item.content.editorId);
      }

      const newItems = state.openItems.filter((i) => i.id !== itemId);

      // Find a new focused item if this was focused
      let newFocusedId = state.focusedItemId;
      if (newFocusedId === itemId) {
        const idx = state.openItems.findIndex((i) => i.id === itemId);
        const next = state.openItems[idx + 1] ?? state.openItems[idx - 1] ?? null;
        newFocusedId = next?.id ?? null;
      }

      setSplitState(splitId, { ...state, openItems: newItems, focusedItemId: newFocusedId });

      // Auto-close empty split (but not if it's the last one)
      if (newItems.length === 0) {
        autoCloseSplitIfEmpty(splitId);
      }
    },

    /** Close all items in the same split as keepItemId, except keepItemId itself. */
    closeOtherItems(keepItemId: string) {
      const splitId = findSplitForItem(keepItemId);
      if (!splitId) return;

      const state = getSplitState(splitId);
      const toClose = state.openItems.filter((i) => i.id !== keepItemId);
      for (const item of toClose) {
        if (item.content.kind === 'query_editor' && item.content.editorId) {
          queryEditorCache.delete(item.content.editorId);
        }
      }

      const keepItem = state.openItems.find((i) => i.id === keepItemId);
      setSplitState(splitId, {
        ...state,
        openItems: keepItem ? [keepItem] : [],
        focusedItemId: keepItem?.id ?? null,
      });
    },

    /** Close all open items associated with a specific connection. */
    closeItemsForConnection(connectionId: string, { skipDirty = false } = {}) {
      for (const [splitId, state] of splitStates) {
        const toClose = state.openItems.filter((item) => {
          const c = item.content;
          if (!('connectionId' in c) || c.connectionId !== connectionId) return false;
          if (skipDirty) {
            const key = dirtyKeyForContent(c);
            if (key && dirtyItemKeys.has(key)) return false;
          }
          return true;
        });

        if (toClose.length === 0) continue;

        for (const item of toClose) {
          if (item.content.kind === 'query_editor' && item.content.editorId) {
            queryEditorCache.delete(item.content.editorId);
          }
        }

        const closeIds = new Set(toClose.map((i) => i.id));
        const newItems = state.openItems.filter((i) => !closeIds.has(i.id));
        let newFocusedId = state.focusedItemId;
        if (newFocusedId && closeIds.has(newFocusedId)) {
          newFocusedId = newItems[0]?.id ?? null;
        }
        setSplitState(splitId, { ...state, openItems: newItems, focusedItemId: newFocusedId });
      }

      // Auto-close empty splits
      for (const splitId of [...splitStates.keys()]) {
        autoCloseSplitIfEmpty(splitId);
      }
    },

    /** Close all open items associated with a specific table. */
    closeItemsForTable(connectionId: string, database: string, table: string) {
      for (const [splitId, state] of splitStates) {
        const toClose = state.openItems.filter((item) => {
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

        if (toClose.length === 0) continue;

        const closeIds = new Set(toClose.map((i) => i.id));
        const newItems = state.openItems.filter((i) => !closeIds.has(i.id));
        let newFocusedId = state.focusedItemId;
        if (newFocusedId && closeIds.has(newFocusedId)) {
          newFocusedId = newItems[0]?.id ?? null;
        }
        setSplitState(splitId, { ...state, openItems: newItems, focusedItemId: newFocusedId });
      }

      for (const splitId of [...splitStates.keys()]) {
        autoCloseSplitIfEmpty(splitId);
      }
    },

    /** Close all open items associated with a specific database. */
    closeItemsForDatabase(connectionId: string, database: string) {
      for (const [splitId, state] of splitStates) {
        const toClose = state.openItems.filter((item) => {
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

        if (toClose.length === 0) continue;

        const closeIds = new Set(toClose.map((i) => i.id));
        const newItems = state.openItems.filter((i) => !closeIds.has(i.id));
        let newFocusedId = state.focusedItemId;
        if (newFocusedId && closeIds.has(newFocusedId)) {
          newFocusedId = newItems[0]?.id ?? null;
        }
        setSplitState(splitId, { ...state, openItems: newItems, focusedItemId: newFocusedId });
      }

      for (const splitId of [...splitStates.keys()]) {
        autoCloseSplitIfEmpty(splitId);
      }
    },

    /** Close the active item in the focused split. */
    closeFocusedItem() {
      const state = getSplitState(focusedSplitIdState);
      if (!state.focusedItemId) return;

      const item = state.openItems.find((i) => i.id === state.focusedItemId);
      if (!item || item.content.kind === 'empty') return;

      if (item.content.kind === 'query_editor' && item.content.editorId) {
        queryEditorCache.delete(item.content.editorId);
      }

      const itemIndex = state.openItems.findIndex((i) => i.id === state.focusedItemId);
      const newItems = state.openItems.filter((_, i) => i !== itemIndex);
      const next = newItems[itemIndex] ?? newItems[itemIndex - 1] ?? null;

      setSplitState(focusedSplitIdState, {
        ...state,
        openItems: newItems,
        focusedItemId: next?.id ?? null,
      });

      if (newItems.length === 0) {
        autoCloseSplitIfEmpty(focusedSplitIdState);
      }
    },

    /** Reorder open items within a specific split (or the split containing fromId). */
    reorderOpenItems(fromId: string, toId: string, position: 'before' | 'after', splitId?: string) {
      if (fromId === toId) return;
      const targetSplitId = splitId ?? findSplitForItem(fromId);
      if (!targetSplitId) return;

      const state = getSplitState(targetSplitId);
      const fromIndex = state.openItems.findIndex((i) => i.id === fromId);
      if (fromIndex === -1) return;

      const items = [...state.openItems];
      const [item] = items.splice(fromIndex, 1);
      const insertAt = items.findIndex((i) => i.id === toId);
      if (insertAt === -1) return;
      items.splice(position === 'before' ? insertAt : insertAt + 1, 0, item);
      setSplitState(targetSplitId, { ...state, openItems: items });
    },

    /** Focus split by DFS index. */
    focus(index: number) {
      const ids = getAllLeafIds(layout);
      if (index >= 0 && index < ids.length) {
        focusedSplitIdState = ids[index];
      }
    },

    /** Focus next split (wraps around). */
    focusNext() {
      const ids = getAllLeafIds(layout);
      if (ids.length === 0) return;
      const current = ids.indexOf(focusedSplitIdState);
      focusedSplitIdState = ids[(current + 1) % ids.length];
    },

    /** Focus previous split (wraps around). */
    focusPrev() {
      const ids = getAllLeafIds(layout);
      if (ids.length === 0) return;
      const current = ids.indexOf(focusedSplitIdState);
      focusedSplitIdState = ids[(current - 1 + ids.length) % ids.length];
    },

    /** Focus a specific split by id. */
    focusSplit(splitId: string) {
      focusedSplitIdState = splitId;
    },

    // ── Split manipulation ──────────────────────────────────────────────────

    /**
     * Split a specific leaf into a container with 2 children.
     * right/down: new split appears after the target.
     * left/up:   new split appears before the target.
     * Returns the new split's id, or null if the limit is reached.
     */
    splitAt(
      targetSplitId: string,
      direction: 'right' | 'down' | 'left' | 'up',
      maxH: number,
      maxV: number,
    ): string | null {
      if (countLeaves(layout) >= maxH * maxV) return null;

      const newSplitId = createId();
      setSplitState(newSplitId, { id: newSplitId, openItems: [], focusedItemId: null });

      const containerId = createId();
      const containerDirection =
        direction === 'right' || direction === 'left' ? 'horizontal' : 'vertical';
      const insertBefore = direction === 'left' || direction === 'up';

      const newLeaf: SplitNode = { type: 'leaf', splitId: newSplitId };

      // Prefer a flat insert: if the target's parent container already runs in the
      // same direction, add the new leaf there (halving the target's size) so that
      // all siblings stay in one container and every divider controls only two panels.
      const flatLayout = tryInsertSiblingInParent(
        layout,
        targetSplitId,
        newLeaf,
        insertBefore,
        containerDirection,
      );
      if (flatLayout !== null) {
        layout = flatLayout;
      } else {
        const targetLeaf: SplitNode = { type: 'leaf', splitId: targetSplitId };
        const newContainer: SplitNode = {
          type: 'container',
          containerId,
          direction: containerDirection,
          children: insertBefore
            ? [
                { node: newLeaf, size: 50 },
                { node: targetLeaf, size: 50 },
              ]
            : [
                { node: targetLeaf, size: 50 },
                { node: newLeaf, size: 50 },
              ],
        };
        layout = replaceNode(layout, targetSplitId, newContainer);
      }

      focusedSplitIdState = newSplitId;
      return newSplitId;
    },

    /**
     * Split the focused leaf into a container with 2 children.
     * The focused leaf stays as child 0; a new empty split is child 1.
     */
    splitFocused(direction: 'right' | 'down', maxH: number, maxV: number) {
      if (countLeaves(layout) >= maxH * maxV) return;

      const newSplitId = createId();
      setSplitState(newSplitId, { id: newSplitId, openItems: [], focusedItemId: null });

      const containerId = createId();
      const containerDirection = direction === 'right' ? 'horizontal' : 'vertical';
      const newLeaf: SplitNode = { type: 'leaf', splitId: newSplitId };

      const flatLayout = tryInsertSiblingInParent(
        layout,
        focusedSplitIdState,
        newLeaf,
        false,
        containerDirection,
      );
      if (flatLayout !== null) {
        layout = flatLayout;
      } else {
        const currentLeaf: SplitNode = { type: 'leaf', splitId: focusedSplitIdState };
        const newContainer: SplitNode = {
          type: 'container',
          containerId,
          direction: containerDirection,
          children: [
            { node: currentLeaf, size: 50 },
            { node: newLeaf, size: 50 },
          ],
        };
        layout = replaceNode(layout, focusedSplitIdState, newContainer);
      }

      focusedSplitIdState = newSplitId;
    },

    /**
     * Add a new column to the right by:
     * 1. If layout root is a horizontal container: add new leaf child.
     * 2. Else: wrap root in a horizontal container.
     * Then move the dragged item from its source split.
     */
    addColumnRight(draggedItemId: string, sourceSplitId: string, maxH: number, maxV: number) {
      if (countLeaves(layout) >= maxH * maxV) return;

      const newSplitId = createId();
      setSplitState(newSplitId, { id: newSplitId, openItems: [], focusedItemId: null });

      const newLeaf: SplitNode = { type: 'leaf', splitId: newSplitId };

      if (layout.type === 'container' && layout.direction === 'horizontal') {
        // Add as a new child
        const childCount = layout.children.length + 1;
        const equalSize = 100 / childCount;
        layout = {
          ...layout,
          children: [
            ...layout.children.map((c) => ({ ...c, size: equalSize })),
            { node: newLeaf, size: equalSize },
          ],
        };
      } else {
        const containerId = createId();
        layout = {
          type: 'container',
          containerId,
          direction: 'horizontal',
          children: [
            { node: layout, size: 50 },
            { node: newLeaf, size: 50 },
          ],
        };
      }

      focusedSplitIdState = newSplitId;

      // Move dragged item
      moveItemBetweenSplits(draggedItemId, newSplitId);
    },

    /**
     * Add a new row at the bottom by:
     * 1. If layout root is a vertical container: add new leaf child.
     * 2. Else: wrap root in a vertical container.
     */
    addRowBottom(draggedItemId: string, sourceSplitId: string, maxH: number, maxV: number) {
      if (countLeaves(layout) >= maxH * maxV) return;

      const newSplitId = createId();
      setSplitState(newSplitId, { id: newSplitId, openItems: [], focusedItemId: null });

      const newLeaf: SplitNode = { type: 'leaf', splitId: newSplitId };

      if (layout.type === 'container' && layout.direction === 'vertical') {
        const childCount = layout.children.length + 1;
        const equalSize = 100 / childCount;
        layout = {
          ...layout,
          children: [
            ...layout.children.map((c) => ({ ...c, size: equalSize })),
            { node: newLeaf, size: equalSize },
          ],
        };
      } else {
        const containerId = createId();
        layout = {
          type: 'container',
          containerId,
          direction: 'vertical',
          children: [
            { node: layout, size: 50 },
            { node: newLeaf, size: 50 },
          ],
        };
      }

      focusedSplitIdState = newSplitId;

      // Move dragged item
      moveItemBetweenSplits(draggedItemId, newSplitId);
    },

    /** Close a split. Items in it are lost. */
    closeSplit(splitId: string) {
      const leafIds = getAllLeafIds(layout);

      // Last split: just reset to empty
      if (leafIds.length <= 1) {
        cleanupSplitItems(splitId);
        const newId = createId();
        layout = { type: 'leaf', splitId: newId };
        splitStates = new Map([[newId, { id: newId, openItems: [], focusedItemId: null }]]);
        focusedSplitIdState = newId;
        return;
      }

      cleanupSplitItems(splitId);

      const newLayout = removeLeaf(layout, splitId);
      if (!newLayout) return;

      layout = newLayout;
      deleteSplitState(splitId);

      const remaining = getAllLeafIds(layout);
      if (!remaining.includes(focusedSplitIdState)) {
        focusedSplitIdState = remaining[0] ?? '';
      }
    },

    /**
     * Move an item from its current split to a target split.
     * Auto-closes the source split if it becomes empty.
     */
    moveItemToSplit(itemId: string, targetSplitId: string) {
      moveItemBetweenSplits(itemId, targetSplitId);
    },

    /**
     * Open a copy of content in targetSplitId.
     * For query_editor: always create a new editor.
     * For others: if already in targetSplitId, just focus it.
     */
    copyItemToSplit(content: PanelKind, targetSplitId: string) {
      if (content.kind === 'query_editor') {
        const newContent = { ...content, editorId: createId() };
        const state = getSplitState(targetSplitId);
        const newItem: OpenItem = { id: createId(), content: newContent };
        setSplitState(targetSplitId, {
          ...state,
          openItems: [...state.openItems, newItem],
          focusedItemId: newItem.id,
        });
        focusedSplitIdState = targetSplitId;
        return;
      }

      // For non-query content: if already in targetSplitId, just focus it
      const targetState = getSplitState(targetSplitId);
      const existing = targetState.openItems.find((i) => sameContent(i.content, content));
      if (existing) {
        setSplitState(targetSplitId, { ...targetState, focusedItemId: existing.id });
        focusedSplitIdState = targetSplitId;
        return;
      }

      const newItem: OpenItem = { id: createId(), content };
      setSplitState(targetSplitId, {
        ...targetState,
        openItems: [...targetState.openItems, newItem],
        focusedItemId: newItem.id,
      });
      focusedSplitIdState = targetSplitId;
    },

    /** Update sizes of a container's children. */
    updateContainerSizes(containerId: string, newSizes: number[]) {
      mutateSizes(layout, containerId, newSizes);
    },

    // ── Dirty tracking ──────────────────────────────────────────────────────

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

    /** Update saved query metadata on a query_editor open item across all splits. */
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

      for (const [splitId, state] of splitStates) {
        const changed = state.openItems.some(
          (i) => i.content.kind === 'query_editor' && i.content.editorId === editorId,
        );
        if (changed) {
          setSplitState(splitId, {
            ...state,
            openItems: state.openItems.map((item) => ({
              ...item,
              content: applyUpdate(item.content),
            })),
          });
        }
      }
    },
  };
}
