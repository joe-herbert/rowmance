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
  import { useGlobalSearchCache } from '$lib/stores/globalSearchCache.svelte';
  import type { DbType } from '$lib/types';

  interface Props {
    onclose: () => void;
  }

  const { onclose }: Props = $props();

  const connections = useConnections();
  const panels = usePanels();
  const searchCache = useGlobalSearchCache();

  let query = $state('');
  let selectedIndex = $state(0);
  let inputEl = $state<HTMLInputElement | undefined>(undefined);
  type KindFilter = 'connection' | 'database' | 'table' | 'column' | null;
  type MatchMode = 'strict' | 'normal' | 'fuzzy';
  type AccessFilter = 'readonly' | 'writable' | null;
  type GroupFilter = string | 'ungrouped' | null;

  const STORAGE_KEY = 'globalSearch.filters';

  function loadSaved(): Record<string, unknown> {
    try {
      return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}');
    } catch {
      return {};
    }
  }

  const saved = loadSaved();

  let filterKind = $state<KindFilter>((saved.filterKind as KindFilter) ?? null);
  let matchMode = $state<MatchMode>((saved.matchMode as MatchMode) ?? 'normal');
  let filterDbType = $state<DbType | null>((saved.filterDbType as DbType) ?? null);
  let filterAccess = $state<AccessFilter>((saved.filterAccess as AccessFilter) ?? null);
  let filterGroup = $state<GroupFilter>((saved.filterGroup as GroupFilter) ?? null);
  let filtersExpanded = $state<boolean>((saved.filtersExpanded as boolean) ?? true);
  let excludedDbs = $state<string[]>((saved.excludedDbs as string[]) ?? []);
  let excludedTables = $state<string[]>((saved.excludedTables as string[]) ?? []);
  let excludeDbInput = $state('');
  let excludeTableInput = $state('');

  $effect(() => {
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        filterKind,
        matchMode,
        filterDbType,
        filterAccess,
        filterGroup,
        filtersExpanded,
        excludedDbs,
        excludedTables,
      }),
    );
  });

  const isFiltersActive = $derived(
    filterKind !== null ||
      matchMode !== 'normal' ||
      filterDbType !== null ||
      filterAccess !== null ||
      filterGroup !== null ||
      excludedDbs.length > 0 ||
      excludedTables.length > 0,
  );

  function clearFilters() {
    filterKind = null;
    matchMode = 'normal';
    filterDbType = null;
    filterAccess = null;
    filterGroup = null;
    excludedDbs = [];
    excludedTables = [];
    inputEl?.focus();
  }

  function matchesExcludePattern(value: string, patterns: string[]): boolean {
    const v = value.toLowerCase();
    return patterns.some((p) => {
      const pat = p.toLowerCase();
      if (pat.startsWith('*') && pat.endsWith('*') && pat.length > 2) {
        return v.includes(pat.slice(1, -1));
      } else if (pat.startsWith('*')) {
        return v.endsWith(pat.slice(1));
      } else if (pat.endsWith('*')) {
        return v.startsWith(pat.slice(0, -1));
      }
      return v === pat;
    });
  }

  function addExcludeDb() {
    const val = excludeDbInput.trim();
    if (val && !excludedDbs.includes(val)) {
      excludedDbs = [...excludedDbs, val];
    }
    excludeDbInput = '';
  }

  function addExcludeTable() {
    const val = excludeTableInput.trim();
    if (val && !excludedTables.includes(val)) {
      excludedTables = [...excludedTables, val];
    }
    excludeTableInput = '';
  }

  function removeExcludeDb(pattern: string) {
    excludedDbs = excludedDbs.filter((p) => p !== pattern);
    inputEl?.focus();
  }

  function removeExcludeTable(pattern: string) {
    excludedTables = excludedTables.filter((p) => p !== pattern);
    inputEl?.focus();
  }

  const filterOptions: Array<{ label: string; value: KindFilter }> = [
    { label: 'All', value: null },
    { label: 'Connections', value: 'connection' },
    { label: 'Databases', value: 'database' },
    { label: 'Tables', value: 'table' },
    { label: 'Columns', value: 'column' },
  ];

  const matchOptions: Array<{ label: string; value: MatchMode; title: string }> = [
    { label: 'Strict', value: 'strict', title: 'Near-exact matches only' },
    { label: 'Normal', value: 'normal', title: 'Balanced fuzzy matching' },
    { label: 'Fuzzy', value: 'fuzzy', title: 'Loose matching — more results' },
  ];

  const dbTypeLabels: Record<DbType, string> = {
    mysql: 'MySQL',
    mariadb: 'MariaDB',
    postgres: 'Postgres',
    sqlite: 'SQLite',
  };

  const availableDbTypes = $derived(
    [...new Set(connections.profiles.map((p) => p.dbType))].sort() as DbType[],
  );

  const hasAnyGroup = $derived(connections.profiles.some((p) => p.groupId !== null));

  const usedGroupIds = $derived(
    new Set(connections.profiles.map((p) => p.groupId).filter(Boolean)),
  );

  const availableGroups = $derived(
    connections.groups
      .filter((g) => usedGroupIds.has(g.id))
      .sort((a, b) => a.position - b.position),
  );

  const hasUngrouped = $derived(connections.profiles.some((p) => p.groupId === null));

  const accessOptions: Array<{ label: string; value: AccessFilter }> = [
    { label: 'All', value: null },
    { label: 'Writable', value: 'writable' },
    { label: 'Read-only', value: 'readonly' },
  ];

  const fuseOptions = $derived.by(() => {
    switch (matchMode) {
      case 'strict':
        return { threshold: 0, distance: 0, minMatchCharLength: 1, ignoreLocation: true };
      case 'fuzzy':
        return { threshold: 0.6, distance: 200, minMatchCharLength: 1, ignoreLocation: true };
      default:
        return { threshold: 0.4, distance: 100, minMatchCharLength: 1, ignoreLocation: false };
    }
  });

  // ── Schema data ────────────────────────────────────────────────────────────

  type DbEntry = import('$lib/stores/globalSearchCache.svelte').DbEntry;
  type TableEntry = import('$lib/stores/globalSearchCache.svelte').TableEntry;
  type ColumnEntry = import('$lib/stores/globalSearchCache.svelte').ColumnEntry;

  const databaseEntries = $derived(searchCache.databaseEntries);
  const tableEntries = $derived(searchCache.tableEntries);
  const columnEntries = $derived(searchCache.columnEntries);

  onMount(() => {
    const activeProfiles = connections.profiles.filter((p) => connections.activeIds.has(p.id));
    if (activeProfiles.length > 0) {
      searchCache.populate(activeProfiles);
    }
  });

  // ── Result types ───────────────────────────────────────────────────────────

  type ResultItem =
    | { kind: 'connection'; id: string; name: string; dbType: string; color: string | null }
    | (DbEntry & { kind: 'database' })
    | (TableEntry & { kind: 'table' })
    | (ColumnEntry & { kind: 'column' });

  // ── Connection-level filtering helpers ────────────────────────────────────

  function matchesConnectionFilters(
    dbType: DbType,
    readOnly: boolean,
    groupId: string | null,
  ): boolean {
    if (filterDbType && dbType !== filterDbType) return false;
    if (filterAccess === 'readonly' && !readOnly) return false;
    if (filterAccess === 'writable' && readOnly) return false;
    if (filterGroup === 'ungrouped' && groupId !== null) return false;
    if (filterGroup !== null && filterGroup !== 'ungrouped' && groupId !== filterGroup)
      return false;
    return true;
  }

  // ── Fuse instances ─────────────────────────────────────────────────────────

  const fuseConnections = $derived(
    new Fuse(
      connections.profiles
        .filter((p) => matchesConnectionFilters(p.dbType, p.readOnly, p.groupId))
        .map((p) => ({
          kind: 'connection' as const,
          id: p.id,
          name: p.name,
          dbType: p.dbType,
          color: p.color,
        })),
      { keys: ['name', 'dbType'], includeScore: true, ...fuseOptions },
    ),
  );

  const fuseDatabases = $derived(
    new Fuse(
      databaseEntries.filter(
        (e) =>
          matchesConnectionFilters(e.connectionDbType, e.connectionReadOnly, e.connectionGroupId) &&
          !matchesExcludePattern(e.database, excludedDbs),
      ),
      { keys: ['database', 'connectionName'], includeScore: true, ...fuseOptions },
    ),
  );

  const fuseTables = $derived(
    new Fuse(
      tableEntries.filter(
        (e) =>
          matchesConnectionFilters(e.connectionDbType, e.connectionReadOnly, e.connectionGroupId) &&
          !matchesExcludePattern(e.database, excludedDbs) &&
          !matchesExcludePattern(e.name, excludedTables),
      ),
      { keys: ['name', 'database', 'connectionName'], includeScore: true, ...fuseOptions },
    ),
  );

  const fuseColumns = $derived(
    new Fuse(
      columnEntries.filter(
        (e) =>
          matchesConnectionFilters(e.connectionDbType, e.connectionReadOnly, e.connectionGroupId) &&
          !matchesExcludePattern(e.database, excludedDbs) &&
          !matchesExcludePattern(e.table, excludedTables),
      ),
      { keys: ['name', 'dataType', 'table'], includeScore: true, ...fuseOptions },
    ),
  );

  // ── Filtered results ───────────────────────────────────────────────────────

  const filtered = $derived.by<ResultItem[]>(() => {
    const q = query.trim();
    const fk = filterKind;

    if (!q) {
      const all: ResultItem[] = connections.profiles
        .filter((p) => matchesConnectionFilters(p.dbType, p.readOnly, p.groupId))
        .slice(0, 5)
        .map((p) => ({
          kind: 'connection' as const,
          id: p.id,
          name: p.name,
          dbType: p.dbType,
          color: p.color,
        }));
      return fk ? all.filter((r) => r.kind === fk) : all;
    }

    const results: ResultItem[] = [];
    if (!fk || fk === 'connection')
      for (const r of fuseConnections.search(q).slice(0, 3)) results.push(r.item);
    if (!fk || fk === 'database')
      for (const r of fuseDatabases.search(q).slice(0, 5))
        results.push({ kind: 'database', ...r.item });
    if (!fk || fk === 'table')
      for (const r of fuseTables.search(q).slice(0, 8)) results.push({ kind: 'table', ...r.item });
    if (!fk || fk === 'column')
      for (const r of fuseColumns.search(q).slice(0, 8))
        results.push({ kind: 'column', ...r.item });
    return results.slice(0, 30);
  });

  // ── Keyboard ───────────────────────────────────────────────────────────────

  function executeItem(item: ResultItem) {
    switch (item.kind) {
      case 'connection':
        panels.openInFocused({ kind: 'query_editor', connectionId: item.id });
        break;
      case 'database':
        panels.openInFocused({
          kind: 'erd',
          connectionId: item.connectionId,
          database: item.database,
        });
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
      selectedIndex =
        (selectedIndex - 1 + Math.max(filtered.length, 1)) % Math.max(filtered.length, 1);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const item = filtered[selectedIndex];
      if (item) executeItem(item);
    }
  }

  $effect(() => {
    query;
    filterKind;
    filterDbType;
    filterAccess;
    filterGroup;
    selectedIndex = 0;
  });

  $effect(() => {
    inputEl?.focus();
  });

  // ── Display helpers ────────────────────────────────────────────────────────

  function itemCategory(item: ResultItem): string {
    switch (item.kind) {
      case 'connection':
        return 'Connections';
      case 'database':
        return 'Databases';
      case 'table':
        return 'Tables';
      case 'column':
        return 'Columns';
    }
  }

  function itemLabel(item: ResultItem): string {
    switch (item.kind) {
      case 'connection':
        return item.name;
      case 'database':
        return item.database;
      case 'table':
        return item.name;
      case 'column':
        return item.name;
    }
  }

  function itemSubtitle(item: ResultItem): string {
    switch (item.kind) {
      case 'connection':
        return item.dbType;
      case 'database':
        return item.connectionName;
      case 'table':
        return `${item.connectionName} › ${item.database}`;
      case 'column':
        return `${item.dataType}${item.isPrimaryKey ? ' · PK' : ''} · ${item.table}`;
    }
  }

  function itemColor(item: ResultItem): string | null {
    switch (item.kind) {
      case 'connection':
        return item.color;
      default:
        return (item as DbEntry).connectionColor;
    }
  }

  interface GroupedItem {
    item: ResultItem;
    showCategory: boolean;
    index: number;
  }

  const groupedItems = $derived.by<GroupedItem[]>(() => {
    let lastCat = '';
    return filtered.map((item, index) => {
      const cat = itemCategory(item);
      const showCategory = cat !== lastCat;
      lastCat = cat;
      return { item, showCategory, index };
    });
  });

  const isLoading = $derived(searchCache.isLoading);
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  class="backdrop"
  role="dialog"
  aria-modal="true"
  aria-label="Global Search"
  tabindex="-1"
  onclick={(e) => {
    if (e.target === e.currentTarget) onclose();
  }}
