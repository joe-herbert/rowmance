<!--
  SchemaTree — shows the database schema for all active connections.
  Databases expand to reveal tables, which expand to reveal columns/indexes/keys.
  Clicking a table opens it as a table browser in the focused panel.
  Phase 1: static tree with no fuzzy search (added in Phase 2).
-->
<script lang="ts">
  import { useConnections } from '$lib/stores/connections.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import * as schemaApi from '$lib/tauri/schema';
  import type { TableInfo } from '$lib/types';

  const connectionStore = useConnections();
  const panelStore = usePanels();

  // Schema cache: connectionId → database → tables
  let schemaCache = $state<Map<string, Map<string, TableInfo[]>>>(new Map());
  let expandedConnections = $state<Set<string>>(new Set());
  let expandedDatabases = $state<Set<string>>(new Set()); // key: `${connectionId}/${database}`
  let loadingKeys = $state<Set<string>>(new Set());

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

    try {
      const databases = await schemaApi.listDatabases(connectionId);
      const dbMap = new Map<string, TableInfo[]>();
      databases.forEach((db) => dbMap.set(db, []));
      schemaCache = new Map([...schemaCache, [connectionId, dbMap]]);
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
    try {
      const tables = await schemaApi.listTables(connectionId, database);
      const connMap = new Map(schemaCache.get(connectionId) ?? []);
      connMap.set(database, tables);
      schemaCache = new Map([...schemaCache, [connectionId, connMap]]);
    } finally {
      loadingKeys = new Set([...loadingKeys].filter((k) => k !== key));
    }
  }

  function openTable(connectionId: string, database: string, table: string) {
    panelStore.openInFocused({ kind: 'table_browser', connectionId, database, table });
  }

  const activeProfiles = $derived(
    connectionStore.profiles.filter((p) => connectionStore.isActive(p.id)),
  );
</script>

<div class="schema-tree">
  <div class="tree-header no-select">
    <span>Schema</span>
  </div>

  {#if activeProfiles.length === 0}
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

          {#if isExpanded && databases}
            <ul class="tree-children" role="group">
              {#each [...databases.keys()] as database}
                {@const dbKey = `${profile.id}/${database}`}
                {@const isDbExpanded = expandedDatabases.has(dbKey)}
                {@const isDbLoading = loadingKeys.has(dbKey)}
                {@const tables = databases.get(database) ?? []}

                <li
                  class="tree-node"
                  role="treeitem"
                  aria-expanded={isDbExpanded}
                  aria-selected={false}
                >
                  <!-- Database node -->
                  <button
                    class="node-row database-node"
                    onclick={() => toggleDatabase(profile.id, database)}
                    aria-label="{isDbExpanded ? 'Collapse' : 'Expand'} database {database}"
                  >
                    <span class="chevron" class:open={isDbExpanded} aria-hidden="true">›</span>
                    <span class="node-icon" aria-hidden="true">🗃</span>
                    <span class="node-label">{database}</span>
                    {#if isDbLoading}
                      <span class="loading-indicator" aria-label="Loading">…</span>
                    {/if}
                  </button>

                  {#if isDbExpanded && tables.length > 0}
                    <ul class="tree-children" role="group">
                      {#each tables as table}
                        <li class="tree-node leaf-node" role="treeitem" aria-selected={false}>
                          <button
                            class="node-row table-node"
                            ondblclick={() => openTable(profile.id, database, table.name)}
                            onclick={() => openTable(profile.id, database, table.name)}
                            title="Open {table.name}"
                            aria-label="Open table {table.name}"
                          >
                            <span class="node-icon" aria-hidden="true">
                              {table.tableType === 'view' ? '◫' : '▦'}
                            </span>
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

  .empty-hint {
    padding: var(--spacing-2) var(--spacing-3);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
    line-height: var(--line-height-normal);
  }

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
