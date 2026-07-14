<!--
  SidebarTopHalf — open panels list.
  Single split: flat list of all items (existing behaviour).
  Multiple splits: group headers per split with items beneath.
-->
<script lang="ts">
  import { usePanels, sameContent, dirtyKeyForContent } from '$lib/stores/panels.svelte';
  import type { OpenItem } from '$lib/stores/panels.svelte';
  import { useTabDrag } from '$lib/stores/tabDragState.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import type { PanelKind } from '$lib/types';
  import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
  import { clearTablePendingState } from '$lib/components/table/TableBrowser.svelte';
  import * as savedQueriesApi from '$lib/tauri/saved_queries';
  import { queryEditorCache } from '$lib/stores/queryEditorState';
  import { useDashboards } from '$lib/stores/dashboards.svelte';
  import { panelLabel } from '$lib/utils/panel-label';
  import PanelIcon from '$lib/components/layout/PanelIcon.svelte';
  import TabContextMenu from '$lib/components/layout/TabContextMenu.svelte';

  const panelStore = usePanels();
  const tabDrag = useTabDrag();
  const connectionStore = useConnections();
  const settingsStore = useSettings();
  const dashboardsStore = useDashboards();
  const dashboardsById = $derived(new Map(dashboardsStore.dashboards.map((d) => [d.id, d])));

  function panelConnInfo(content: PanelKind): { color: string | null; shortName: string } | null {
    if (!('connectionId' in content)) return null;
    const conn = connectionStore.getById(content.connectionId);
    if (!conn) return null;
    const shortName = conn.name.length > 14 ? conn.name.slice(0, 13) + '…' : conn.name;
    return { color: conn.color, shortName };
  }

  // For global header: show total items and new query button
  const allOpenItems = $derived(panelStore.openItems);
  const focusedContent = $derived(panelStore.focusedPanel.content);
  const hasFocusedConnection = $derived(
    focusedContent !== undefined && 'connectionId' in focusedContent,
  );

  const isMultiSplit = $derived(panelStore.splitCount > 1);

  // ── Drag state ────────────────────────────────────────────────────────────────

  let dragId = $state<string | null>(null);
  let isDragging = $state(false);
  let dropTarget = $state<{ id: string; position: 'before' | 'after' } | null>(null);
  let pointerStartY = 0;
  let dragSourceSplitId = $state<string | null>(null);

  $effect(() => {
    if (!dragId) return;

    function onMove(e: PointerEvent) {
      if (!isDragging && Math.abs(e.clientY - pointerStartY) > 4) {
        isDragging = true;
        if (dragSourceSplitId) tabDrag.start(dragId!, dragSourceSplitId);
      }
      if (!isDragging) return;

      const el = document.elementFromPoint(e.clientX, e.clientY);
      const item = el?.closest<HTMLElement>('[data-drag-id]');
      const targetId = item?.dataset.dragId;

      if (!targetId || targetId === dragId) {
        dropTarget = null;
        return;
      }

      const rect = item!.getBoundingClientRect();
      const position = e.clientY < rect.top + rect.height / 2 ? 'before' : 'after';
      dropTarget = { id: targetId, position };
    }

    function onUp() {
      if (isDragging) {
        if (dropTarget) {
          // Check if target is in a different split group
          const targetEl = document.querySelector<HTMLElement>(`[data-drag-id="${dropTarget.id}"]`);
          const targetSplitId = targetEl?.dataset.itemSplitId;

          if (targetSplitId && dragSourceSplitId && targetSplitId !== dragSourceSplitId) {
            // Cross-split move
            panelStore.moveItemToSplit(dragId!, targetSplitId);
          } else {
            // Same-split reorder
            panelStore.reorderOpenItems(dragId!, dropTarget.id, dropTarget.position);
          }
        }
        if (tabDrag.isDragging) tabDrag.end();
      }
      dragId = null;
      isDragging = false;
      dropTarget = null;
      dragSourceSplitId = null;
    }

    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);

    return () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };
  });

  function onPointerDown(e: PointerEvent, id: string, srcSplitId: string) {
    if (e.button !== 0) return;
    if ((e.target as HTMLElement).closest('.close-btn')) return;
    pointerStartY = e.clientY;
    dragId = id;
    dragSourceSplitId = srcSplitId;
  }

  let confirmCloseItemId = $state<string | null>(null);

  // ── Context menu + rename ─────────────────────────────────────────────────────

  let contextMenuItemId = $state<string | null>(null);
  let contextMenuItemSplitId = $state<string | null>(null);
  let contextMenuTop = $state(0);
  let contextMenuLeft = $state(0);
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

  function onContextMenu(e: MouseEvent, item: OpenItem, itemSplitId: string) {
    const hasSavedQuery = item.content.kind === 'query_editor' && !!item.content.savedQueryId;
    const hasConnection = 'connectionId' in item.content;
    const hasOtherTabs = panelStore.getSplitItems(itemSplitId).length > 1;
    const hasOtherSplits = panelStore.splitCount > 1;
    if (!hasSavedQuery && !hasConnection && !hasOtherTabs && !hasOtherSplits) return;
    e.preventDefault();
    contextMenuItemId = item.id;
    contextMenuItemSplitId = itemSplitId;
    contextMenuTop = e.clientY;
    contextMenuLeft = e.clientX;
  }

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
    const focused = panelStore.focusedPanel.content;
    const connectionId = 'connectionId' in focused ? focused.connectionId : null;
    if (connectionId) panelStore.openInFocused({ kind: 'query_editor', connectionId });
  }

  function openNewQueryEditorInSplit(sid: string) {
    const items = panelStore.getSplitItems(sid);
    const focId = panelStore.getSplitFocusedItemId(sid);
    const activeContent = items.find((i) => i.id === focId)?.content;
    const connectionId =
      activeContent && 'connectionId' in activeContent ? activeContent.connectionId : null;
    if (connectionId) panelStore.openInSplit({ kind: 'query_editor', connectionId }, sid);
  }
