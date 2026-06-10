<script lang="ts">
  import { executeQuery } from '$lib/tauri/query';
  import { useConnections } from '$lib/stores/connections.svelte';
  import type { QueryResult } from '$lib/types';
  import DataTable from '$lib/components/table/DataTable.svelte';

  interface Props {
    connectionId: string;
    database: string;
    table: string;
  }

  let { connectionId, database, table }: Props = $props();

  const connections = useConnections();

  const PAGE_SIZE = 50;

  let page = $state(1);
  let filterValue = $state('');
  let pendingFilter = $state('');
  let result = $state<QueryResult | null>(null);
  let isLoading = $state(false);
  let error = $state<string | null>(null);

  let dbType = $derived(connections.getById(connectionId)?.dbType ?? 'mysql');

  // Postgres uses double-quote identifiers; MySQL/MariaDB use backticks.
  function quoteIdentifier(name: string): string {
    return dbType === 'postgres' ? `"${name}"` : `\`${name}\``;
  }

  function buildSql(): string {
    const quotedDb = quoteIdentifier(database);
    const quotedTable = quoteIdentifier(table);
    const offset = (page - 1) * PAGE_SIZE;
    let base = `SELECT * FROM ${quotedDb}.${quotedTable}`;
    if (filterValue.trim()) {
      base += ` WHERE ${filterValue.trim()}`;
    }
    base += ` LIMIT ${PAGE_SIZE} OFFSET ${offset}`;
    return base;
  }

  async function load(): Promise<void> {
    isLoading = true;
    error = null;
    try {
      result = await executeQuery(connectionId, buildSql(), page, PAGE_SIZE);
      if (result.error) {
        error = result.error;
        result = null;
      }
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
      result = null;
    } finally {
      isLoading = false;
    }
  }

  function handleFilterKeydown(e: KeyboardEvent): void {
    if (e.key === 'Enter') {
      filterValue = pendingFilter;
      page = 1;
      load();
    }
  }

  function handleRefresh(): void {
    load();
  }

  // Re-fetch whenever the identity props change so switching to a different
  // table in the same panel does not show stale data.
  $effect(() => {
    // Referencing all three identity props ensures the effect re-runs when any changes.
    const _conn = connectionId;
    const _db = database;
    const _tbl = table;

    page = 1;
    filterValue = '';
    pendingFilter = '';
    load();
  });
</script>

<div class="table-browser">
  <div class="toolbar">
    <span class="table-name" title={`${database}.${table}`}>
      <span class="db-name">{database}</span>
      <span class="separator">.</span>
      <span class="tbl-name">{table}</span>
    </span>

    <div class="toolbar-spacer"></div>

    <div class="filter-wrapper">
      <label class="filter-label" for="tb-filter">WHERE</label>
      <input
        id="tb-filter"
        class="filter-input"
        type="text"
        placeholder="condition… (press Enter)"
        value={pendingFilter}
        oninput={(e) => {
          pendingFilter = (e.target as HTMLInputElement).value;
        }}
        onkeydown={handleFilterKeydown}
        aria-label="WHERE filter clause"
      />
    </div>

    <button
      class="refresh-button"
      onclick={handleRefresh}
      disabled={isLoading}
      title="Refresh"
      aria-label="Refresh table data"
    >
      ↺
    </button>
  </div>

  <div class="content">
    {#if isLoading}
      <div class="loading">
        <span class="loading-text">Loading…</span>
      </div>
    {:else if error !== null}
      <div class="error-box" role="alert">
        <span class="error-label">Error</span>
        <span class="error-message">{error}</span>
      </div>
    {:else if result !== null}
      <DataTable columns={result.columns} rows={result.rows} pageSize={PAGE_SIZE} />
    {:else}
      <div class="loading">
        <span class="loading-text">No data.</span>
      </div>
    {/if}
  </div>
</div>

<style>
  .table-browser {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: var(--color-bg-primary);
  }

  .toolbar {
    flex: 0 0 var(--toolbar-height);
    min-height: var(--toolbar-height);
    max-height: var(--toolbar-height);
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: 0 var(--spacing-3);
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
  }

  .table-name {
    display: flex;
    align-items: center;
    gap: 0;
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    flex-shrink: 0;
    max-width: 220px;
    overflow: hidden;
  }

  .db-name {
    color: var(--color-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .separator {
    color: var(--color-text-muted);
    margin: 0 1px;
    flex-shrink: 0;
  }

  .tbl-name {
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-family-mono);
  }

  .toolbar-spacer {
    flex: 1;
  }

  .filter-wrapper {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
  }

  .filter-label {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-muted);
    font-family: var(--font-family-mono);
    flex-shrink: 0;
    user-select: none;
  }

  .filter-input {
    width: 200px;
    padding: var(--spacing-1) var(--spacing-2);
    background: var(--color-bg-primary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-primary);
    outline: none;
    transition:
      border-color var(--transition-fast),
      width var(--transition-md);
  }

  .filter-input:focus {
    border-color: var(--color-accent);
    width: 280px;
  }

  .filter-input::placeholder {
    color: var(--color-text-muted);
    font-family: var(--font-family-ui);
  }

  .refresh-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--spacing-6);
    height: var(--spacing-6);
    padding: 0;
    background: transparent;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-md);
    color: var(--color-text-secondary);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
    flex-shrink: 0;
    line-height: 1;
  }

  .refresh-button:hover:not(:disabled) {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .refresh-button:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .content {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .loading {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .loading-text {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .error-box {
    margin: var(--spacing-4);
    padding: var(--spacing-3) var(--spacing-4);
    background: var(--color-danger-subtle);
    border: 1px solid var(--color-danger);
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-1);
  }

  .error-label {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-danger);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .error-message {
    font-size: var(--font-size-sm);
    color: var(--color-danger);
    font-family: var(--font-family-mono);
    white-space: pre-wrap;
    word-break: break-word;
  }
</style>
