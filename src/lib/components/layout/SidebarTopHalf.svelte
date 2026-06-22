<!--
  SidebarTopHalf — open panels list.
  Header: OPEN label + count + new-window button + collapse button.
  Each row: connection color dot + type SVG icon + name + connection short name + × close.
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


  const panelStore = usePanels();
  const connectionStore = useConnections();
  const settingsStore = useSettings();

  function panelLabel(content: PanelKind): string {
    switch (content.kind) {
      case 'query_editor': return 'Query';
      case 'table_browser': return content.table;
      case 'table_structure': return content.table;
      case 'ddl_viewer': return content.objectName;
      case 'erd': return 'ERD';
      case 'explain': return 'Explain';
      case 'settings': return 'Settings';
      case 'user_manager': return 'Users';
      case 'empty': return 'Empty';
    }
  }

  function panelConnInfo(content: PanelKind): { color: string | null; shortName: string } | null {
    if (!('connectionId' in content)) return null;
    const conn = connectionStore.getById(content.connectionId);
    if (!conn) return null;
    const shortName = conn.name.length > 14 ? conn.name.slice(0, 13) + '…' : conn.name;
    return { color: conn.color, shortName };
  }

  const focusedContent = $derived(panelStore.panels[panelStore.focusedIndex]?.content);
  const hasFocusedConnection = $derived(focusedContent !== undefined && 'connectionId' in focusedContent);

  // ── Drag state ────────────────────────────────────────────────────────────────

  let dragId = $state<string | null>(null);
  let isDragging = $state(false);
  let dropTarget = $state<{ id: string; position: 'before' | 'after' } | null>(null);
  let pointerStartY = 0;

  $effect(() => {
    if (!dragId) return;

    function onMove(e: PointerEvent) {
      if (!isDragging && Math.abs(e.clientY - pointerStartY) > 4) {
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
      const position = e.clientY < rect.top + rect.height / 2 ? 'before' : 'after';
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
    pointerStartY = e.clientY;
    dragId = id;
  }

  let confirmCloseItemId = $state<string | null>(null);

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

<div class="section">
  <div class="section-header no-select">
    <span class="header-label">OPEN</span>
    <span class="header-count">{panelStore.openItems.length}</span>
    <div class="spacer"></div>
    {#if hasFocusedConnection}
    <button
      class="icon-btn"
      onclick={openNewQueryEditor}
      title="New Query Editor (⌘N)"
      aria-label="New query editor"
    >
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round">
        <line x1="12" y1="5" x2="12" y2="19"></line>
        <line x1="5" y1="12" x2="19" y2="12"></line>
      </svg>
    </button>
    {/if}
  </div>

  {#if panelStore.openItems.length === 0}
    <div class="empty-hint">No open editors</div>
  {:else}
    <ul class="panel-list" class:is-dragging={isDragging} role="listbox" aria-label="Open panels">
      {#each panelStore.openItems as item (item.id)}
        {@const isFocused = focusedContent !== undefined && sameContent(focusedContent, item.content)}
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
          onclick={() => panelStore.showItem(item)}
          onkeydown={(e) => e.key === 'Enter' && panelStore.showItem(item)}
          onpointerdown={(e) => onPointerDown(e, item.id)}
          tabindex="0"
        >
          <span
            class="conn-dot"
            style={connInfo?.color ? `background:${connInfo.color}` : (item.content.kind === 'settings' ? 'background:transparent' : undefined)}
            aria-hidden="true"
          ></span>
          <span class="panel-icon" aria-hidden="true">
            {#if item.content.kind === 'table_browser'}
              <TableIcon system={isSystemDatabase(item.content.database, settingsStore.settings.systemDatabases) || isSystemTable(item.content.table, settingsStore.settings.systemTablePatterns)} />
            {:else if item.content.kind === 'table_structure'}
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
                <rect x="3" y="4" width="18" height="16" rx="2"></rect>
                <line x1="9" y1="4" x2="9" y2="20"></line>
                <line x1="15" y1="4" x2="15" y2="20"></line>
              </svg>
            {:else if item.content.kind === 'query_editor'}
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="8 7 4 12 8 17"></polyline>
                <polyline points="16 7 20 12 16 17"></polyline>
              </svg>
            {:else if item.content.kind === 'ddl_viewer'}
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
                <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
                <polyline points="14 2 14 8 20 8"></polyline>
                <line x1="8" y1="13" x2="16" y2="13"></line>
                <line x1="8" y1="17" x2="13" y2="17"></line>
              </svg>
            {:else if item.content.kind === 'settings'}
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="12" r="3"></circle>
                <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
              </svg>
            {:else if item.content.kind === 'user_manager'}
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="8" r="4"></circle>
                <path d="M4 20c0-4 3.6-7 8-7s8 3 8 7"></path>
              </svg>
            {:else}
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round">
                <rect x="3" y="3" width="18" height="18" rx="2"></rect>
              </svg>
            {/if}
          </span>
          <span class="panel-label">{panelLabel(item.content)}</span>
          {#if itemIsDirty(item)}
            <span class="dirty-dot" title="Unsaved changes" aria-label="Has unsaved changes"></span>
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
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
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

{#if confirmCloseItemId !== null}
  {@const itemToClose = panelStore.openItems.find(i => i.id === confirmCloseItemId)}
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
      oncancel={() => { confirmCloseItemId = null; }}
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

  .spacer { flex: 1; }

  .icon-btn {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 6px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    flex-shrink: 0;
    transition: background var(--transition-fast), color var(--transition-fast);
  }

  .icon-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .empty-hint {
    padding: 2px 10px 4px;
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
  }

  .panel-list {
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    min-height: 0;
    padding: 1px 0;
  }

  .panel-list.is-dragging {
    cursor: grabbing;
  }

  .panel-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 8px 7px 10px;
    font-size: 13px;
    color: var(--color-text-secondary);
    cursor: pointer;
    border-radius: 8px;
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
    border-radius: 1px;
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
    transition: opacity var(--transition-fast), background var(--transition-fast);
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
