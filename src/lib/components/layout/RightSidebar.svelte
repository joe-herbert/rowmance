<!--
  RightSidebar — toggleable right panel with icon tabs.
  Only one panel is active at a time; clicking the active tab collapses the sidebar.
  Panels: Query History, Saved Queries.
-->
<script lang="ts">
  import { useConnections } from '$lib/stores/connections.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useCellSelection } from '$lib/stores/cellSelection.svelte';
  import * as historyApi from '$lib/tauri/history';
  import * as savedQueriesApi from '$lib/tauri/saved_queries';
  import * as schemaApi from '$lib/tauri/schema';
  import type { QueryHistoryEntry, SavedQuery, SavedQueryFolder, ColumnInfo, IndexInfo, ForeignKeyInfo } from '$lib/types';
  import { errorMessage } from '$lib/utils/errors';
  import RelationsPanel from '$lib/components/relations/RelationsPanel.svelte';
  import Select from '$lib/components/ui/Select.svelte';

  type ActivePanel = 'history' | 'saved' | 'column' | 'table-info' | 'relations' | null;

  interface Props {
    onClose: () => void;
    initialPanel?: ActivePanel;
    onPanelChange?: (panel: ActivePanel) => void;
  }

  const { onClose, initialPanel = 'history', onPanelChange }: Props = $props();

  const connectionStore = useConnections();
  const panelStore = usePanels();
  const cellSelectionStore = useCellSelection();

  let activePanel = $state<ActivePanel>(initialPanel);

  function selectPanel(panel: ActivePanel) {
    if (activePanel === panel) {
      onClose();
    } else {
      activePanel = panel;
      onPanelChange?.(panel);
      if (panel === 'history') loadHistory();
      if (panel === 'saved') loadSavedQueries();
      if (panel === 'column') columnInspectorKey = null;
      if (panel === 'table-info') tableInfoKey = null;
    }
  }

  // ── Relations panel ───────────────────────────────────────────────────────

  // ── Column Inspector ─────────────────────────────────────────────────────────

  let columnInfoLoading = $state(false);
  let columnInfoError = $state<string | null>(null);
  let columnInfoData = $state<ColumnInfo | null>(null);
  let columnIndexes = $state<IndexInfo[]>([]);
  let columnForeignKeys = $state<ForeignKeyInfo[]>([]);
  let columnInspectorKey = $state<string | null>(null);

  $effect(() => {
    const sel = cellSelectionStore.current;
    if (activePanel !== 'column') return;
    const key = sel ? `${sel.connectionId}:${sel.database}:${sel.table}:${sel.columnName}` : null;
    if (key === columnInspectorKey) return;
    columnInspectorKey = key;

    if (!sel) {
      columnInfoData = null;
      columnIndexes = [];
      columnForeignKeys = [];
      return;
    }

    columnInfoLoading = true;
    columnInfoError = null;
    Promise.all([
      schemaApi.listColumns(sel.connectionId, sel.database, sel.table),
      schemaApi.listIndexes(sel.connectionId, sel.database, sel.table),
      schemaApi.listForeignKeys(sel.connectionId, sel.database, sel.table),
    ]).then(([cols, idxs, fks]) => {
      columnInfoData = cols.find((c) => c.name === sel.columnName) ?? null;
      columnIndexes = idxs.filter((idx) => idx.columns.includes(sel.columnName));
      columnForeignKeys = fks.filter((fk) => fk.columns.includes(sel.columnName));
      columnInfoLoading = false;
    }).catch((err) => {
      columnInfoError = errorMessage(err);
      columnInfoLoading = false;
    });
  });

  // ── Table Info ───────────────────────────────────────────────────────────────

  let tableInfoLoading = $state(false);
  let tableInfoError = $state<string | null>(null);
  let tableColumns = $state<ColumnInfo[]>([]);
  let tableIndexes = $state<IndexInfo[]>([]);
  let tableForeignKeys = $state<ForeignKeyInfo[]>([]);
  let tableInfoKey = $state<string | null>(null);

  $effect(() => {
    const sel = cellSelectionStore.current;
    if (activePanel !== 'table-info') return;
    const key = sel ? `${sel.connectionId}:${sel.database}:${sel.table}` : null;
    if (key === tableInfoKey) return;
    tableInfoKey = key;

    if (!sel) {
      tableColumns = [];
      tableIndexes = [];
      tableForeignKeys = [];
      return;
    }

    tableInfoLoading = true;
    tableInfoError = null;
    Promise.all([
      schemaApi.listColumns(sel.connectionId, sel.database, sel.table),
      schemaApi.listIndexes(sel.connectionId, sel.database, sel.table),
      schemaApi.listForeignKeys(sel.connectionId, sel.database, sel.table),
    ]).then(([cols, idxs, fks]) => {
      tableColumns = cols;
      tableIndexes = idxs;
      tableForeignKeys = fks;
      tableInfoLoading = false;
    }).catch((err) => {
      tableInfoError = errorMessage(err);
      tableInfoLoading = false;
    });
  });

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
  const queriesByFolder = $derived.by<Map<string | null, SavedQuery[]>>(() => {
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
            <Select
              bind:value={selectedHistoryConnectionId}
              options={activeProfiles.map(p => ({ value: p.id, label: p.name }))}
              aria-label="Select connection"
              size="xs"
            />
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
            {#if (queriesByFolder.get(null) ?? []).length > 0}
              <li class="folder-node" role="treeitem" aria-selected={false}>
                <span class="folder-label muted">Unfiled</span>
                <ul class="folder-children" role="group">
                  {#each queriesByFolder.get(null) ?? [] as query (query.id)}
                    <li class="query-node" role="treeitem" aria-selected={false}>
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
              {@const folderQueries = queriesByFolder.get(folder.id) ?? []}
              {@const isOpen = expandedFolders.has(folder.id)}
              <li class="folder-node" role="treeitem" aria-expanded={isOpen} aria-selected={false}>
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
                      <li class="query-node" role="treeitem" aria-selected={false}>
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
        {#if !cellSelectionStore.current}
          <div class="placeholder-panel"><p>Select a cell in a table to inspect its column.</p></div>
        {:else if columnInfoLoading}
          <div class="loading-row">Loading…</div>
        {:else if columnInfoError}
          <div class="error-row">{columnInfoError}</div>
        {:else if !columnInfoData}
          <div class="placeholder-panel"><p>Column not found.</p></div>
        {:else}
          {@const sel = cellSelectionStore.current}
          <div class="context-bar">
            <span class="ctx-table">{sel!.table}</span><span class="ctx-dot">.</span><span class="ctx-col">{columnInfoData.name}</span>
          </div>
          <div class="info-section">
            <div class="info-section-title">Properties</div>
            <dl class="info-dl">
              <div class="info-row"><dt>Type</dt><dd class="mono">{columnInfoData.dataType}</dd></div>
              <div class="info-row"><dt>Nullable</dt><dd>{columnInfoData.nullable ? 'Yes' : 'No'}</dd></div>
              {#if columnInfoData.defaultValue !== null}
                <div class="info-row"><dt>Default</dt><dd class="mono">{columnInfoData.defaultValue}</dd></div>
              {/if}
              {#if columnInfoData.isPrimaryKey}
                <div class="info-row"><dt>Primary Key</dt><dd class="badge-pk">PK</dd></div>
              {/if}
              {#if columnInfoData.isAutoIncrement}
                <div class="info-row"><dt>Auto Increment</dt><dd>Yes</dd></div>
              {/if}
              {#if columnInfoData.isForeignKey}
                <div class="info-row"><dt>Foreign Key</dt><dd>Yes</dd></div>
              {/if}
              {#if columnInfoData.comment}
                <div class="info-row"><dt>Comment</dt><dd>{columnInfoData.comment}</dd></div>
              {/if}
            </dl>
          </div>
          {#if columnIndexes.length > 0}
            <div class="info-section">
              <div class="info-section-title">Indexes</div>
              {#each columnIndexes as idx (idx.name)}
                <div class="tag-row">
                  <span class="tag-name mono">{idx.name}</span>
                  {#if idx.unique}<span class="tag-badge">UNIQUE</span>{/if}
                  <span class="tag-type">{idx.indexType}</span>
                </div>
              {/each}
            </div>
          {/if}
          {#if columnForeignKeys.length > 0}
            <div class="info-section">
              <div class="info-section-title">Foreign Keys</div>
              {#each columnForeignKeys as fk (fk.constraintName)}
                <div class="fk-card">
                  <div class="fk-name mono">{fk.constraintName}</div>
                  <div class="fk-ref">→ <span class="mono">{fk.referencedTable}.{fk.referencedColumns.join(', ')}</span></div>
                  <div class="fk-actions">ON DELETE {fk.onDelete} · ON UPDATE {fk.onUpdate}</div>
                </div>
              {/each}
            </div>
          {/if}
        {/if}
      </div>

    {:else if activePanel === 'table-info'}
      <div id="panel-table-info" role="tabpanel" aria-label="Table Info">
        <div class="panel-toolbar">
          <span class="panel-title">Table Info</span>
        </div>
        {#if !cellSelectionStore.current}
          <div class="placeholder-panel"><p>Select a cell in a table to see its details.</p></div>
        {:else if tableInfoLoading}
          <div class="loading-row">Loading…</div>
        {:else if tableInfoError}
          <div class="error-row">{tableInfoError}</div>
        {:else}
          {@const sel = cellSelectionStore.current}
          <div class="context-bar">
            <span class="ctx-table">{sel!.table}</span>
            <span class="ctx-sub">{sel!.database}</span>
          </div>
          {#if tableColumns.length > 0}
            <div class="info-section">
              <div class="info-section-title">Columns ({tableColumns.length})</div>
              <table class="col-table">
                <thead>
                  <tr>
                    <th>Name</th>
                    <th>Type</th>
                    <th title="Nullable">N</th>
                    <th title="Primary Key">PK</th>
                  </tr>
                </thead>
                <tbody>
                  {#each tableColumns as col (col.name)}
                    <tr class:selected-col={col.name === sel!.columnName}>
                      <td class="mono">{col.name}</td>
                      <td class="mono type-cell">{col.dataType}</td>
                      <td class="center-cell">{col.nullable ? '✓' : ''}</td>
                      <td class="center-cell">{col.isPrimaryKey ? '✓' : ''}</td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {/if}
          {#if tableIndexes.length > 0}
            <div class="info-section">
              <div class="info-section-title">Indexes ({tableIndexes.length})</div>
              {#each tableIndexes as idx (idx.name)}
                <div class="tag-row">
                  <span class="tag-name mono">{idx.name}</span>
                  {#if idx.unique}<span class="tag-badge">UNIQUE</span>{/if}
                  <span class="tag-type">{idx.columns.join(', ')}</span>
                </div>
              {/each}
            </div>
          {/if}
          {#if tableForeignKeys.length > 0}
            <div class="info-section">
              <div class="info-section-title">Foreign Keys ({tableForeignKeys.length})</div>
              {#each tableForeignKeys as fk (fk.constraintName)}
                <div class="fk-card">
                  <div class="fk-name mono">{fk.constraintName}</div>
                  <div class="fk-ref"><span class="mono">{fk.columns.join(', ')}</span> → <span class="mono">{fk.referencedTable}.{fk.referencedColumns.join(', ')}</span></div>
                  <div class="fk-actions">ON DELETE {fk.onDelete} · ON UPDATE {fk.onUpdate}</div>
                </div>
              {/each}
            </div>
          {/if}
        {/if}
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

  /* ── Column Inspector / Table Info shared ────────────────────────────────── */

  .context-bar {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 1px;
    padding: var(--spacing-2);
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .ctx-table {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-secondary);
    font-weight: var(--font-weight-medium);
  }

  .ctx-dot {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  .ctx-col {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-accent);
    font-weight: var(--font-weight-medium);
  }

  .ctx-sub {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    margin-left: var(--spacing-1);
  }

  .info-section {
    border-bottom: 1px solid var(--color-border);
    padding: var(--spacing-2) 0;
  }

  .info-section-title {
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.07em;
    padding: 0 var(--spacing-2) var(--spacing-1);
  }

  .info-dl {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 0 var(--spacing-2);
  }

  .info-row {
    display: flex;
    align-items: baseline;
    gap: var(--spacing-2);
  }

  .info-row dt {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    min-width: 90px;
    flex-shrink: 0;
  }

  .info-row dd {
    font-size: var(--font-size-xs);
    color: var(--color-text-primary);
  }

  .mono {
    font-family: var(--font-family-mono);
  }

  .badge-pk {
    font-size: 10px;
    background: var(--color-accent-subtle);
    color: var(--color-accent);
    border-radius: var(--radius-sm);
    padding: 1px 5px;
    font-weight: var(--font-weight-semibold);
  }

  .tag-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: 2px var(--spacing-2);
    font-size: var(--font-size-xs);
  }

  .tag-name {
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 120px;
  }

  .tag-badge {
    font-size: 10px;
    background: var(--color-success-subtle);
    color: var(--color-success);
    border-radius: var(--radius-sm);
    padding: 1px 4px;
    font-weight: var(--font-weight-semibold);
    flex-shrink: 0;
  }

  .tag-type {
    font-size: 10px;
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .fk-card {
    margin: 0 var(--spacing-2) var(--spacing-1);
    padding: var(--spacing-1) var(--spacing-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-secondary);
  }

  .fk-name {
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
    font-weight: var(--font-weight-medium);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .fk-ref {
    font-size: var(--font-size-xs);
    color: var(--color-text-primary);
    margin-top: 1px;
  }

  .fk-actions {
    font-size: 10px;
    color: var(--color-text-muted);
    margin-top: 1px;
  }

  .col-table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--font-size-xs);
  }

  .col-table th {
    padding: 3px var(--spacing-2);
    background: var(--color-table-header-bg);
    color: var(--color-text-muted);
    font-weight: var(--font-weight-medium);
    font-size: 10px;
    text-align: left;
    border-bottom: 1px solid var(--color-border);
  }

  .col-table td {
    padding: 3px var(--spacing-2);
    border-bottom: 1px solid var(--color-border);
    color: var(--color-text-secondary);
    vertical-align: middle;
  }

  .col-table tr.selected-col td {
    background: var(--color-accent-subtle);
    color: var(--color-text-primary);
  }

  .type-cell {
    color: var(--color-text-muted);
    max-width: 70px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .center-cell {
    text-align: center;
    color: var(--color-accent);
  }

  .relations-tabpanel {
    flex: 1;
    overflow: hidden;
  }
</style>
