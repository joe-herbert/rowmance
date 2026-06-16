<!--
  RightSidebar — toggleable right panel with icon tabs.
  Only one panel is active at a time; clicking the active tab collapses the sidebar.
  Panels: Query History, Saved Queries.
-->
<script lang="ts">
  import { useConnections } from '$lib/stores/connections.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import * as historyApi from '$lib/tauri/history';
  import * as savedQueriesApi from '$lib/tauri/saved_queries';
  import type { QueryHistoryEntry, SavedQuery, SavedQueryFolder } from '$lib/types';
  import { errorMessage } from '$lib/utils/errors';
  import RelationsPanel from '$lib/components/relations/RelationsPanel.svelte';

  type ActivePanel = 'history' | 'saved' | 'column' | 'table-info' | 'relations' | null;

  interface Props {
    onClose: () => void;
  }

  const { onClose }: Props = $props();

  const connectionStore = useConnections();
  const panelStore = usePanels();

  let activePanel = $state<ActivePanel>('history');

  function selectPanel(panel: ActivePanel) {
    if (activePanel === panel) {
      onClose();
    } else {
      activePanel = panel;
      if (panel === 'history') loadHistory();
      if (panel === 'saved') loadSavedQueries();
    }
  }

  // ── Relations panel ───────────────────────────────────────────────────────

  // ── Column Inspector placeholder ──────────────────────────────────────────────
  // Future: receive selected column via event or store; for now, shows a hint.

  // ── Table Info placeholder ────────────────────────────────────────────────────
  // Future: receive focused table and run a quick schema query to show stats.

  // ── Query History ─────────────────────────────────────────────────────────────

  let historyEntries = $state<QueryHistoryEntry[]>([]);
  let historyLoading = $state(false);
  let historyError = $state<string | null>(null);
  let selectedHistoryConnectionId = $state<string>('');

  const activeProfiles = $derived(
    connectionStore.profiles.filter((p) => connectionStore.isActive(p.id)),
  );

  $effect(() => {
    // When active connections change, default to the first one.
    const ids = activeProfiles.map((p) => p.id);
    if (ids.length > 0 && !ids.includes(selectedHistoryConnectionId)) {
      selectedHistoryConnectionId = ids[0];
    }
  });

  async function loadHistory() {
    if (!selectedHistoryConnectionId) return;
    historyLoading = true;
    historyError = null;
    try {
      historyEntries = await historyApi.listHistory(selectedHistoryConnectionId, 100, 0);
    } catch (err) {
      historyError = errorMessage(err);
    } finally {
      historyLoading = false;
    }
  }

  async function handleClearHistory() {
    if (!selectedHistoryConnectionId) return;
    await historyApi.clearHistory(selectedHistoryConnectionId);
    historyEntries = [];
  }

  function openHistoryEntry(entry: QueryHistoryEntry) {
    panelStore.openInFocused({
      kind: 'query_editor',
      connectionId: entry.connectionId,
      initialSql: entry.sql,
    });
  }

  function formatDuration(ms: number | null): string {
    if (ms === null) return '';
    if (ms < 1000) return `${ms}ms`;
    return `${(ms / 1000).toFixed(1)}s`;
  }

  function timeAgo(isoString: string): string {
    const diff = Date.now() - new Date(isoString).getTime();
    const mins = Math.floor(diff / 60_000);
    if (mins < 1) return 'just now';
    if (mins < 60) return `${mins}m ago`;
    const hrs = Math.floor(mins / 60);
    if (hrs < 24) return `${hrs}h ago`;
    return `${Math.floor(hrs / 24)}d ago`;
  }

  function truncateSql(sql: string, max = 80): string {
    const cleaned = sql.replace(/\s+/g, ' ').trim();
    return cleaned.length > max ? cleaned.slice(0, max) + '…' : cleaned;
  }

  // Reload history when the connection selector changes.
  $effect(() => {
    if (activePanel === 'history' && selectedHistoryConnectionId) {
      loadHistory();
    }
  });

  // ── Saved Queries ─────────────────────────────────────────────────────────────

  let savedFolders = $state<SavedQueryFolder[]>([]);
  let savedQueries = $state<SavedQuery[]>([]);
  let savedLoading = $state(false);
  let savedError = $state<string | null>(null);
  let expandedFolders = $state<Set<string>>(new Set());

  // Inline new-folder input
  let newFolderName = $state('');
  let showNewFolder = $state(false);

  // Inline new-query input
  let newQueryName = $state('');
  let newQueryFolderId = $state<string | null>(null);
  let showNewQuery = $state(false);

  // Context menu
  interface SavedCtxMenu {
    x: number;
    y: number;
    kind: 'query' | 'folder';
    id: string;
    name: string;
  }
  let savedCtxMenu = $state<SavedCtxMenu | null>(null);

  async function loadSavedQueries() {
    savedLoading = true;
    savedError = null;
    try {
      [savedFolders, savedQueries] = await Promise.all([
        savedQueriesApi.listFolders(),
        savedQueriesApi.listSavedQueries(),
      ]);
    } catch (err) {
      savedError = errorMessage(err);
    } finally {
      savedLoading = false;
    }
  }

  function toggleFolder(folderId: string) {
    const next = new Set(expandedFolders);
    if (next.has(folderId)) next.delete(folderId);
    else next.add(folderId);
    expandedFolders = next;
  }

  function openSavedQuery(query: SavedQuery) {
    panelStore.openInFocused({
      kind: 'query_editor',
      connectionId: query.connectionId ?? connectionStore.profiles[0]?.id ?? '',
      initialSql: query.sql,
    });
  }

  async function createFolder() {
    if (!newFolderName.trim()) return;
    await savedQueriesApi.createFolder({ name: newFolderName.trim() });
    newFolderName = '';
    showNewFolder = false;
    await loadSavedQueries();
  }

  async function createQuery() {
    if (!newQueryName.trim()) return;
    const connectionId = connectionStore.profiles.find((p) => connectionStore.isActive(p.id))?.id ?? null;
    await savedQueriesApi.createSavedQuery({
      name: newQueryName.trim(),
      sql: '',
      folderId: newQueryFolderId,
      connectionId,
    });
    newQueryName = '';
    showNewQuery = false;
    newQueryFolderId = null;
    await loadSavedQueries();
  }

  function showSavedCtxMenu(e: MouseEvent, kind: 'query' | 'folder', id: string, name: string) {
    e.preventDefault();
    savedCtxMenu = { x: e.clientX, y: e.clientY, kind, id, name };
  }

  async function handleSavedCtxDelete() {
    if (!savedCtxMenu) return;
    if (savedCtxMenu.kind === 'query') {
      await savedQueriesApi.deleteSavedQuery(savedCtxMenu.id);
    } else {
      await savedQueriesApi.deleteFolder(savedCtxMenu.id);
    }
    savedCtxMenu = null;
    await loadSavedQueries();
  }

  function closeSavedCtxMenu() {
    savedCtxMenu = null;
  }

  // Load saved queries when the panel first opens.
  $effect(() => {
    if (activePanel === 'saved' && savedFolders.length === 0 && savedQueries.length === 0 && !savedLoading) {
      loadSavedQueries();
    }
  });

  // Group queries by folderId.
  const queriesByFolder = $derived<Map<string | null, SavedQuery[]>>(() => {
    const map = new Map<string | null, SavedQuery[]>();
    for (const q of savedQueries) {
      const key = q.folderId;
      const arr = map.get(key) ?? [];
      arr.push(q);
      map.set(key, arr);
    }
    return map;
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="right-sidebar" onclick={() => { savedCtxMenu = null; }}>
  <!-- Icon tab strip -->
  <div class="tab-strip" role="tablist" aria-label="Right sidebar panels">
    <button
      class="tab-btn"
      class:active={activePanel === 'history'}
      role="tab"
      aria-selected={activePanel === 'history'}
      aria-controls="panel-history"
      title="Query History"
      onclick={(e) => { e.stopPropagation(); selectPanel('history'); }}
    >
      🕐
    </button>

    <button
      class="tab-btn"
      class:active={activePanel === 'saved'}
      role="tab"
      aria-selected={activePanel === 'saved'}
      aria-controls="panel-saved"
      title="Saved Queries"
      onclick={(e) => { e.stopPropagation(); selectPanel('saved'); }}
    >
      🔖
    </button>

    <button
      class="tab-btn"
      class:active={activePanel === 'column'}
      role="tab"
      aria-selected={activePanel === 'column'}
      aria-controls="panel-column"
      title="Column Inspector"
      onclick={(e) => { e.stopPropagation(); selectPanel('column'); }}
    >
      ≡
    </button>

    <button
      class="tab-btn"
      class:active={activePanel === 'table-info'}
      role="tab"
      aria-selected={activePanel === 'table-info'}
      aria-controls="panel-table-info"
      title="Table Info"
      onclick={(e) => { e.stopPropagation(); selectPanel('table-info'); }}
    >
      ℹ
    </button>

    <button
      class="tab-btn"
      class:active={activePanel === 'relations'}
      role="tab"
      aria-selected={activePanel === 'relations'}
      aria-controls="panel-relations"
      title="Relations"
      onclick={(e) => { e.stopPropagation(); selectPanel('relations'); }}
    >
      ↔
    </button>

    <div class="spacer"></div>

    <button class="tab-btn close-btn" title="Close sidebar" onclick={onClose} aria-label="Close sidebar">
      ›
    </button>
  </div>

  <!-- Panel content -->
  <div class="panel-content">
    {#if activePanel === 'history'}
      <div id="panel-history" role="tabpanel" aria-label="Query History">
        <div class="panel-toolbar">
          {#if activeProfiles.length > 1}
            <select
              class="connection-select"
              bind:value={selectedHistoryConnectionId}
              aria-label="Select connection"
            >
              {#each activeProfiles as profile (profile.id)}
                <option value={profile.id}>{profile.name}</option>
              {/each}
            </select>
          {:else if activeProfiles.length === 1}
            <span class="connection-label">{activeProfiles[0].name}</span>
          {:else}
            <span class="muted">No active connections</span>
          {/if}

          <div class="toolbar-gap"></div>

          {#if historyEntries.length > 0}
            <button class="action-btn danger-btn" onclick={handleClearHistory} title="Clear history">
              Clear
            </button>
          {/if}
        </div>

        {#if historyLoading}
          <div class="loading-row">Loading…</div>
        {:else if historyError}
          <div class="error-row">{historyError}</div>
        {:else if historyEntries.length === 0}
          <div class="empty-row">No history yet.</div>
        {:else}
          <ul class="history-list" role="list">
            {#each historyEntries as entry (entry.id)}
              <li class="history-item" role="listitem">
                <button
                  class="history-btn"
                  onclick={() => openHistoryEntry(entry)}
                  title="Open in editor"
                >
                  <span class="history-sql">{truncateSql(entry.sql)}</span>
                  <span class="history-meta">
                    <span
                      class="status-badge"
                      class:success={entry.status === 'success'}
                      class:error={entry.status === 'error'}
                    >
                      {entry.status}
                    </span>
                    {#if entry.durationMs !== null}
                      <span class="meta-item">{formatDuration(entry.durationMs)}</span>
                    {/if}
                    <span class="meta-item time-ago">{timeAgo(entry.executedAt)}</span>
                  </span>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>

    {:else if activePanel === 'saved'}
      <div id="panel-saved" role="tabpanel" aria-label="Saved Queries">
        <div class="panel-toolbar">
          <span class="panel-title">Saved Queries</span>
          <div class="toolbar-gap"></div>
          <button class="action-btn" onclick={() => { showNewFolder = !showNewFolder; }} title="New folder">
            + Folder
          </button>
          <button class="action-btn" onclick={() => { showNewQuery = !showNewQuery; }} title="New query">
            + Query
          </button>
        </div>

        {#if showNewFolder}
          <div class="inline-input-row">
            <input
              class="inline-input"
              type="text"
              placeholder="Folder name…"
              bind:value={newFolderName}
              onkeydown={(e) => { if (e.key === 'Enter') createFolder(); if (e.key === 'Escape') { showNewFolder = false; } }}
              aria-label="New folder name"
            />
            <button class="action-btn" onclick={createFolder}>Add</button>
          </div>
        {/if}

        {#if showNewQuery}
          <div class="inline-input-row">
            <input
              class="inline-input"
              type="text"
              placeholder="Query name…"
              bind:value={newQueryName}
              onkeydown={(e) => { if (e.key === 'Enter') createQuery(); if (e.key === 'Escape') { showNewQuery = false; } }}
              aria-label="New query name"
            />
            <button class="action-btn" onclick={createQuery}>Add</button>
          </div>
        {/if}

        {#if savedLoading}
          <div class="loading-row">Loading…</div>
        {:else if savedError}
          <div class="error-row">{savedError}</div>
        {:else}
          <ul class="saved-list" role="tree" aria-label="Saved queries">
            <!-- Unfiled queries -->
            {#if (queriesByFolder().get(null) ?? []).length > 0}
              <li class="folder-node" role="treeitem">
                <span class="folder-label muted">Unfiled</span>
                <ul class="folder-children" role="group">
                  {#each queriesByFolder().get(null) ?? [] as query (query.id)}
                    <li class="query-node" role="treeitem">
                      <button
                        class="query-btn"
                        onclick={() => openSavedQuery(query)}
                        oncontextmenu={(e) => showSavedCtxMenu(e, 'query', query.id, query.name)}
                        title="Open {query.name}"
                      >
                        <span class="query-icon" aria-hidden="true">⌨</span>
                        <span class="query-name">{query.name}</span>
                      </button>
                    </li>
                  {/each}
                </ul>
              </li>
            {/if}

            <!-- Folders -->
            {#each savedFolders as folder (folder.id)}
              {@const folderQueries = queriesByFolder().get(folder.id) ?? []}
              {@const isOpen = expandedFolders.has(folder.id)}
              <li class="folder-node" role="treeitem" aria-expanded={isOpen}>
                <button
                  class="folder-btn"
                  onclick={() => toggleFolder(folder.id)}
                  oncontextmenu={(e) => showSavedCtxMenu(e, 'folder', folder.id, folder.name)}
                  aria-label="{isOpen ? 'Collapse' : 'Expand'} folder {folder.name}"
                >
                  <span class="chevron" class:open={isOpen} aria-hidden="true">›</span>
                  <span class="folder-icon" aria-hidden="true">📁</span>
                  <span class="folder-name">{folder.name}</span>
                  {#if folderQueries.length > 0}
                    <span class="count-badge">{folderQueries.length}</span>
                  {/if}
                </button>

                {#if isOpen && folderQueries.length > 0}
                  <ul class="folder-children" role="group">
                    {#each folderQueries as query (query.id)}
                      <li class="query-node" role="treeitem">
                        <button
                          class="query-btn"
                          onclick={() => openSavedQuery(query)}
                          oncontextmenu={(e) => showSavedCtxMenu(e, 'query', query.id, query.name)}
                          title="Open {query.name}"
                        >
                          <span class="query-icon" aria-hidden="true">⌨</span>
                          <span class="query-name">{query.name}</span>
                        </button>
                      </li>
                    {/each}
                  </ul>
                {/if}
              </li>
            {/each}

            {#if savedFolders.length === 0 && savedQueries.length === 0}
              <li><div class="empty-row">No saved queries yet.</div></li>
            {/if}
          </ul>
        {/if}
      </div>
    {:else if activePanel === 'column'}
      <div id="panel-column" role="tabpanel" aria-label="Column Inspector">
        <div class="panel-toolbar">
          <span class="panel-title">Column Inspector</span>
        </div>
        <div class="placeholder-panel">
          <p>Click a column in the schema tree to inspect it.</p>
        </div>
      </div>

    {:else if activePanel === 'table-info'}
      <div id="panel-table-info" role="tabpanel" aria-label="Table Info">
        <div class="panel-toolbar">
          <span class="panel-title">Table Info</span>
        </div>
        <div class="placeholder-panel">
          <p>Open a table from the schema tree to see its details here.</p>
        </div>
      </div>

    {:else if activePanel === 'relations'}
      <div id="panel-relations" role="tabpanel" aria-label="Relations" class="relations-tabpanel">
        <div class="panel-toolbar">
          <span class="panel-title">Relations</span>
        </div>
        <RelationsPanel />
      </div>
    {/if}
  </div>
</div>

<!-- Saved queries context menu -->
{#if savedCtxMenu}
  <div
    class="ctx-menu"
    style="top: {savedCtxMenu.y}px; left: {savedCtxMenu.x}px;"
    role="menu"
    aria-label="Actions for {savedCtxMenu.name}"
  >
    {#if savedCtxMenu.kind === 'query'}
      <button class="ctx-item" role="menuitem" onclick={() => { openSavedQuery(savedQueries.find(q => q.id === savedCtxMenu?.id)!); closeSavedCtxMenu(); }}>
        Open
      </button>
    {/if}
    <button class="ctx-item danger" role="menuitem" onclick={handleSavedCtxDelete}>
      Delete
    </button>
  </div>
{/if}

<style>
  .right-sidebar {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .tab-strip {
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 2px;
    padding: var(--spacing-1) var(--spacing-2);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .tab-btn {
    width: 32px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    border-radius: var(--radius-sm);
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .tab-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .tab-btn.active {
    background: var(--color-accent-subtle);
    color: var(--color-accent);
  }

  .spacer {
    flex: 1;
  }

  .close-btn {
    font-size: var(--font-size-lg);
  }

  .panel-content {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .panel-content > div {
    flex: 1;
    display: flex;
    flex-direction: column;
  }

  .panel-toolbar {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: var(--spacing-1) var(--spacing-2);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .panel-title {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .toolbar-gap {
    flex: 1;
  }

  .connection-select {
    font-size: var(--font-size-xs);
    padding: 2px var(--spacing-1);
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--color-text-primary);
    max-width: 130px;
  }

  .connection-label {
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
    font-weight: var(--font-weight-medium);
  }

  .action-btn {
    font-size: var(--font-size-xs);
    padding: 2px var(--spacing-2);
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--color-text-secondary);
    cursor: pointer;
    transition: background var(--transition-fast);
    white-space: nowrap;
  }

  .action-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .danger-btn {
    color: var(--color-danger);
    border-color: var(--color-danger);
  }

  .danger-btn:hover {
    background: var(--color-danger-subtle);
  }

  .muted {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
  }

  .loading-row,
  .empty-row {
    padding: var(--spacing-3) var(--spacing-2);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
  }

  .error-row {
    padding: var(--spacing-2);
    font-size: var(--font-size-xs);
    color: var(--color-danger);
    background: var(--color-danger-subtle);
    border-radius: var(--radius-sm);
    margin: var(--spacing-1) var(--spacing-2);
  }

  /* ── History panel ─────────────────────────────────────────────────────── */

  .history-list {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-1) 0;
  }

  .history-item {
    border-bottom: 1px solid var(--color-border);
  }

  .history-btn {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: var(--spacing-2) var(--spacing-3);
    text-align: left;
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .history-btn:hover {
    background: var(--color-bg-hover);
  }

  .history-sql {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }

  .history-meta {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
  }

  .status-badge {
    font-size: 10px;
    padding: 1px 5px;
    border-radius: var(--radius-sm);
    font-weight: var(--font-weight-semibold);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .status-badge.success {
    background: var(--color-success-subtle);
    color: var(--color-success);
  }

  .status-badge.error {
    background: var(--color-danger-subtle);
    color: var(--color-danger);
  }

  .meta-item {
    font-size: 10px;
    color: var(--color-text-muted);
  }

  .time-ago {
    margin-left: auto;
  }

  /* ── Saved Queries panel ───────────────────────────────────────────────── */

  .inline-input-row {
    display: flex;
    gap: var(--spacing-1);
    padding: var(--spacing-1) var(--spacing-2);
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
  }

  .inline-input {
    flex: 1;
    padding: 3px var(--spacing-2);
    font-size: var(--font-size-xs);
    background: var(--color-bg-primary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--color-text-primary);
    outline: none;
  }

  .inline-input:focus {
    border-color: var(--color-accent);
  }

  .saved-list {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-1) 0;
  }

  .folder-node {
    display: flex;
    flex-direction: column;
  }

  .folder-btn,
  .query-btn {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    width: 100%;
    padding: 3px var(--spacing-2);
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
    text-align: left;
    cursor: pointer;
    border-radius: var(--radius-sm);
    transition: background var(--transition-fast);
    user-select: none;
  }

  .folder-btn:hover,
  .query-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .folder-label {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: 3px var(--spacing-2);
    font-size: var(--font-size-xs);
  }

  .folder-icon,
  .query-icon {
    font-size: var(--font-size-xs);
    flex-shrink: 0;
  }

  .folder-name,
  .query-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .folder-name {
    font-weight: var(--font-weight-medium);
  }

  .count-badge {
    font-size: 10px;
    color: var(--color-text-muted);
    background: var(--color-bg-tertiary);
    border-radius: var(--radius-sm);
    padding: 0 4px;
  }

  .chevron {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    width: 10px;
    display: inline-block;
    transition: transform var(--transition-fast);
    flex-shrink: 0;
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .folder-children {
    padding-left: var(--spacing-3);
  }

  .query-node {
    display: flex;
  }

  /* ── Context menu ──────────────────────────────────────────────────────── */

  .ctx-menu {
    position: fixed;
    z-index: 1000;
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-md);
    padding: var(--spacing-1) 0;
    min-width: 140px;
  }

  .ctx-item {
    display: block;
    width: 100%;
    padding: var(--spacing-1) var(--spacing-3);
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    text-align: left;
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .ctx-item:hover {
    background: var(--color-bg-hover);
  }

  .ctx-item.danger {
    color: var(--color-danger);
  }

  .ctx-item.danger:hover {
    background: var(--color-danger-subtle);
  }

  .placeholder-panel {
    padding: var(--spacing-3) var(--spacing-3);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
    line-height: var(--line-height-normal);
  }

  .relations-tabpanel {
    flex: 1;
    overflow: hidden;
  }
</style>