</script>

{#if settingsStore.settings.openItemsLocation !== 'top'}
  <div class="section">
    <div class="section-header no-select">
      <span class="header-label">OPEN</span>
      <span class="header-count">{allOpenItems.length}</span>
      <div class="spacer"></div>
      {#if hasFocusedConnection && !isMultiSplit}
        <button
          class="icon-btn"
          onclick={openNewQueryEditor}
          title="New Query Editor (⌘N)"
          aria-label="New query editor"
        >
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.9"
            stroke-linecap="round"
          >
            <line x1="12" y1="5" x2="12" y2="19"></line>
            <line x1="5" y1="12" x2="19" y2="12"></line>
          </svg>
        </button>
      {/if}
    </div>

    {#if !isMultiSplit}
      <!-- Single split: flat list (existing behaviour) -->
      {#if allOpenItems.length === 0}
        <div class="empty-hint">No open editors</div>
      {:else}
        <ul
          class="panel-list"
          class:is-dragging={isDragging}
          role="listbox"
          aria-label="Open panels"
        >
          {#each allOpenItems as item (item.id)}
            {@const isFocused =
              focusedContent !== undefined && sameContent(focusedContent, item.content)}
            {@const connInfo = panelConnInfo(item.content)}
            {#if dropTarget?.id === item.id && dropTarget.position === 'before'}
              <div class="drop-indicator" aria-hidden="true"></div>
            {/if}
            <li
              class="panel-item"
              class:focused={isFocused}
              class:dragging={isDragging && dragId === item.id}
              role="option"
              aria-selected={isFocused}
              data-drag-id={item.id}
              data-item-split-id={panelStore.getAllLeafIds()[0]}
              onclick={() => panelStore.showItem(item)}
              onkeydown={(e) => e.key === 'Enter' && panelStore.showItem(item)}
              onpointerdown={(e) => onPointerDown(e, item.id, panelStore.getAllLeafIds()[0])}
              oncontextmenu={(e) => onContextMenu(e, item, panelStore.getAllLeafIds()[0])}
              tabindex="0"
            >
              <span
                class="conn-dot"
                style={connInfo
                  ? `background:${connInfo.color ?? 'var(--color-accent)'}`
                  : 'background:transparent'}
                aria-hidden="true"
              ></span>
              <span class="panel-icon" aria-hidden="true">
                <PanelIcon content={item.content} size={14} />
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
                <span class="panel-label" title={panelLabel(item.content, dashboardsById)}
                  >{panelLabel(item.content, dashboardsById)}</span
                >
              {/if}
              {#if itemIsDirty(item)}
                <span class="dirty-dot" title="Unsaved changes" aria-label="Has unsaved changes"
                ></span>
              {/if}
              {#if connInfo}
                <span class="conn-short">{connInfo.shortName}</span>
              {/if}
              <button
                class="close-btn"
                aria-label="Close panel"
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
                  width="13"
                  height="13"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="1.8"
                  stroke-linecap="round"
                >
                  <line x1="6" y1="6" x2="18" y2="18"></line>
                  <line x1="18" y1="6" x2="6" y2="18"></line>
                </svg>
              </button>
            </li>
            {#if dropTarget?.id === item.id && dropTarget.position === 'after'}
              <div class="drop-indicator" aria-hidden="true"></div>
            {/if}
          {/each}
        </ul>
      {/if}
    {:else}
      <!-- Multi-split: group by split -->
      <div class="split-groups" class:is-dragging={isDragging}>
        {#each panelStore.getAllLeafIds() as sid}
          {@const splitItems = panelStore.getSplitItems(sid)}
          {@const splitFocId = panelStore.getSplitFocusedItemId(sid)}
          {@const splitLabel = panelStore.getSplitLabel(sid)}
          {@const splitActiveContent = splitItems.find((i) => i.id === splitFocId)?.content}
          <div class="split-group">
            <div class="split-group-header">
              <span class="split-group-label">{splitLabel}</span>
              <div class="spacer"></div>
              {#if splitActiveContent && 'connectionId' in splitActiveContent}
                <button
                  class="icon-btn icon-btn--sm"
                  onclick={() => openNewQueryEditorInSplit(sid)}
                  title="New Query Editor in {splitLabel}"
                  aria-label="New query editor in {splitLabel}"
                >
                  <svg
                    width="12"
                    height="12"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1.9"
                    stroke-linecap="round"
                  >
                    <line x1="12" y1="5" x2="12" y2="19"></line>
                    <line x1="5" y1="12" x2="19" y2="12"></line>
                  </svg>
                </button>
              {/if}
              <button
                class="icon-btn icon-btn--sm close-split-btn"
                onclick={() => panelStore.closeSplit(sid)}
                title="Close {splitLabel}"
                aria-label="Close {splitLabel}"
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

            {#if splitItems.length === 0}
              <div class="empty-hint empty-hint--group">Empty split</div>
            {:else}
              <ul class="panel-list" role="listbox" aria-label="Open panels in {splitLabel}">
                {#each splitItems as item (item.id)}
                  {@const isFocused = item.id === splitFocId && panelStore.focusedSplitId === sid}
                  {@const connInfo = panelConnInfo(item.content)}
                  {#if dropTarget?.id === item.id && dropTarget.position === 'before'}
                    <div class="drop-indicator" aria-hidden="true"></div>
                  {/if}
                  <li
                    class="panel-item"
                    class:focused={isFocused}
                    class:dragging={isDragging && dragId === item.id}
                    role="option"
                    aria-selected={isFocused}
                    data-drag-id={item.id}
                    data-item-split-id={sid}
                    onclick={() => {
                      panelStore.focusSplit(sid);
                      panelStore.showItem(item);
                    }}
                    onkeydown={(e) => {
                      if (e.key === 'Enter') {
                        panelStore.focusSplit(sid);
                        panelStore.showItem(item);
                      }
                    }}
                    onpointerdown={(e) => onPointerDown(e, item.id, sid)}
                    oncontextmenu={(e) => onContextMenu(e, item, sid)}
                    tabindex="0"
                  >
                    <span
                      class="conn-dot"
                      style={connInfo
                        ? `background:${connInfo.color ?? 'var(--color-accent)'}`
                        : 'background:transparent'}
                      aria-hidden="true"
                    ></span>
                    <span class="panel-icon" aria-hidden="true">
                      <PanelIcon content={item.content} size={14} />
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
                      <span class="panel-label" title={panelLabel(item.content, dashboardsById)}
                        >{panelLabel(item.content, dashboardsById)}</span
                      >
                    {/if}
                    {#if itemIsDirty(item)}
                      <span
                        class="dirty-dot"
                        title="Unsaved changes"
                        aria-label="Has unsaved changes"
                      ></span>
                    {/if}
                    {#if connInfo}
                      <span class="conn-short">{connInfo.shortName}</span>
                    {/if}
                    <button
                      class="close-btn"
                      aria-label="Close panel"
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
                        width="13"
                        height="13"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="1.8"
                        stroke-linecap="round"
                      >
                        <line x1="6" y1="6" x2="18" y2="18"></line>
                        <line x1="18" y1="6" x2="6" y2="18"></line>
                      </svg>
                    </button>
                  </li>
                  {#if dropTarget?.id === item.id && dropTarget.position === 'after'}
                    <div class="drop-indicator" aria-hidden="true"></div>
                  {/if}
                {/each}
              </ul>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>
{/if}

{#if contextMenuItemId !== null && contextMenuItemSplitId !== null}
  {@const allItems = panelStore.openItems}
  {@const contextItem = allItems.find((i) => i.id === contextMenuItemId)}
  {#if contextItem}
    <TabContextMenu
      item={contextItem}
      splitId={contextMenuItemSplitId}
      x={contextMenuLeft}
      y={contextMenuTop}
      open={true}
      onclose={() => (contextMenuItemId = null)}
      onrename={(item) => {
        if (item.content.kind !== 'query_editor') return;
        renamingItemId = item.id;
        renameValue = item.content.savedQueryName ?? 'Query';
      }}
    />
  {/if}
{/if}

{#if confirmCloseItemId !== null}
  {@const itemToClose = panelStore.openItems.find((i) => i.id === confirmCloseItemId)}
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
  .section {
    max-height: 50%;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    padding: 9px 8px;
    border-bottom: 1px solid var(--color-border);
  }

  .section-header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 1px 6px 7px;
    min-height: 32px;
  }

  .header-label {
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.06em;
    color: var(--color-text-muted);
  }

  .header-count {
    font-size: 10.5px;
    color: var(--color-text-disabled);
  }

  .spacer {
    flex: 1;
  }

  .icon-btn {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    flex-shrink: 0;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .icon-btn--sm {
    width: 20px;
    height: 20px;
    border-radius: var(--radius-sm);
  }

  .icon-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .close-split-btn:hover {
    background: color-mix(in srgb, var(--color-danger, #e53e3e) 12%, transparent);
    color: var(--color-danger, #e53e3e);
  }

  .empty-hint {
    padding: 2px 10px 4px;
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
  }

  .empty-hint--group {
    padding: 4px 12px 4px;
    font-size: 11px;
  }

  /* ── Split groups ─────────────────────────────────────────────────────────── */

  .split-groups {
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    min-height: 0;
    flex: 1;
  }

  .split-groups.is-dragging {
    cursor: grabbing;
  }

  .split-group {
    display: flex;
    flex-direction: column;
  }

  .split-group + .split-group {
    border-top: 1px solid var(--color-border);
    margin-top: 2px;
    padding-top: 2px;
  }

  .split-group-header {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 8px 2px 12px;
    min-height: 24px;
  }

  .split-group-label {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.06em;
    color: var(--color-text-disabled);
    text-transform: uppercase;
  }

  /* ── Items list ─────────────────────────────────────────────────────────── */

  .panel-list {
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    min-height: 0;
    padding: 1px 0;
  }

  .panel-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 8px 7px 10px;
    font-size: 13px;
    color: var(--color-text-secondary);
    cursor: pointer;
    border-radius: var(--radius-lg);
    -webkit-user-select: none;
    user-select: none;
    outline: none;
    transition: background var(--transition-fast);
  }

  .panel-item:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .panel-item.focused {
    background: var(--color-accent-subtle);
    color: var(--color-accent);
    font-weight: 600;
    box-shadow: inset 2px 0 0 var(--color-accent);
  }

  .panel-item.dragging {
    opacity: 0.4;
  }

  .drop-indicator {
    height: 2px;
    margin: -1px 18px;
    background: var(--color-accent);
    border-radius: var(--radius-xs);
    flex-shrink: 0;
    pointer-events: none;
  }

  .conn-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
    background: var(--color-text-disabled);
  }

  .panel-icon {
    flex-shrink: 0;
    display: flex;
    align-items: center;
  }

  .panel-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .conn-short {
    font-size: 10px;
    color: var(--color-text-disabled);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 78px;
    flex-shrink: 0;
    font-weight: initial;
  }

  .dirty-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--color-warning, #f59e0b);
    flex-shrink: 0;
  }

  .rename-input {
    flex: 1;
    min-width: 0;
    height: 20px;
    padding: 0 var(--spacing-1);
    background: var(--color-bg-input, var(--color-bg-secondary));
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-sm);
    color: var(--color-text-primary);
    font-size: 13px;
    font-family: var(--font-family-ui);
    outline: none;
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

  .panel-item:hover .close-btn,
  .panel-item.focused .close-btn {
    opacity: 1;
  }

  .close-btn:hover {
    background: var(--color-bg-active);
    color: var(--color-text-primary);
  }
</style>
