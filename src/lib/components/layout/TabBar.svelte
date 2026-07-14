<!--
  TabBar — horizontal open-items tab strip rendered at the top of each split.
  Shown when openItemsLocation === 'top'. Displays items for a single split.
-->
<script lang="ts">
  import { usePanels, dirtyKeyForContent } from '$lib/stores/panels.svelte';
  import type { OpenItem } from '$lib/stores/panels.svelte';
  import { useTabDrag } from '$lib/stores/tabDragState.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import type { PanelKind } from '$lib/types';
  import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
  import { clearTablePendingState } from '$lib/components/table/TableBrowser.svelte';
  import * as savedQueriesApi from '$lib/tauri/saved_queries';
  import { queryEditorCache } from '$lib/stores/queryEditorState';
  import { useDashboards } from '$lib/stores/dashboards.svelte';
  import { panelLabel } from '$lib/utils/panel-label';
  import PanelIcon from '$lib/components/layout/PanelIcon.svelte';
  import TabContextMenu from '$lib/components/layout/TabContextMenu.svelte';

  interface Props {
    splitId: string;
  }

  const { splitId }: Props = $props();

  const panelStore = usePanels();
  const tabDrag = useTabDrag();
  const connectionStore = useConnections();
  const dashboardsStore = useDashboards();

  function panelConnInfo(content: PanelKind): { color: string | null } | null {
    if (!('connectionId' in content)) return null;
    const conn = connectionStore.getById(content.connectionId);
    if (!conn) return null;
    return { color: conn.color };
  }
  const dashboardsById = $derived(new Map(dashboardsStore.dashboards.map((d) => [d.id, d])));

  // Items for this split only
  const items = $derived(panelStore.getSplitItems(splitId));
  const focusedItemId = $derived(panelStore.getSplitFocusedItemId(splitId));
  const isThisSplitFocused = $derived(panelStore.focusedSplitId === splitId);

  const splitActiveContent = $derived.by(() => {
    const id = panelStore.getSplitFocusedItemId(splitId);
    const splitItems = panelStore.getSplitItems(splitId);
    return splitItems.find((i) => i.id === id)?.content;
  });
  const hasFocusedConnection = $derived(
    splitActiveContent !== undefined && 'connectionId' in splitActiveContent,
  );

  // ── Same-split drag state ─────────────────────────────────────────────────

  let dragId = $state<string | null>(null);
  let isDragging = $state(false);
  let dropTarget = $state<{ id: string; position: 'before' | 'after' } | null>(null);
  let crossSplitTarget = $state<string | null>(null); // target splitId for cross-split drop
  let pointerStartX = 0;

  $effect(() => {
    if (!dragId) return;

    function onMove(e: PointerEvent) {
      if (!isDragging && Math.abs(e.clientX - pointerStartX) > 4) {
        isDragging = true;
        tabDrag.start(dragId!, splitId);
      }
      if (!isDragging) return;

      const el = document.elementFromPoint(e.clientX, e.clientY);

      // Check for cross-split drop: tab in another split's TabBar
      const tabEl = el?.closest<HTMLElement>('[data-drag-id]');
      const targetTabId = tabEl?.dataset.dragId;
      const targetSplitAttr = tabEl?.dataset.splitId;
      if (targetSplitAttr && targetSplitAttr !== splitId) {
        crossSplitTarget = targetSplitAttr;
        dropTarget = null;
        return;
      }

      // Check for cross-split drop: anywhere over another split's content area
      const leafEl = el?.closest<HTMLElement>('[data-split-leaf-id]');
      const targetLeafSplitId = leafEl?.dataset.splitLeafId;
      if (targetLeafSplitId && targetLeafSplitId !== splitId) {
        crossSplitTarget = targetLeafSplitId;
        dropTarget = null;
        return;
      }

      crossSplitTarget = null;

      if (!targetTabId || targetTabId === dragId) {
        dropTarget = null;
        return;
      }

      const rect = tabEl!.getBoundingClientRect();
      const position = e.clientX < rect.left + rect.width / 2 ? 'before' : 'after';
      dropTarget = { id: targetTabId, position };
    }

    function onUp() {
      if (isDragging) {
        // tabDrag.isDragging will be false if a drop zone already handled this drop
        if (tabDrag.isDragging) {
          if (crossSplitTarget) {
            // Move to the other split
            panelStore.moveItemToSplit(dragId!, crossSplitTarget);
          } else if (dropTarget) {
            panelStore.reorderOpenItems(dragId!, dropTarget.id, dropTarget.position, splitId);
          }
          tabDrag.end();
        }
      }
      dragId = null;
      isDragging = false;
      dropTarget = null;
      crossSplitTarget = null;
    }

    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);

    return () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };
  });

  function onPointerDown(e: PointerEvent, id: string) {
    if (e.button !== 0) return;
    if ((e.target as HTMLElement).closest('.close-btn')) return;
    pointerStartX = e.clientX;
    dragId = id;
    // Release implicit pointer capture so pointerup fires on the element actually
    // under the pointer, letting split-leaf's onpointerup handle cross-split drops.
    (e.target as HTMLElement).releasePointerCapture(e.pointerId);
  }

  let confirmCloseItemId = $state<string | null>(null);

  // ── Context menu ──────────────────────────────────────────────────────────

  let contextMenuItemId = $state<string | null>(null);
  let contextMenuTop = $state(0);
  let contextMenuLeft = $state(0);

  function onContextMenu(e: MouseEvent, item: OpenItem) {
    const hasConnection = 'connectionId' in item.content;
    const hasSavedQuery = item.content.kind === 'query_editor' && !!item.content.savedQueryId;
    const hasOtherTabs = panelStore.getSplitItems(splitId).length > 1;
    const hasOtherSplits = panelStore.splitCount > 1;
    if (!hasConnection && !hasSavedQuery && !hasOtherTabs && !hasOtherSplits) return;
    e.preventDefault();
    contextMenuItemId = item.id;
    contextMenuTop = e.clientY;
    contextMenuLeft = e.clientX;
  }

  // ── Rename ────────────────────────────────────────────────────────────────

  let renamingItemId = $state<string | null>(null);
  let renameValue = $state('');
  let renameInputEl = $state<HTMLInputElement | undefined>(undefined);

  $effect(() => {
    if (!renamingItemId) return;
    requestAnimationFrame(() => {
      renameInputEl?.focus();
      renameInputEl?.select();
    });
  });

  async function commitRename(item: OpenItem) {
    if (!renameValue.trim() || item.content.kind !== 'query_editor' || !item.content.savedQueryId) {
      renamingItemId = null;
      return;
    }
    const name = renameValue.trim();
    const currentSql = item.content.editorId
      ? (queryEditorCache.get(item.content.editorId)?.sql ?? item.content.initialSql ?? '')
      : (item.content.initialSql ?? '');
    const currentDescription = item.content.editorId
      ? (queryEditorCache.get(item.content.editorId)?.description ?? item.content.initialDescription ?? null)
      : (item.content.initialDescription ?? null);
    const currentAnnotations = item.content.editorId
      ? (queryEditorCache.get(item.content.editorId)?.annotations ?? item.content.initialAnnotations ?? null)
      : (item.content.initialAnnotations ?? null);
    try {
      const updated = await savedQueriesApi.fileUpdateSavedQuery(item.content.savedQueryId, {
        name,
        sql: currentSql,
        description: currentDescription,
        annotations: currentAnnotations,
        connectionId: item.content.connectionId,
      });
      if (updated.id !== item.content.savedQueryId && item.content.editorId) {
        panelStore.updateQueryEditorMeta(item.content.editorId, { savedQueryId: updated.id });
      }
    } catch {
      /* ignore */
    }
    if (item.content.editorId) {
      panelStore.updateQueryEditorMeta(item.content.editorId, { savedQueryName: name });
    }
    renamingItemId = null;
  }

  function itemIsDirty(item: OpenItem): boolean {
    const key = dirtyKeyForContent(item.content);
    return key ? panelStore.isItemDirty(key) : false;
  }

  function openNewQueryEditor() {
    if (!splitActiveContent) return;
    const connectionId =
      'connectionId' in splitActiveContent ? splitActiveContent.connectionId : null;
    if (connectionId) panelStore.openInSplit({ kind: 'query_editor', connectionId }, splitId);
  }
