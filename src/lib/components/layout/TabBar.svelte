<!--
  TabBar — horizontal open-items tab strip rendered at the top of the main area.
  Shown when openItemsLocation === 'top'. Mirrors SidebarTopHalf's logic but
  laid out as a scrollable row of tabs.
-->
<script lang="ts">
  import { usePanels, sameContent, dirtyKeyForContent } from '$lib/stores/panels.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import TableIcon from '$lib/components/icons/TableIcon.svelte';
  import { isSystemDatabase, isSystemTable } from '$lib/utils/system-items';
  import type { PanelKind } from '$lib/types';
  import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
  import { clearTablePendingState } from '$lib/components/table/TableBrowser.svelte';
  import * as savedQueriesApi from '$lib/tauri/saved_queries';
  import { queryEditorCache } from '$lib/stores/queryEditorState';
  import ContextMenu from '$lib/components/ui/ContextMenu.svelte';
  import CtxItem from '$lib/components/ui/CtxItem.svelte';

  const panelStore = usePanels();
  const connectionStore = useConnections();
  const settingsStore = useSettings();

  function panelLabel(content: PanelKind): string {
    switch (content.kind) {
      case 'query_editor':
        return content.savedQueryName ?? 'Query';
      case 'table_browser':
        return content.table;
      case 'table_structure':
        return content.table;
      case 'ddl_viewer':
        return content.objectName;
      case 'erd':
        return 'ERD';
      case 'explain':
        return 'Explain';
      case 'settings':
        return 'Settings';
      case 'user_manager':
        return 'Users';
      case 'speed_analysis':
        return 'Speed Analysis';
      case 'empty':
        return 'Empty';
    }
  }

  function panelConnInfo(content: PanelKind): { color: string | null } | null {
    if (!('connectionId' in content)) return null;
    const conn = connectionStore.getById(content.connectionId);
    if (!conn) return null;
    return { color: conn.color };
  }

  const focusedContent = $derived(panelStore.panels[panelStore.focusedIndex]?.content);
  const hasFocusedConnection = $derived(
    focusedContent !== undefined && 'connectionId' in focusedContent,
  );

  // ── Drag state (horizontal) ───────────────────────────────────────────────

  let dragId = $state<string | null>(null);
  let isDragging = $state(false);
  let dropTarget = $state<{ id: string; position: 'before' | 'after' } | null>(null);
  let pointerStartX = 0;

  $effect(() => {
    if (!dragId) return;

    function onMove(e: PointerEvent) {
      if (!isDragging && Math.abs(e.clientX - pointerStartX) > 4) {
        isDragging = true;
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
      const position = e.clientX < rect.left + rect.width / 2 ? 'before' : 'after';
      dropTarget = { id: targetId, position };
    }

    function onUp() {
      if (isDragging && dropTarget) {
        panelStore.reorderOpenItems(dragId!, dropTarget.id, dropTarget.position);
      }
      dragId = null;
      isDragging = false;
      dropTarget = null;
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
  }

  let confirmCloseItemId = $state<string | null>(null);

  // ── Context menu ──────────────────────────────────────────────────────────

  let contextMenuItemId = $state<string | null>(null);
  let contextMenuTop = $state(0);
  let contextMenuLeft = $state(0);

  function onContextMenu(e: MouseEvent, item: import('$lib/stores/panels.svelte').OpenItem) {
    const hasConnection = 'connectionId' in item.content;
    const hasSavedQuery = item.content.kind === 'query_editor' && !!item.content.savedQueryId;
    const hasOtherTabs = panelStore.openItems.length > 1;
    if (!hasConnection && !hasSavedQuery && !hasOtherTabs) return;
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

  async function commitRename(item: import('$lib/stores/panels.svelte').OpenItem) {
    if (!renameValue.trim() || item.content.kind !== 'query_editor' || !item.content.savedQueryId) {
      renamingItemId = null;
      return;
    }
    const name = renameValue.trim();
    const currentSql = item.content.editorId
      ? (queryEditorCache.get(item.content.editorId)?.sql ?? item.content.initialSql ?? '')
      : (item.content.initialSql ?? '');
    try {
      const updated = await savedQueriesApi.fileUpdateSavedQuery(item.content.savedQueryId, {
        name,
        sql: currentSql,
        connectionId: item.content.connectionId,
      });
      if (item.content.editorId) {
        panelStore.updateQueryEditorMeta(item.content.editorId, {
          savedQueryName: name,
          savedQueryId: updated.id,
        });
      }
    } catch {
      if (item.content.editorId) {
        panelStore.updateQueryEditorMeta(item.content.editorId, { savedQueryName: name });
      }
    }
    renamingItemId = null;
  }

  function itemIsDirty(item: import('$lib/stores/panels.svelte').OpenItem): boolean {
    const key = dirtyKeyForContent(item.content);
    return key ? panelStore.isItemDirty(key) : false;
  }

  function openNewQueryEditor() {
    const focused = panelStore.focusedPanel.content;
    const connectionId = 'connectionId' in focused ? focused.connectionId : null;
    if (connectionId) panelStore.openInFocused({ kind: 'query_editor', connectionId });
  }
</script>

<div class="tab-bar" class:is-dragging={isDragging}>
  <div class="tabs-scroll">
    {#each panelStore.openItems as item (item.id)}
      {@const isFocused = focusedContent !== undefined && sameContent(focusedContent, item.content)}
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
        onclick={() => panelStore.showItem(item)}
        onkeydown={(e) => e.key === 'Enter' && panelStore.showItem(item)}
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
          {#if item.content.kind === 'table_browser'}
            <TableIcon
              system={isSystemDatabase(
                item.content.database,
                settingsStore.settings.systemDatabases,
              ) || isSystemTable(item.content.table, settingsStore.settings.systemTablePatterns)}
            />
          {:else if item.content.kind === 'table_structure'}
            <svg
              width="13"
              height="13"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.7"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <rect x="3" y="4" width="18" height="16" rx="2"></rect>
              <line x1="9" y1="4" x2="9" y2="20"></line>
              <line x1="15" y1="4" x2="15" y2="20"></line>
            </svg>
          {:else if item.content.kind === 'query_editor'}
            <svg
              width="13"
              height="13"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.7"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <polyline points="8 7 4 12 8 17"></polyline>
              <polyline points="16 7 20 12 16 17"></polyline>
            </svg>
          {:else if item.content.kind === 'ddl_viewer'}
            <svg
              width="13"
              height="13"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.7"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
              <polyline points="14 2 14 8 20 8"></polyline>
            </svg>
          {:else if item.content.kind === 'settings'}
            <svg
              width="13"
              height="13"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.7"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <circle cx="12" cy="12" r="3"></circle>
              <path
                d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"
              ></path>
            </svg>
          {:else if item.content.kind === 'user_manager'}
            <svg
              width="13"
              height="13"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.7"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <circle cx="12" cy="8" r="4"></circle>
              <path d="M4 20c0-4 3.6-7 8-7s8 3 8 7"></path>
            </svg>
          {:else if item.content.kind === 'speed_analysis'}
            <svg
              width="13"
              height="13"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.7"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <line x1="18" y1="20" x2="18" y2="10"></line>
              <line x1="12" y1="20" x2="12" y2="4"></line>
              <line x1="6" y1="20" x2="6" y2="14"></line>
            </svg>
          {:else}
            <svg
              width="13"
              height="13"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.7"
              stroke-linecap="round"
            >
              <rect x="3" y="3" width="18" height="18" rx="2"></rect>
            </svg>
          {/if}
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
          <span class="tab-label" title={panelLabel(item.content)}>{panelLabel(item.content)}</span>
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
        width="13"
        height="13"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
      >
        <line x1="12" y1="5" x2="12" y2="19"></line>
        <line x1="5" y1="12" x2="19" y2="12"></line>
      </svg>
    </button>
  {/if}
</div>

{#if contextMenuItemId !== null}
  {@const contextItem = panelStore.openItems.find((i) => i.id === contextMenuItemId)}
  {#if contextItem}
    <ContextMenu
      x={contextMenuLeft}
      y={contextMenuTop}
      open={true}
      onclose={() => (contextMenuItemId = null)}
      minWidth={200}
    >
      {#if contextItem.content.kind === 'query_editor' && contextItem.content.savedQueryId}
        <CtxItem
          onclick={() => {
            contextMenuItemId = null;
            renamingItemId = contextItem.id;
            renameValue =
              contextItem.content.kind === 'query_editor'
                ? (contextItem.content.savedQueryName ?? 'Query')
                : '';
          }}>Rename</CtxItem
        >
      {/if}
      {#if panelStore.openItems.length > 1}
        <CtxItem
          onclick={() => {
            const id = contextItem.id;
            contextMenuItemId = null;
            panelStore.closeOtherItems(id);
          }}>Close other tabs</CtxItem
        >
      {/if}
      {#if 'connectionId' in contextItem.content}
        <CtxItem
          onclick={() => {
            const connId = (contextItem.content as { connectionId: string }).connectionId;
            contextMenuItemId = null;
            panelStore.closeItemsForConnection(connId);
          }}>Close all tabs for this connection</CtxItem
        >
      {/if}
    </ContextMenu>
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
  .tab-bar {
    display: flex;
    align-items: center;
    height: 36px;
    flex-shrink: 0;
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    overflow: hidden;
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
    border-radius: 1px;
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
