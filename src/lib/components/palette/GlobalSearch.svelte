<!--
  GlobalSearch — opens from the titlebar search button.
  Searches connections, databases, tables, and columns.
  Schema data loads progressively on mount.
-->
<script lang="ts">
  import Fuse from 'fuse.js';
  import { onMount } from 'svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { focusTrap } from '$lib/utils/focus-trap';
  import { listDatabases, listTables, listColumns } from '$lib/tauri/schema';
  import type { ColumnInfo } from '$lib/types';

  interface Props {
    onclose: () => void;
  }

  const { onclose }: Props = $props();

  const connections = useConnections();
  const panels = usePanels();

  let query = $state('');
  let selectedIndex = $state(0);
  let inputEl = $state<HTMLInputElement | undefined>(undefined);
  type KindFilter = 'connection' | 'database' | 'table' | 'column' | null;
  let filterKind = $state<KindFilter>(null);

  const filterOptions: Array<{ label: string; value: KindFilter }> = [
    { label: 'All', value: null },
    { label: 'Connections', value: 'connection' },
    { label: 'Databases', value: 'database' },
    { label: 'Tables', value: 'table' },
    { label: 'Columns', value: 'column' },
  ];

  // ── Schema data ────────────────────────────────────────────────────────────

  type DbEntry = {
    connectionId: string;
    connectionName: string;
    connectionColor: string | null;
    database: string;
  };

  type TableEntry = {
    connectionId: string;
    connectionName: string;
    connectionColor: string | null;
    database: string;
    name: string;
    tableType: 'table' | 'view';
  };

  type ColumnEntry = {
    connectionId: string;
    connectionName: string;
    connectionColor: string | null;
    database: string;
    table: string;
    name: string;
    dataType: string;
    isPrimaryKey: boolean;
  };

  let databaseEntries = $state<DbEntry[]>([]);
  let tableEntries = $state<TableEntry[]>([]);
  let columnEntries = $state<ColumnEntry[]>([]);
  let loadingDatabases = $state(true);
  let loadingTables = $state(false);

  onMount(async () => {
    const profiles = connections.profiles;
    if (profiles.length === 0) {
      loadingDatabases = false;
      return;
    }

    const dbResults = await Promise.allSettled(
      profiles.map(async (profile) => {
        const dbs = await listDatabases(profile.id);
        return { profile, dbs };
      }),
    );

    const newDbs: DbEntry[] = [];
    for (const result of dbResults) {
      if (result.status !== 'fulfilled') continue;
      const { profile, dbs } = result.value;
      for (const db of dbs) {
        newDbs.push({
          connectionId: profile.id,
          connectionName: profile.name,
          connectionColor: profile.color,
          database: db,
        });
      }
    }
    databaseEntries = newDbs;
    loadingDatabases = false;

    loadingTables = true;
    const tableResults = await Promise.allSettled(
      newDbs.map(async (entry) => {
        const tables = await listTables(entry.connectionId, entry.database);
        return { entry, tables };
      }),
    );

    const newTables: TableEntry[] = [];
    for (const result of tableResults) {
      if (result.status !== 'fulfilled') continue;
      const { entry, tables } = result.value;
      for (const t of tables) {
        newTables.push({
          connectionId: entry.connectionId,
          connectionName: entry.connectionName,
          connectionColor: entry.connectionColor,
          database: entry.database,
          name: t.name,
          tableType: t.tableType,
        });
      }
    }
    tableEntries = newTables;
    loadingTables = false;

    await Promise.allSettled(
      newTables.map(async (entry) => {
        try {
          const cols = await listColumns(entry.connectionId, entry.database, entry.name);
          const newCols: ColumnEntry[] = cols.map((c: ColumnInfo) => ({
            connectionId: entry.connectionId,
            connectionName: entry.connectionName,
            connectionColor: entry.connectionColor,
            database: entry.database,
            table: entry.name,
            name: c.name,
            dataType: c.dataType,
            isPrimaryKey: c.isPrimaryKey,
          }));
          columnEntries = [...columnEntries, ...newCols];
        } catch {
          // Silently ignore
        }
      }),
    );
  });

  // ── Result types ───────────────────────────────────────────────────────────

  type ResultItem =
    | { kind: 'connection'; id: string; name: string; dbType: string; color: string | null }
    | (DbEntry & { kind: 'database' })
    | (TableEntry & { kind: 'table' })
    | (ColumnEntry & { kind: 'column' });

  // ── Fuse instances ─────────────────────────────────────────────────────────

  const fuseConnections = $derived(
    new Fuse(
      connections.profiles.map((p) => ({
        kind: 'connection' as const,
        id: p.id,
        name: p.name,
        dbType: p.dbType,
        color: p.color,
      })),
      { keys: ['name', 'dbType'], threshold: 0.4, includeScore: true },
    ),
  );

  const fuseDatabases = $derived(
    new Fuse(databaseEntries, {
      keys: ['database', 'connectionName'],
      threshold: 0.4,
      includeScore: true,
    }),
  );

  const fuseTables = $derived(
    new Fuse(tableEntries, {
      keys: ['name', 'database', 'connectionName'],
      threshold: 0.4,
      includeScore: true,
    }),
  );

  const fuseColumns = $derived(
    new Fuse(columnEntries, {
      keys: ['name', 'dataType', 'table'],
      threshold: 0.4,
      includeScore: true,
    }),
  );

  // ── Filtered results ───────────────────────────────────────────────────────

  const filtered = $derived.by<ResultItem[]>(() => {
    const q = query.trim();
    const fk = filterKind;

    if (!q) {
      const all: ResultItem[] = connections.profiles.slice(0, 5).map((p) => ({
        kind: 'connection' as const,
        id: p.id,
        name: p.name,
        dbType: p.dbType,
        color: p.color,
      }));
      return fk ? all.filter((r) => r.kind === fk) : all;
    }

    const results: ResultItem[] = [];
    if (!fk || fk === 'connection') for (const r of fuseConnections.search(q).slice(0, 3)) results.push(r.item);
    if (!fk || fk === 'database') for (const r of fuseDatabases.search(q).slice(0, 5)) results.push({ kind: 'database', ...r.item });
    if (!fk || fk === 'table') for (const r of fuseTables.search(q).slice(0, 8)) results.push({ kind: 'table', ...r.item });
    if (!fk || fk === 'column') for (const r of fuseColumns.search(q).slice(0, 8)) results.push({ kind: 'column', ...r.item });
    return results.slice(0, 30);
  });

  // ── Keyboard ───────────────────────────────────────────────────────────────

  function executeItem(item: ResultItem) {
    switch (item.kind) {
      case 'connection':
        panels.openInFocused({ kind: 'query_editor', connectionId: item.id });
        break;
      case 'database':
        panels.openInFocused({ kind: 'erd', connectionId: item.connectionId, database: item.database });
        break;
      case 'table':
        panels.openInFocused({
          kind: 'table_browser',
          connectionId: item.connectionId,
          database: item.database,
          table: item.name,
        });
        break;
      case 'column':
        panels.openInFocused({
          kind: 'table_structure',
          connectionId: item.connectionId,
          database: item.database,
          table: item.table,
        });
        break;
    }
    onclose();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      onclose();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      selectedIndex = (selectedIndex + 1) % Math.max(filtered.length, 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      selectedIndex = (selectedIndex - 1 + Math.max(filtered.length, 1)) % Math.max(filtered.length, 1);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const item = filtered[selectedIndex];
      if (item) executeItem(item);
    }
  }

  $effect(() => {
    query;
    filterKind;
    selectedIndex = 0;
  });

  $effect(() => {
    inputEl?.focus();
  });

  // ── Display helpers ────────────────────────────────────────────────────────

  function itemCategory(item: ResultItem): string {
    switch (item.kind) {
      case 'connection': return 'Connections';
      case 'database': return 'Databases';
      case 'table': return 'Tables';
      case 'column': return 'Columns';
    }
  }

  function itemLabel(item: ResultItem): string {
    switch (item.kind) {
      case 'connection': return item.name;
      case 'database': return item.database;
      case 'table': return item.name;
      case 'column': return item.name;
    }
  }

  function itemSubtitle(item: ResultItem): string {
    switch (item.kind) {
      case 'connection': return item.dbType;
      case 'database': return item.connectionName;
      case 'table': return `${item.connectionName} › ${item.database}`;
      case 'column': return `${item.dataType}${item.isPrimaryKey ? ' · PK' : ''} · ${item.table}`;
    }
  }

  function itemColor(item: ResultItem): string | null {
    switch (item.kind) {
      case 'connection': return item.color;
      default: return (item as DbEntry).connectionColor;
    }
  }

  interface GroupedItem { item: ResultItem; showCategory: boolean; index: number }

  const groupedItems = $derived.by<GroupedItem[]>(() => {
    let lastCat = '';
    return filtered.map((item, index) => {
      const cat = itemCategory(item);
      const showCategory = cat !== lastCat;
      lastCat = cat;
      return { item, showCategory, index };
    });
  });

  const isLoading = $derived(loadingDatabases || loadingTables);
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  class="backdrop"
  role="dialog"
  aria-modal="true"
  aria-label="Global Search"
  tabindex="-1"
  onclick={(e) => { if (e.target === e.currentTarget) onclose(); }}
