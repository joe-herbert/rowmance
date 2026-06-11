<script lang="ts">
  import { executeQuery, updateRows } from '$lib/tauri/query';
  import type { RowChange } from '$lib/tauri/query';
  import { useConnections } from '$lib/stores/connections.svelte';
  import type { QueryResult, ColumnMeta } from '$lib/types';
  import DataTable from '$lib/components/table/DataTable.svelte';
  import ColumnPicker from '$lib/components/table/ColumnPicker.svelte';

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

  // ── Pending changes ───────────────────────────────────────────────────────

  type CellValue = string | number | boolean | null;
  let pendingChanges = $state<Map<string, Map<string, CellValue>>>(new Map());
  let isSaving = $state(false);
  let saveError = $state<string | null>(null);

  // Key to force DataTable to fully reset (clears pending changes displayed)
  let tableKey = $state(0);

  const pendingCount = $derived(pendingChanges.size);

  function handleChangePending(changes: Map<string, Map<string, CellValue>>): void {
    pendingChanges = changes;
  }

  function discardChanges(): void {
    pendingChanges = new Map();
    tableKey++;
  }

  async function saveChanges(): Promise<void> {
    if (!result) return;
    isSaving = true;
    saveError = null;

    try {
      const pkColumns = result.columns.filter((c) => c.isPrimaryKey).map((c) => c.name);

      const rowChanges: RowChange[] = [];

      for (const [rowKey, colMap] of pendingChanges) {
        // Build primary keys from the row key
        // rowKey is either the PK values joined with | or the row index
        const primaryKeys: Record<string, unknown> = {};

        if (pkColumns.length > 0) {
          const parts = rowKey.split('|');
          pkColumns.forEach((pkCol, i) => {
            primaryKeys[pkCol] = parts[i] ?? null;
          });
        } else {
          // No PK — use _rowIndex as a hint; server may not support this
          primaryKeys['_rowIndex'] = rowKey;
        }

        const changes: Record<string, unknown> = {};
        for (const [col, val] of colMap) {
          changes[col] = val;
        }

        rowChanges.push({ primaryKeys, changes });
      }

      await updateRows(connectionId, database, table, rowChanges);
      pendingChanges = new Map();
      tableKey++;
      await load();
    } catch (err) {
      saveError = err instanceof Error ? err.message : String(err);
    } finally {
      isSaving = false;
    }
  }

  // ── Column visibility ─────────────────────────────────────────────────────

  let hiddenColumns = $state<Set<string>>(new Set());
  let showColumnPicker = $state(false);
  let columnPickerAnchorEl = $state<HTMLButtonElement | null>(null);

  function toggleColumn(name: string): void {
    const next = new Set(hiddenColumns);
    if (next.has(name)) next.delete(name);
    else next.add(name);
    hiddenColumns = next;
  }

  // ── DB type + SQL helpers ─────────────────────────────────────────────────

  let dbType = $derived(connections.getById(connectionId)?.dbType ?? 'mysql');

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

  // Re-fetch whenever the identity props change.
  $effect(() => {
    const _conn = connectionId;
    const _db = database;
    const _tbl = table;

    page = 1;
    filterValue = '';
    pendingFilter = '';
    pendingChanges = new Map();
    hiddenColumns = new Set();
    showColumnPicker = false;
    load();
  });

  // Column picker position
  let pickerTop = $state(0);
  let pickerLeft = $state(0);

  function openColumnPicker(): void {
    if (columnPickerAnchorEl) {
      const rect = columnPickerAnchorEl.getBoundingClientRect();
      const containerRect = columnPickerAnchorEl
        .closest('.table-browser')
        ?.getBoundingClientRect();
      if (containerRect) {
        pickerTop = rect.bottom - containerRect.top + 4;
        pickerLeft = rect.right - containerRect.left - 220; // align right edge
      }
    }
    showColumnPicker = true;
  }

  const currentColumns = $derived<ColumnMeta[]>(result?.columns ?? []);
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

    {#if currentColumns.length > 0}
      <button
        bind:this={columnPickerAnchorEl}
        class="toolbar-btn"
        onclick={openColumnPicker}
        title="Show/hide columns"
        aria-label="Column visibility"
        aria-expanded={showColumnPicker}
      >
        Columns
        {#if hiddenColumns.size > 0}
          <span class="badge">{hiddenColumns.size} hidden</span>
        {/if}
      </button>
    {/if}

    {#if pendingCount > 0}
      <button
        class="toolbar-btn save-btn"
        onclick={saveChanges}
        disabled={isSaving}
        title="Save pending changes to the database"
        aria-label="Save {pendingCount} pending changes"
      >
        {isSaving ? 'Saving…' : `Save ${pendingCount} change${pendingCount !== 1 ? 's' : ''}`}
      </button>

      <button
        class="toolbar-btn discard-btn"
        onclick={discardChanges}
        disabled={isSaving}
        title="Discard all pending changes"
        aria-label="Discard changes"
      >
        Discard
      </button>
    {/if}

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

  {#if saveError !== null}
    <div class="save-error-bar" role="alert">
      <span class="save-error-label">Save failed:</span>
      <span class="save-error-message">{saveError}</span>
      <button class="save-error-close" onclick={() => (saveError = null)} aria-label="Dismiss">
        ✕
      </button>
    </div>
  {/if}

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
      {#key tableKey}
        <DataTable
          columns={result.columns}
          rows={result.rows}
          pageSize={PAGE_SIZE}
          editable={true}
          {hiddenColumns}
          onChangePending={handleChangePending}
        />
      {/key}
    {:else}
      <div class="loading">
        <span class="loading-text">No data.</span>
      </div>
    {/if}
  </div>

  {#if showColumnPicker && currentColumns.length > 0}
    <div class="picker-positioner" style="top: {pickerTop}px; left: {pickerLeft}px;">
      <ColumnPicker
        columns={currentColumns}
        {hiddenColumns}
        onToggle={toggleColumn}
        onClose={() => (showColumnPicker = false)}
      />
    </div>
  {/if}
</div>

<style>
  .table-browser {
    position: relative;
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
    overflow: hidden;
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

  .toolbar-btn {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    height: calc(var(--toolbar-height) - var(--spacing-2) * 2);
    padding: 0 var(--spacing-2);
    background: var(--color-bg-tertiary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    color: var(--color-text-secondary);
    cursor: pointer;
    white-space: nowrap;
    flex-shrink: 0;
    transition:
      background var(--transition-fast),
      color var(--transition-fast),
      border-color var(--transition-fast);
  }

  .toolbar-btn:hover:not(:disabled) {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .toolbar-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .save-btn {
    background: var(--color-accent-subtle);
    border-color: var(--color-accent);
    color: var(--color-accent);
    font-weight: var(--font-weight-medium);
  }

  .save-btn:hover:not(:disabled) {
    background: var(--color-accent);
    color: var(--color-text-on-accent);
  }

  .discard-btn {
    color: var(--color-danger);
    border-color: var(--color-danger);
    background: var(--color-danger-subtle);
  }

  .discard-btn:hover:not(:disabled) {
    background: var(--color-danger);
    color: var(--color-text-on-accent);
  }

  .badge {
    font-size: 10px;
    padding: 1px var(--spacing-1);
    background: var(--color-accent);
    color: var(--color-text-on-accent);
    border-radius: var(--radius-sm);
    font-variant-numeric: tabular-nums;
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

  /* ── Save error bar ──────────────────────────────────────────────────────── */

  .save-error-bar {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-1) var(--spacing-3);
    background: var(--color-danger-subtle);
    border-bottom: 1px solid var(--color-danger);
    font-size: var(--font-size-xs);
  }

  .save-error-label {
    font-weight: var(--font-weight-semibold);
    color: var(--color-danger);
    flex-shrink: 0;
  }

  .save-error-message {
    color: var(--color-danger);
    font-family: var(--font-family-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
  }

  .save-error-close {
    flex-shrink: 0;
    padding: 0 var(--spacing-1);
    background: transparent;
    border: none;
    font-size: var(--font-size-xs);
    color: var(--color-danger);
    cursor: pointer;
    border-radius: var(--radius-sm);
    transition: background var(--transition-fast);
  }

  .save-error-close:hover {
    background: var(--color-danger-subtle);
  }

  /* ── Content area ────────────────────────────────────────────────────────── */

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

  /* ── Column picker positioner ────────────────────────────────────────────── */

  .picker-positioner {
    position: absolute;
    z-index: 200;
  }
</style>
