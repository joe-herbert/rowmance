<!--
  SchemaTree — shows the database schema for all active connections.
  Databases expand to reveal tables/views.
  Right-click on a table shows a context menu (Open, View DDL, Copy Name).
  A fuzzy-search box filters tables across all loaded databases.
-->
<script lang="ts">
  import { useConnections } from '$lib/stores/connections.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import DbIcon from '$lib/components/icons/DbIcon.svelte';
  import TableIcon from '$lib/components/icons/TableIcon.svelte';
  import { isSystemDatabase, isSystemTable } from '$lib/utils/system-items';
  import * as schemaApi from '$lib/tauri/schema';
  import type { TableInfo } from '$lib/types';
  import { errorMessage } from '$lib/utils/errors';
  import Fuse from 'fuse.js';
  import { listen } from '@tauri-apps/api/event';
  import ContextMenu from '$lib/components/ui/ContextMenu.svelte';
  import CtxItem from '$lib/components/ui/CtxItem.svelte';
  import CtxSep from '$lib/components/ui/CtxSep.svelte';

  const connectionStore = useConnections();
  const panelStore = usePanels();
  const settingsStore = useSettings();

  // ── Schema state ──────────────────────────────────────────────────────────────

  // Schema cache: connectionId → database → tables
  let schemaCache = $state<Map<string, Map<string, TableInfo[]>>>(new Map());

  $effect(() => {
    let unlisten: (() => void) | null = null;
    listen<{ connectionId: string; database: string; tableName: string; count: number }>(
      'table-count-updated',
      (event) => {
        const { connectionId, database, tableName, count } = event.payload;
        const connMap = schemaCache.get(connectionId);
        const tables = connMap?.get(database);
        if (!tables) return;
        const updated = tables.map((t) => (t.name === tableName ? { ...t, rowCount: count } : t));
        const newConnMap = new Map(connMap);
        newConnMap.set(database, updated);
        schemaCache = new Map([...schemaCache, [connectionId, newConnMap]]);
      },
    ).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  });
  let expandedConnections = $state<Set<string>>(new Set());
  let expandedDatabases = $state<Set<string>>(new Set()); // key: `${connectionId}/${database}`
  let loadingKeys = $state<Set<string>>(new Set());
  // Error state: key → error message (key is connectionId or `${connectionId}/${database}`)
  let loadErrors = $state<Map<string, string>>(new Map());

  // ── Context menu state ────────────────────────────────────────────────────────

  interface ContextMenu {
    x: number;
    y: number;
    connectionId: string;
    database: string;
    table: TableInfo;
  }

  interface DbContextMenu {
    x: number;
    y: number;
    connectionId: string;
    database: string;
  }

  let contextMenu = $state<ContextMenu | null>(null);
  let dbContextMenu = $state<DbContextMenu | null>(null);

  // ── Search state ──────────────────────────────────────────────────────────────

  let searchQuery = $state('');

  interface SearchItem {
    connectionId: string;
    database: string;
    name: string;
    label: string;
  }

  // Exported for testing.
  export function buildSearchItems(
    profiles: typeof connectionStore.profiles,
    cache: Map<string, Map<string, TableInfo[]>>,
  ): SearchItem[] {
    const items: SearchItem[] = [];
    for (const profile of profiles) {
      const dbMap = cache.get(profile.id);
      if (!dbMap) continue;
      for (const [database, tables] of dbMap.entries()) {
        for (const table of tables) {
          items.push({
            connectionId: profile.id,
            database,
            name: table.name,
            label: `${profile.name}.${database}.${table.name}`,
          });
        }
      }
    }
    return items;
  }

  const searchResults = $derived.by<SearchItem[]>(() => {
    if (!searchQuery.trim()) return [];
    const items = buildSearchItems(connectionStore.profiles, schemaCache);
    if (items.length === 0) return [];
    const fuse = new Fuse(items, { keys: ['name', 'label'], threshold: 0.4 });
    return fuse.search(searchQuery).map((r) => r.item);
  });

  const isSearching = $derived(searchQuery.trim().length > 0);

  // ── Tree helpers ──────────────────────────────────────────────────────────────

  async function toggleConnection(connectionId: string) {
    if (expandedConnections.has(connectionId)) {
      expandedConnections = new Set([...expandedConnections].filter((id) => id !== connectionId));
    } else {
      expandedConnections = new Set([...expandedConnections, connectionId]);
      await loadDatabases(connectionId);
    }
  }

  async function loadDatabases(connectionId: string) {
    if (schemaCache.has(connectionId)) return;
    loadingKeys = new Set([...loadingKeys, connectionId]);
    loadErrors = new Map([...loadErrors].filter(([k]) => k !== connectionId));

    try {
      const databases = await schemaApi.listDatabases(connectionId);
      const dbMap = new Map<string, TableInfo[]>();
      databases.forEach((db) => dbMap.set(db, []));
      schemaCache = new Map([...schemaCache, [connectionId, dbMap]]);
    } catch (err) {
      loadErrors = new Map([...loadErrors, [connectionId, errorMessage(err)]]);
    } finally {
      loadingKeys = new Set([...loadingKeys].filter((k) => k !== connectionId));
    }
  }

  async function toggleDatabase(connectionId: string, database: string) {
    const key = `${connectionId}/${database}`;
    if (expandedDatabases.has(key)) {
      expandedDatabases = new Set([...expandedDatabases].filter((k) => k !== key));
    } else {
      expandedDatabases = new Set([...expandedDatabases, key]);
      await loadTables(connectionId, database);
    }
  }

  async function loadTables(connectionId: string, database: string) {
    const key = `${connectionId}/${database}`;
    const existing = schemaCache.get(connectionId)?.get(database);
    if (existing && existing.length > 0) return;

    loadingKeys = new Set([...loadingKeys, key]);
    loadErrors = new Map([...loadErrors].filter(([k]) => k !== key));

    try {
      const tables = await schemaApi.listTables(connectionId, database);
      const connMap = new Map(schemaCache.get(connectionId) ?? []);
      connMap.set(database, tables);
      schemaCache = new Map([...schemaCache, [connectionId, connMap]]);
    } catch (err) {
      loadErrors = new Map([...loadErrors, [key, errorMessage(err)]]);
    } finally {
      loadingKeys = new Set([...loadingKeys].filter((k) => k !== key));
    }
  }

  function openTable(connectionId: string, database: string, table: string) {
    panelStore.openInFocused({ kind: 'table_browser', connectionId, database, table });
  }

  // ── Context menu ──────────────────────────────────────────────────────────────

  function showContextMenu(
    event: MouseEvent,
    connectionId: string,
    database: string,
    table: TableInfo,
  ) {
    event.preventDefault();
    contextMenu = { x: event.clientX, y: event.clientY, connectionId, database, table };
  }

  function closeContextMenu() {
    contextMenu = null;
  }

  function ctxOpenTable() {
    if (!contextMenu) return;
    openTable(contextMenu.connectionId, contextMenu.database, contextMenu.table.name);
    closeContextMenu();
  }

  function ctxViewDdl() {
    if (!contextMenu) return;
    panelStore.openInFocused({
      kind: 'ddl_viewer',
      connectionId: contextMenu.connectionId,
      database: contextMenu.database,
      objectName: contextMenu.table.name,
      objectType: contextMenu.table.tableType,
    });
    closeContextMenu();
  }

  function ctxCopyName() {
    if (!contextMenu) return;
    navigator.clipboard.writeText(contextMenu.table.name);
    closeContextMenu();
  }

  function showDbContextMenu(event: MouseEvent, connectionId: string, database: string) {
    event.preventDefault();
    dbContextMenu = { x: event.clientX, y: event.clientY, connectionId, database };
  }

  function closeDbContextMenu() {
    dbContextMenu = null;
  }

  function ctxOpenErd() {
    if (!dbContextMenu) return;
    panelStore.openInFocused({
      kind: 'erd',
      connectionId: dbContextMenu.connectionId,
      database: dbContextMenu.database,
    });
    closeDbContextMenu();
  }

  const activeProfiles = $derived(
    connectionStore.profiles.filter((p) => connectionStore.isActive(p.id)),
  );

  // ── System item helpers ───────────────────────────────────────────────────────

  function checkSystemDatabase(db: string): boolean {
    return isSystemDatabase(db, settingsStore.settings.systemDatabases);
  }

  function checkSystemTable(name: string): boolean {
    return isSystemTable(name, settingsStore.settings.systemTablePatterns);
  }