>
  <div class="palette" use:focusTrap>
    <div class="search-row">
      <span class="search-icon" aria-hidden="true">
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
          <circle cx="11" cy="11" r="7"></circle>
          <line x1="21" y1="21" x2="16.5" y2="16.5"></line>
        </svg>
      </span>
      <input
        class="search-input"
        type="text"
        placeholder="Search connections, databases, tables, columns…"
        bind:value={query}
        bind:this={inputEl}
        autocomplete="off"
        spellcheck="false"
        aria-label="Global search"
        aria-autocomplete="list"
        aria-controls="global-search-list"
      />
      {#if isLoading}
        <span class="loading-indicator" aria-label="Loading schema…">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" class="spin">
            <path d="M21 12a9 9 0 1 1-6.219-8.56"></path>
          </svg>
        </span>
      {/if}
    </div>

    <div class="filter-row" role="group" aria-label="Filter by type">
      {#each filterOptions as opt}
        <button
          class="filter-chip"
          class:active={filterKind === opt.value}
          onclick={() => { filterKind = opt.value; inputEl?.focus(); }}
          tabindex="0"
          type="button"
        >{opt.label}</button>
      {/each}
    </div>

    <ul id="global-search-list" class="results-list" role="listbox">
      {#each groupedItems as { item, showCategory, index } (index)}
        {#if showCategory}
          <li class="category-label" role="presentation">
            <span class="category-icon" aria-hidden="true">
              {#if item.kind === 'connection'}
                <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
                  <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"></path>
                  <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"></path>
                </svg>
              {:else if item.kind === 'database'}
                <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
                  <ellipse cx="12" cy="5" rx="9" ry="3"></ellipse>
                  <path d="M3 5v14c0 1.66 4.03 3 9 3s9-1.34 9-3V5"></path>
                  <path d="M3 12c0 1.66 4.03 3 9 3s9-1.34 9-3"></path>
                </svg>
              {:else if item.kind === 'table'}
                <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <rect x="3" y="4" width="18" height="16" rx="2"></rect>
                  <line x1="3" y1="9.5" x2="21" y2="9.5"></line>
                  <line x1="9" y1="9.5" x2="9" y2="20"></line>
                </svg>
              {:else}
                <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
                  <line x1="3" y1="6" x2="21" y2="6"></line>
                  <line x1="3" y1="12" x2="21" y2="12"></line>
                  <line x1="3" y1="18" x2="21" y2="18"></line>
                </svg>
              {/if}
            </span>
            {itemCategory(item)}
          </li>
        {/if}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <li
          class="result-item"
          class:selected={index === selectedIndex}
          role="option"
          aria-selected={index === selectedIndex}
          onclick={() => executeItem(item)}
          onmouseenter={() => (selectedIndex = index)}
        >
          <span
            class="result-dot"
            style={itemColor(item) ? `background: ${itemColor(item)}` : undefined}
            aria-hidden="true"
          ></span>
          <span class="item-main">
            <span class="item-label">{itemLabel(item)}</span>
            {#if item.kind === 'table' && item.tableType === 'view'}
              <span class="item-badge">view</span>
            {/if}
            {#if item.kind === 'column' && item.isPrimaryKey}
              <span class="item-badge item-badge--pk">PK</span>
            {/if}
          </span>
          <span class="item-subtitle">{itemSubtitle(item)}</span>
        </li>
      {:else}
        {#if !isLoading && query.trim()}
          <li class="empty-item" role="presentation">No results</li>
        {:else if !query.trim()}
          <li class="empty-item" role="presentation">Type to search</li>
        {/if}
      {/each}
    </ul>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    -webkit-backdrop-filter: blur(4px);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 15vh;
    z-index: 500;
  }

  .palette {
    background: var(--color-bg-overlay);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-xl);
    box-shadow: var(--shadow-overlay);
    width: 560px;
    max-width: calc(100vw - var(--spacing-8));
    max-height: 480px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .search-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-3) var(--spacing-4);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .search-icon {
    color: var(--color-text-muted);
    flex-shrink: 0;
    display: flex;
    align-items: center;
  }

  .search-input {
    flex: 1;
    border: none;
    background: transparent;
    font-size: var(--font-size-md);
    color: var(--color-text-primary);
    outline: none;
    font-family: var(--font-family-ui);
  }

  .search-input::placeholder {
    color: var(--color-text-muted);
  }

  .loading-indicator {
    color: var(--color-text-disabled);
    flex-shrink: 0;
    display: flex;
    align-items: center;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .spin {
    animation: spin 0.8s linear infinite;
  }

  .filter-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: var(--spacing-2) var(--spacing-4);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .filter-chip {
    padding: 2px var(--spacing-2);
    border-radius: var(--radius-md);
    border: 1px solid var(--color-border);
    background: transparent;
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
    line-height: 1.6;
  }

  .filter-chip:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
    border-color: var(--color-border-strong);
  }

  .filter-chip.active {
    background: color-mix(in srgb, var(--color-accent) 15%, transparent);
    color: var(--color-accent);
    border-color: color-mix(in srgb, var(--color-accent) 40%, transparent);
  }

  .results-list {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-1) 0;
  }

  .category-label {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: var(--spacing-1) var(--spacing-4);
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .category-icon {
    display: flex;
    align-items: center;
    opacity: 0.7;
  }

  .result-item {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-4);
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .result-item.selected,
  .result-item:hover {
    background: var(--color-bg-hover);
  }

  .result-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
    background: var(--color-text-disabled);
  }

  .item-main {
    display: flex;
    align-items: center;
    gap: 5px;
    flex: 1;
    min-width: 0;
    overflow: hidden;
  }

  .item-label {
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .item-badge {
    font-size: 9px;
    padding: 1px 5px;
    border-radius: var(--radius-sm);
    background: var(--color-bg-tertiary, var(--color-bg-hover));
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    font-weight: var(--font-weight-semibold);
    flex-shrink: 0;
  }

  .item-badge--pk {
    background: color-mix(in srgb, var(--color-accent) 15%, transparent);
    color: var(--color-accent);
  }

  .item-subtitle {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-family: var(--font-family-mono);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 200px;
    flex-shrink: 0;
  }

  .empty-item {
    padding: var(--spacing-4);
    text-align: center;
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    font-style: italic;
  }
</style>