>
  <div class="palette" use:focusTrap>
    <div class="search-row">
      <span class="search-icon" aria-hidden="true">
        <svg
          width="15"
          height="15"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
          stroke-linecap="round"
        >
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
          <svg
            width="13"
            height="13"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            class="spin"
          >
            <path d="M21 12a9 9 0 1 1-6.219-8.56"></path>
          </svg>
        </span>
      {/if}
      {#if isFiltersActive}
        <button
          class="search-action-btn"
          onclick={clearFilters}
          type="button"
          title="Clear all filters"
        >
          Clear
        </button>
      {/if}
      <button
        class="search-action-btn search-action-btn--icon"
        class:active={filtersExpanded}
        class:has-filters={isFiltersActive && !filtersExpanded}
        onclick={() => {
          filtersExpanded = !filtersExpanded;
          inputEl?.focus();
        }}
        type="button"
        title={filtersExpanded ? 'Hide filters' : 'Show filters'}
        aria-expanded={filtersExpanded}
      >
        <svg
          width="13"
          height="13"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3"></polygon>
        </svg>
      </button>
    </div>

    {#if filtersExpanded}
      <div class="filter-row">
        <div role="group" aria-label="Filter by type" class="filter-group">
          {#each filterOptions as opt}
            <button
              class="filter-chip"
              class:active={filterKind === opt.value}
              onclick={() => {
                filterKind = opt.value;
                inputEl?.focus();
              }}
              tabindex="0"
              type="button">{opt.label}</button
            >
          {/each}
        </div>
        <div role="group" aria-label="Match strictness" class="filter-group filter-group--right">
          {#each matchOptions as opt}
            <button
              class="filter-chip filter-chip--match"
              class:active={matchMode === opt.value}
              onclick={() => {
                matchMode = opt.value;
                inputEl?.focus();
              }}
              tabindex="0"
              type="button"
              title={opt.title}>{opt.label}</button
            >
          {/each}
        </div>
      </div>

      <div class="filter-row">
        <div role="group" aria-label="Filter by database type" class="filter-group">
          <span class="filter-label">DB</span>
          <button
            class="filter-chip"
            class:active={filterDbType === null}
            onclick={() => {
              filterDbType = null;
              inputEl?.focus();
            }}
            type="button">All</button
          >
          {#each availableDbTypes as dt}
            <button
              class="filter-chip"
              class:active={filterDbType === dt}
              onclick={() => {
                filterDbType = dt;
                inputEl?.focus();
              }}
              type="button">{dbTypeLabels[dt]}</button
            >
          {/each}
        </div>
        <div role="group" aria-label="Filter by access" class="filter-group filter-group--right">
          {#each accessOptions as opt}
            <button
              class="filter-chip"
              class:active={filterAccess === opt.value}
              onclick={() => {
                filterAccess = opt.value;
                inputEl?.focus();
              }}
              type="button">{opt.label}</button
            >
          {/each}
        </div>
      </div>

      {#if hasAnyGroup}
        <div class="filter-row">
          <div role="group" aria-label="Filter by folder" class="filter-group filter-group--wrap">
            <span class="filter-label">Folder</span>
            <button
              class="filter-chip"
              class:active={filterGroup === null}
              onclick={() => {
                filterGroup = null;
                inputEl?.focus();
              }}
              type="button">All</button
            >
            {#each availableGroups as group}
              <button
                class="filter-chip"
                class:active={filterGroup === group.id}
                onclick={() => {
                  filterGroup = group.id;
                  inputEl?.focus();
                }}
                type="button">{group.name}</button
              >
            {/each}
            {#if hasUngrouped}
              <button
                class="filter-chip"
                class:active={filterGroup === 'ungrouped'}
                onclick={() => {
                  filterGroup = 'ungrouped';
                  inputEl?.focus();
                }}
                type="button">Ungrouped</button
              >
            {/if}
          </div>
        </div>
      {/if}

      <div class="filter-row filter-row--exclude">
        <div class="exclude-group">
          <span class="filter-label">Exclude DBs</span>
          <div class="exclude-chips">
            {#each excludedDbs as pattern}
              <span class="exclude-chip">
                {pattern}<button
                  class="exclude-chip-remove"
                  onclick={() => removeExcludeDb(pattern)}
                  type="button"
                  aria-label="Remove {pattern}">×</button
                >
              </span>
            {/each}
            <input
              class="exclude-input"
              type="text"
              placeholder="e.g. sys, tmp_*"
              bind:value={excludeDbInput}
              onkeydown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  addExcludeDb();
                }
              }}
              onblur={addExcludeDb}
              aria-label="Add database exclusion pattern"
            />
          </div>
        </div>
        <div class="exclude-group exclude-group--right">
          <span class="filter-label">Exclude Tables</span>
          <div class="exclude-chips">
            {#each excludedTables as pattern}
              <span class="exclude-chip">
                {pattern}<button
                  class="exclude-chip-remove"
                  onclick={() => removeExcludeTable(pattern)}
                  type="button"
                  aria-label="Remove {pattern}">×</button
                >
              </span>
            {/each}
            <input
              class="exclude-input"
              type="text"
              placeholder="e.g. _*, tmp_*"
              bind:value={excludeTableInput}
              onkeydown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  addExcludeTable();
                }
              }}
              onblur={addExcludeTable}
              aria-label="Add table exclusion pattern"
            />
          </div>
        </div>
      </div>
    {/if}

    <ul id="global-search-list" class="results-list" role="listbox">
      {#each groupedItems as { item, showCategory, index } (index)}
        {#if showCategory}
          <li class="category-label" role="presentation">
            <span class="category-icon" aria-hidden="true">
              {#if item.kind === 'connection'}
                <svg
                  width="10"
                  height="10"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                >
                  <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"></path>
                  <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"></path>
                </svg>
              {:else if item.kind === 'database'}
                <svg
                  width="10"
                  height="10"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                >
                  <ellipse cx="12" cy="5" rx="9" ry="3"></ellipse>
                  <path d="M3 5v14c0 1.66 4.03 3 9 3s9-1.34 9-3V5"></path>
                  <path d="M3 12c0 1.66 4.03 3 9 3s9-1.34 9-3"></path>
                </svg>
              {:else if item.kind === 'table'}
                <svg
                  width="10"
                  height="10"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                >
                  <rect x="3" y="4" width="18" height="16" rx="2"></rect>
                  <line x1="3" y1="9.5" x2="21" y2="9.5"></line>
                  <line x1="9" y1="9.5" x2="9" y2="20"></line>
                </svg>
              {:else}
                <svg
                  width="10"
                  height="10"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                >
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

  .search-action-btn {
    flex-shrink: 0;
    background: transparent;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    padding: 2px var(--spacing-2);
    cursor: pointer;
    line-height: 1.6;
    transition:
      background var(--transition-fast),
      color var(--transition-fast),
      border-color var(--transition-fast);
  }

  .search-action-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
    border-color: var(--color-border-strong);
  }

  .search-action-btn--icon {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 3px 5px;
  }

  .search-action-btn--icon.active {
    background: color-mix(in srgb, var(--color-accent) 12%, transparent);
    color: var(--color-accent);
    border-color: color-mix(in srgb, var(--color-accent) 35%, transparent);
  }

  .search-action-btn--icon.has-filters {
    position: relative;
  }

  .search-action-btn--icon.has-filters::after {
    content: '';
    position: absolute;
    top: 2px;
    right: 2px;
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--color-accent);
  }

  .loading-indicator {
    color: var(--color-text-disabled);
    flex-shrink: 0;
    display: flex;
    align-items: center;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .spin {
    animation: spin 0.8s linear infinite;
  }

  .filter-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-4);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .filter-group {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
  }

  .filter-group--right {
    border-left: 1px solid var(--color-border);
    padding-left: var(--spacing-2);
  }

  .filter-group--wrap {
    flex-wrap: wrap;
  }

  .filter-label {
    font-size: var(--font-size-xs);
    color: var(--color-text-disabled);
    font-weight: var(--font-weight-semibold);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    margin-right: 2px;
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
    transition:
      background var(--transition-fast),
      color var(--transition-fast),
      border-color var(--transition-fast);
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
    background: var(--color-accent);
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

  .filter-row--exclude {
    align-items: flex-start;
    gap: var(--spacing-3);
  }

  .exclude-group {
    display: flex;
    align-items: flex-start;
    gap: var(--spacing-1);
    flex: 1;
    min-width: 0;
  }

  .exclude-group--right {
    border-left: 1px solid var(--color-border);
    padding-left: var(--spacing-3);
  }

  .exclude-chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 3px;
    flex: 1;
    min-width: 0;
  }

  .exclude-chip {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    padding: 1px 4px 1px 6px;
    border-radius: var(--radius-md);
    background: color-mix(in srgb, var(--color-accent) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--color-accent) 30%, transparent);
    color: var(--color-accent);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    white-space: nowrap;
  }

  .exclude-chip-remove {
    background: transparent;
    border: none;
    color: inherit;
    cursor: pointer;
    font-size: 11px;
    line-height: 1;
    padding: 0 1px;
    opacity: 0.7;
    display: flex;
    align-items: center;
  }

  .exclude-chip-remove:hover {
    opacity: 1;
  }

  .exclude-input {
    background: transparent;
    border: none;
    border-bottom: 1px solid var(--color-border);
    color: var(--color-text-primary);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    outline: none;
    padding: 1px 2px;
    min-width: 80px;
    max-width: 120px;
  }

  .exclude-input::placeholder {
    color: var(--color-text-disabled);
  }

  .exclude-input:focus {
    border-bottom-color: var(--color-accent);
  }
</style>