</script>

<div class="schema-tree">
  <div class="tree-header no-select">
    <span>Schema</span>
  </div>

  <!-- Fuzzy search input -->
  <div class="search-bar">
    <input
      class="search-input"
      type="search"
      placeholder="Search tables…"
      bind:value={searchQuery}
      aria-label="Search tables"
    />
    {#if searchQuery}
      <button class="search-clear" onclick={() => (searchQuery = '')} aria-label="Clear search"
        ><svg
          width="10"
          height="10"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.5"
          stroke-linecap="round"
          aria-hidden="true"
          ><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg
        ></button
      >
    {/if}
  </div>

  {#if isSearching}
    <!-- Flat search results -->
    {#if searchResults.length === 0}
      <div class="empty-hint">No tables match "{searchQuery}".</div>
    {:else}
      <ul class="search-results" role="listbox" aria-label="Search results">
        {#each searchResults as item (item.label)}
          <li role="option" aria-selected={false}>
            <button
              class="search-result-row"
              onclick={() => openTable(item.connectionId, item.database, item.name)}
              title={item.label}
            >
              <span class="result-icon" aria-hidden="true">▦</span>
              <span class="result-name">{item.name}</span>
              <span class="result-path">{item.database}</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  {:else if activeProfiles.length === 0}
    <div class="empty-hint">Connect to a database to browse its schema.</div>
  {:else}
    <ul class="tree-root" role="tree" aria-label="Database schema">
      {#each activeProfiles as profile (profile.id)}
        {@const isExpanded = expandedConnections.has(profile.id)}
        {@const isLoading = loadingKeys.has(profile.id)}
        {@const databases = schemaCache.get(profile.id)}

        <li class="tree-node" role="treeitem" aria-expanded={isExpanded} aria-selected={false}>
          <!-- Connection node -->
          <button
            class="node-row connection-node"
            onclick={() => toggleConnection(profile.id)}
            aria-label="{isExpanded ? 'Collapse' : 'Expand'} {profile.name}"
          >
            <span class="chevron" class:open={isExpanded} aria-hidden="true">›</span>
            {#if profile.color}
              <span class="color-dot" style="background: {profile.color};" aria-hidden="true"
              ></span>
            {/if}
            <span class="node-label">{profile.name}</span>
            {#if isLoading}
              <span class="loading-indicator" aria-label="Loading">…</span>
            {/if}
          </button>

          {#if isExpanded && loadErrors.has(profile.id)}
            <div class="load-error">{loadErrors.get(profile.id)}</div>
          {/if}

          {#if isExpanded && databases}
            <ul class="tree-children" role="group">
              {#each [...databases.keys()].filter((db) => settingsStore.settings.showSystemItems || !checkSystemDatabase(db)) as database}
                {@const dbKey = `${profile.id}/${database}`}
                {@const isDbExpanded = expandedDatabases.has(dbKey)}
                {@const isDbLoading = loadingKeys.has(dbKey)}
                {@const tables = databases.get(database) ?? []}
                {@const isDbSystem = checkSystemDatabase(database)}

                <li
                  class="tree-node"
                  role="treeitem"
                  aria-expanded={isDbExpanded}
                  aria-selected={false}
                >
                  <!-- Database node -->
                  <button
                    class="node-row database-node"
                    class:system={isDbSystem}
                    onclick={() => toggleDatabase(profile.id, database)}
                    oncontextmenu={(e) => showDbContextMenu(e, profile.id, database)}
                    aria-label="{isDbExpanded ? 'Collapse' : 'Expand'} database {database}"
                  >
                    <span class="chevron" class:open={isDbExpanded} aria-hidden="true">›</span>
                    <DbIcon system={isDbSystem} size={14} aria-hidden="true" />
                    <span class="node-label">{database}</span>
                    {#if isDbLoading}
                      <span class="loading-indicator" aria-label="Loading">…</span>
                    {/if}
                  </button>

                  {#if isDbExpanded && loadErrors.has(dbKey)}
                    <div class="load-error db-load-error">{loadErrors.get(dbKey)}</div>
                  {/if}

                  {#if isDbExpanded && tables.length > 0}
                    <ul class="tree-children" role="group">
                      {#each tables.filter((t) => settingsStore.settings.showSystemItems || !(isDbSystem || checkSystemTable(t.name))) as table}
                        {@const isTableSystem = isDbSystem || checkSystemTable(table.name)}
                        <li class="tree-node leaf-node" role="treeitem" aria-selected={false}>
                          <button
                            class="node-row table-node"
                            class:system={isTableSystem}
                            ondblclick={() => openTable(profile.id, database, table.name)}
                            onclick={() => openTable(profile.id, database, table.name)}
                            oncontextmenu={(e) => showContextMenu(e, profile.id, database, table)}
                            title="Open {table.name}"
                            aria-label="Open table {table.name}"
                          >
                            {#if table.tableType === 'view'}
                              <span class="node-icon" aria-hidden="true">◫</span>
                            {:else}
                              <TableIcon system={isTableSystem} aria-hidden="true" />
                            {/if}
                            <span class="node-label">{table.name}</span>
                            {#if table.rowCount !== null}
                              <span class="row-count">{table.rowCount.toLocaleString()}</span>
                            {/if}
                          </button>
                        </li>
                      {/each}
                    </ul>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<ContextMenu
  x={contextMenu?.x ?? 0}
  y={contextMenu?.y ?? 0}
  open={contextMenu !== null}
  onclose={closeContextMenu}
  minWidth={140}
>
  <CtxItem onclick={ctxOpenTable}>Open Table</CtxItem>
  <CtxItem onclick={ctxViewDdl}>View DDL</CtxItem>
  <CtxItem onclick={ctxCopyName}>Copy Name</CtxItem>
</ContextMenu>

<ContextMenu
  x={dbContextMenu?.x ?? 0}
  y={dbContextMenu?.y ?? 0}
  open={dbContextMenu !== null}
  onclose={closeDbContextMenu}
  minWidth={140}
>
  <CtxItem onclick={ctxOpenErd}>Open ERD</CtxItem>
  <CtxSep />
  <CtxItem
    onclick={() => {
      settingsStore.set('showSystemItems', !settingsStore.settings.showSystemItems);
      closeDbContextMenu();
    }}
  >
    {settingsStore.settings.showSystemItems ? 'Hide System Items' : 'Show System Items'}
  </CtxItem>
</ContextMenu>

<style>
  .schema-tree {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .tree-header {
    padding: var(--spacing-1) var(--spacing-3);
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    flex-shrink: 0;
  }

  /* ── Search ── */

  .search-bar {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: var(--spacing-1) var(--spacing-2);
    flex-shrink: 0;
  }

  .search-input {
    flex: 1;
    height: 26px;
    padding: 0 var(--spacing-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-primary);
    color: var(--color-text-primary);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    outline: none;
    transition: border-color var(--transition-fast);
    min-width: 0;
  }

  .search-input:focus {
    border-color: var(--color-accent);
  }

  /* Remove the default search × button in webkit */
  .search-input::-webkit-search-cancel-button {
    display: none;
  }

  .search-clear {
    flex-shrink: 0;
    width: 18px;
    height: 18px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 9px;
    color: var(--color-text-muted);
    border-radius: var(--radius-sm);
    transition:
      color var(--transition-fast),
      background var(--transition-fast);
  }

  .search-clear:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  /* ── Search results ── */

  .search-results {
    padding: 0;
    flex: 1;
    overflow-y: auto;
  }

  .search-result-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    width: 100%;
    padding: 3px var(--spacing-2);
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
    text-align: left;
    border-radius: var(--radius-sm);
    transition: background var(--transition-fast);
    -webkit-user-select: none;
    user-select: none;
    cursor: pointer;
  }

  .search-result-row:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .result-icon {
    flex-shrink: 0;
    font-size: var(--font-size-xs);
  }

  .result-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .result-path {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    flex-shrink: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 80px;
  }

  /* ── Empty hint ── */

  .empty-hint {
    padding: var(--spacing-2) var(--spacing-3);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
    line-height: var(--line-height-normal);
  }

  .load-error {
    padding: var(--spacing-1) var(--spacing-3);
    font-size: var(--font-size-xs);
    color: var(--color-error, #e05c5c);
    line-height: var(--line-height-normal);
    word-break: break-word;
  }

  .db-load-error {
    padding-left: calc(var(--spacing-3) * 2);
  }

  /* ── Tree ── */

  .tree-root,
  .tree-children {
    padding: 0;
  }

  .tree-node {
    display: flex;
    flex-direction: column;
  }

  .node-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    width: 100%;
    padding: 3px var(--spacing-2);
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
    text-align: left;
    border-radius: var(--radius-sm);
    transition: background var(--transition-fast);
    -webkit-user-select: none;
    user-select: none;
    cursor: pointer;
  }

  .node-row:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .chevron {
    flex-shrink: 0;
    font-size: var(--font-size-xs);
    transition: transform var(--transition-fast);
    color: var(--color-text-muted);
    display: inline-block;
    width: 10px;
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .color-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .node-icon {
    flex-shrink: 0;
    font-size: var(--font-size-xs);
  }

  .node-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .loading-indicator {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    animation: pulse 1s infinite;
  }

  .row-count {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .node-row.system {
    opacity: 0.5;
  }

  .connection-node {
    font-weight: var(--font-weight-medium);
  }

  .database-node {
    padding-left: var(--spacing-3);
  }

  .table-node {
    padding-left: calc(var(--spacing-3) * 2);
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.4;
    }
  }
</style>