</script>

<div class="tab-bar" class:is-dragging={isDragging} class:split-focused={isThisSplitFocused}>
  <div class="tabs-scroll">
    {#each items as item (item.id)}
      {@const isFocused = item.id === focusedItemId}
      {@const connInfo = panelConnInfo(item.content)}
      {#if dropTarget?.id === item.id && dropTarget.position === 'before'}
        <div class="drop-indicator" aria-hidden="true"></div>
      {/if}
      <div
        class="tab"
        class:focused={isFocused}
        class:dragging={isDragging && dragId === item.id}
        role="tab"
        aria-selected={isFocused}
        data-drag-id={item.id}
        data-split-id={splitId}
        onclick={() => {
          panelStore.focusSplit(splitId);
          panelStore.showItem(item);
        }}
        onkeydown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            panelStore.focusSplit(splitId);
            panelStore.showItem(item);
            return;
          }
          if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
            e.preventDefault();
            const container = (e.currentTarget as HTMLElement).closest('.tabs-scroll');
            const tabs = Array.from(container?.querySelectorAll<HTMLElement>('[role="tab"]') ?? []);
            const idx = tabs.indexOf(e.currentTarget as HTMLElement);
            const next = e.key === 'ArrowRight' ? idx + 1 : idx - 1;
            tabs[next]?.focus();
            return;
          }
          if (e.key === 'Delete' || e.key === 'Backspace') {
            e.preventDefault();
            if (itemIsDirty(item)) {
              confirmCloseItemId = item.id;
            } else {
              panelStore.closeOpenItem(item.id);
            }
          }
        }}
        onpointerdown={(e) => onPointerDown(e, item.id)}
        oncontextmenu={(e) => onContextMenu(e, item)}
        ondblclick={() => {
          if (item.content.kind === 'query_editor' && item.content.savedQueryId) {
            renamingItemId = item.id;
            renameValue = item.content.savedQueryName ?? 'Query';
          }
        }}
        tabindex="0"
      >
        <span
          class="conn-dot"
          style={connInfo
            ? `background:${connInfo.color ?? 'var(--color-accent)'}`
            : 'background:transparent'}
          aria-hidden="true"
        ></span>
        <span class="tab-icon" aria-hidden="true">
          <PanelIcon content={item.content} size={13} />
        </span>
        {#if renamingItemId === item.id}
          <input
            bind:this={renameInputEl}
            bind:value={renameValue}
            class="rename-input"
            type="text"
            maxlength="120"
            autocomplete="off"
            spellcheck={false}
            onclick={(e) => e.stopPropagation()}
            onkeydown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                commitRename(item);
              }
              if (e.key === 'Escape') {
                renamingItemId = null;
              }
            }}
            onblur={() => commitRename(item)}
          />
        {:else}
          <span class="tab-label" title={panelLabel(item.content, dashboardsById)}>{panelLabel(item.content, dashboardsById)}</span>
        {/if}
        {#if itemIsDirty(item)}
          <span class="dirty-dot" title="Unsaved changes" aria-label="Has unsaved changes"></span>
        {/if}
        <button
          class="close-btn"
          aria-label="Close tab"
          onclick={(e) => {
            e.stopPropagation();
            if (itemIsDirty(item)) {
              confirmCloseItemId = item.id;
            } else {
              panelStore.closeOpenItem(item.id);
            }
          }}
        >
          <svg
            width="11"
            height="11"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
          >
            <line x1="6" y1="6" x2="18" y2="18"></line>
            <line x1="18" y1="6" x2="6" y2="18"></line>
          </svg>
        </button>
      </div>
      {#if dropTarget?.id === item.id && dropTarget.position === 'after'}
        <div class="drop-indicator" aria-hidden="true"></div>
      {/if}
    {/each}
  </div>

  {#if hasFocusedConnection}
    <button
      class="new-tab-btn"
      onclick={openNewQueryEditor}
      title="New Query Editor (⌘N)"
      aria-label="New query editor"
    >
      <svg
        width="11"
        height="11"
        viewBox="0 0 11 11"
        fill="none"
        stroke="currentColor"
        stroke-width="1"
        stroke-linecap="round"
      >
        <line x1="5.5" y1="1.5" x2="5.5" y2="9.5"></line>
        <line x1="1.5" y1="5.5" x2="9.5" y2="5.5"></line>
      </svg>
    </button>
  {/if}
</div>

{#if contextMenuItemId !== null}
  {@const contextItem = items.find((i) => i.id === contextMenuItemId)}
  {#if contextItem}
    <TabContextMenu
      item={contextItem}
      {splitId}
      x={contextMenuLeft}
      y={contextMenuTop}
      open={true}
      minWidth={200}
      onclose={() => (contextMenuItemId = null)}
      onrename={(item) => {
        renamingItemId = item.id;
        renameValue = item.content.kind === 'query_editor'
          ? (item.content.savedQueryName ?? 'Query')
          : '';
      }}
    />
  {/if}
{/if}

{#if confirmCloseItemId !== null}
  {@const itemToClose = items.find((i) => i.id === confirmCloseItemId)}
  {#if itemToClose}
    <ConfirmDialog
      title="Close tab"
      message="This tab has unsaved changes. Close anyway?"
      confirmText="Close"
      cancelText="Cancel"
      danger={true}
      onconfirm={() => {
        const key = dirtyKeyForContent(itemToClose.content);
        if (key) clearTablePendingState(key);
        panelStore.setItemDirty(key ?? '', false);
        panelStore.closeOpenItem(confirmCloseItemId!);
        confirmCloseItemId = null;
      }}
      oncancel={() => {
        confirmCloseItemId = null;
      }}
    />
  {/if}
{/if}

<style>
  .tab-bar {
    display: flex;
    align-items: center;
    height: 36px;
    flex-shrink: 0;
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    overflow: hidden;
    position: relative;
  }

  .tab-bar.is-dragging {
    cursor: grabbing;
  }

  .tabs-scroll {
    display: flex;
    align-items: center;
    flex: 1;
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: none;
    min-width: 0;
  }

  .tabs-scroll::-webkit-scrollbar {
    display: none;
  }

  .tab {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 0 10px;
    height: 36px;
    flex-shrink: 0;
    max-width: 200px;
    font-size: 12.5px;
    font-family: var(--font-family-ui);
    color: var(--color-text-secondary);
    background: transparent;
    border: none;
    border-right: 1px solid var(--color-border);
    cursor: pointer;
    -webkit-user-select: none;
    user-select: none;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
    position: relative;
  }

  .tab:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .tab.focused {
    background: var(--color-bg-primary);
    color: var(--color-accent);
    font-weight: 600;
    box-shadow: inset 0 -2px 0 var(--color-accent);
  }

  .tab.dragging {
    opacity: 0.4;
  }

  .drop-indicator {
    width: 2px;
    height: 24px;
    background: var(--color-accent);
    border-radius: var(--radius-xs);
    flex-shrink: 0;
    pointer-events: none;
  }

  .conn-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .tab-icon {
    flex-shrink: 0;
    display: flex;
    align-items: center;
  }

  .tab-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    min-width: 0;
  }

  .dirty-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--color-warning, #f59e0b);
    flex-shrink: 0;
  }

  .close-btn {
    flex-shrink: 0;
    width: 16px;
    height: 16px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-muted);
    border-radius: var(--radius-sm);
    opacity: 0;
    transition:
      opacity var(--transition-fast),
      background var(--transition-fast);
  }

  .tab:hover .close-btn,
  .tab.focused .close-btn {
    opacity: 1;
  }

  .close-btn:hover {
    background: var(--color-bg-active);
    color: var(--color-text-primary);
  }

  .rename-input {
    flex: 1;
    min-width: 60px;
    height: 20px;
    padding: 0 var(--spacing-1);
    background: var(--color-bg-input, var(--color-bg-secondary));
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-sm);
    color: var(--color-text-primary);
    font-size: 12.5px;
    font-family: var(--font-family-ui);
    outline: none;
  }

  .new-tab-btn {
    flex-shrink: 0;
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border-left: 1px solid var(--color-border);
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .new-tab-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }
</style>
