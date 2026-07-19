<script lang="ts">
  import { onMount } from 'svelte';
  import { portal } from '$lib/actions/portal';
  import { executeQuery } from '$lib/tauri/query';
  import type { QueryResult } from '$lib/types';

  interface Props {
    connectionId: string;
    database: string;
    /** The table to search in (the referenced/parent table) */
    referencedTable: string;
    /** The column from that table whose value to insert (usually the PK) */
    referencedColumn: string;
    /** Pixel coords to anchor the popup near (cursor position) */
    anchorX: number;
    anchorY: number;
    onselect: (_value: string) => void;
    onclose: () => void;
  }

  let {
    connectionId,
    database,
    referencedTable,
    referencedColumn,
    anchorX,
    anchorY,
    onselect,
    onclose,
  }: Props = $props();

  let searchInput = $state('');
  let result = $state<QueryResult | null>(null);
  let isLoading = $state(true);
  let error = $state<string | null>(null);
  let inputEl = $state<HTMLInputElement | undefined>(undefined);
  let popupEl = $state<HTMLDivElement | undefined>(undefined);

  /** Quote char determined on first successful query — reused for all subsequent searches. */
  let quoteChar = $state('`');
  /** Column names available for WHERE clause building, set after the first load. */
  let searchableColumns = $state<string[]>([]);

  const PAGE_SIZE = 50;

  // Computed popup position — stay within viewport
  let popupStyle = $derived.by(() => {
    const w = 520;
    const h = 360;
    let left = anchorX;
    let top = anchorY + 4;
    if (left + w > window.innerWidth - 8) left = window.innerWidth - w - 8;
    if (top + h > window.innerHeight - 8) top = anchorY - h - 4;
    if (left < 8) left = 8;
    if (top < 8) top = 8;
    return `left:${left}px;top:${top}px;width:${w}px`;
  });

  function q(name: string): string {
    return `${quoteChar}${name}${quoteChar}`;
  }

  /** Escape a value for use in a LIKE string literal (MariaDB/Postgres both use \ as LIKE escape). */
  function escapeLike(s: string): string {
    return s.replace(/\\/g, '\\\\').replace(/%/g, '\\%').replace(/_/g, '\\_').replace(/'/g, "''");
  }

  function buildSql(search: string): string {
    const table = q(referencedTable);
    if (!search || searchableColumns.length === 0) {
      return `SELECT * FROM ${table}`;
    }
    const term = escapeLike(search);
    const conditions = searchableColumns.map((col) => `${q(col)} LIKE '%${term}%'`).join(' OR ');
    return `SELECT * FROM ${table} WHERE ${conditions}`;
  }

  async function runQuery(sql: string): Promise<void> {
    isLoading = true;
    error = null;
    const db = database || null;

    const attempt = async (sql: string, qc: string): Promise<QueryResult> => {
      quoteChar = qc;
      return executeQuery(
        connectionId,
        sql.replace(/`/g, qc === '"' ? '"' : '`'),
        0,
        PAGE_SIZE,
        db,
      );
    };

    try {
      const r = await attempt(sql, '`');
      if (r.error) {
        error = r.error;
      } else {
        result = r;
        if (searchableColumns.length === 0) {
          searchableColumns = r.columns.map((c) => c.name);
        }
      }
    } catch {
      // Backtick rejected (e.g. Postgres) — switch to double-quote style permanently
      try {
        const sqlDq = sql.replace(/`/g, '"');
        const r2 = await attempt(sqlDq, '"');
        if (r2.error) {
          error = r2.error;
        } else {
          result = r2;
          if (searchableColumns.length === 0) {
            searchableColumns = r2.columns.map((c) => c.name);
          }
        }
      } catch (e2) {
        error = e2 instanceof Error ? e2.message : String(e2);
      }
    } finally {
      isLoading = false;
    }
  }

  // Initial unfiltered load
  onMount(() => {
    runQuery(buildSql(''));
  });

  $effect(() => {
    if (!isLoading && inputEl) inputEl.focus();
  });

  // Debounced server-side search
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;
  let lastSearched = '';

  $effect(() => {
    const term = searchInput.trim();
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      if (term === lastSearched) return;
      lastSearched = term;
      runQuery(buildSql(term));
    }, 300);
    return () => {
      if (debounceTimer) clearTimeout(debounceTimer);
    };
  });

  // Index of the referenced column in the result columns
  const refColIndex = $derived(
    result?.columns.findIndex((c) => c.name.toLowerCase() === referencedColumn.toLowerCase()) ?? -1,
  );

  function handleSelect(row: (string | number | boolean | null)[]): void {
    const value = refColIndex >= 0 ? row[refColIndex] : row[0];
    if (value !== null && value !== undefined) {
      onselect(String(value));
    }
  }

  function handleKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape') {
      e.preventDefault();
      onclose();
    }
  }

  // Close on click outside
  $effect(() => {
    function onMousedown(e: MouseEvent) {
      if (popupEl && !popupEl.contains(e.target as Node)) {
        onclose();
      }
    }
    document.addEventListener('mousedown', onMousedown, true);
    return () => document.removeEventListener('mousedown', onMousedown, true);
  });

  function cellDisplay(value: string | number | boolean | null): string {
    if (value === null) return 'NULL';
    return String(value);
  }

  // Determine which columns to show (limit to first 6 for readability)
  const displayColumns = $derived(result?.columns.slice(0, 6) ?? []);
</script>

<div
  bind:this={popupEl}
  class="fk-popup"
  style={popupStyle}
  role="dialog"
  aria-label="Search {referencedTable}"
  tabindex="-1"
  onkeydown={handleKeydown}
  use:portal
>
  <div class="fk-popup-header">
    <span class="fk-popup-title">Search <code>{referencedTable}</code></span>
    <span class="fk-popup-hint">Select a row to insert <code>{referencedColumn}</code></span>
  </div>

  <div class="fk-popup-search">
    <input
      bind:this={inputEl}
      bind:value={searchInput}
      class="fk-search-input"
      type="text"
      placeholder="Filter rows…"
      autocomplete="off"
      spellcheck={false}
    />
  </div>

  <div class="fk-popup-body">
    {#if isLoading}
      <div class="fk-popup-status">Loading…</div>
    {:else if error}
      <div class="fk-popup-status fk-popup-status--error">{error}</div>
    {:else if !result || result.rows.length === 0}
      <div class="fk-popup-status">No rows{searchInput.trim() ? ' matching search' : ''}</div>
    {:else}
      <table class="fk-table">
        <thead>
          <tr>
            {#each displayColumns as col}
              <th class:fk-col-ref={col.name.toLowerCase() === referencedColumn.toLowerCase()}>
                {col.name}
              </th>
            {/each}
          </tr>
        </thead>
        <tbody>
          {#each result.rows as row}
            <tr
              class="fk-row"
              role="button"
              tabindex="0"
              onclick={() => handleSelect(row)}
              onkeydown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  handleSelect(row);
                }
              }}
            >
              {#each displayColumns as _col, i}
                <td
                  class:fk-cell-null={row[i] === null}
                  class:fk-col-ref={_col.name.toLowerCase() === referencedColumn.toLowerCase()}
                  title={row[i] !== null ? String(row[i]) : 'NULL'}
                >
                  {cellDisplay(row[i])}
                </td>
              {/each}
            </tr>
          {/each}
        </tbody>
      </table>
      {#if result.rows.length >= PAGE_SIZE}
        <div class="fk-popup-footer">
          Showing first {PAGE_SIZE} results — type to search
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .fk-popup {
    position: fixed;
    z-index: 9999;
    background: var(--color-bg-overlay);
    -webkit-backdrop-filter: blur(20px) saturate(160%);
    backdrop-filter: blur(20px) saturate(160%);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-overlay);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: fk-popup-in 120ms both;
    transform-origin: top left;
  }

  @keyframes fk-popup-in {
    from {
      opacity: 0;
      transform: scale(0.96) translateY(-4px);
    }
    to {
      opacity: 1;
      transform: scale(1) translateY(0);
    }
  }

  .fk-popup-header {
    display: flex;
    align-items: baseline;
    gap: var(--spacing-3);
    padding: var(--spacing-2) var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .fk-popup-title {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    font-family: var(--font-family-ui);
  }

  .fk-popup-title code,
  .fk-popup-hint code {
    font-family: var(--font-family-mono);
    font-weight: var(--font-weight-normal);
    color: var(--color-accent);
  }

  .fk-popup-hint {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-family: var(--font-family-ui);
  }

  .fk-popup-search {
    padding: var(--spacing-2) var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .fk-search-input {
    width: 100%;
    height: 26px;
    padding: 0 var(--spacing-2);
    background: var(--color-bg-input, var(--color-bg-secondary));
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--color-text-primary);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    outline: none;
    box-sizing: border-box;
  }

  .fk-search-input:focus {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 2px var(--color-accent-subtle);
  }

  .fk-popup-body {
    flex: 1;
    overflow: auto;
    min-height: 0;
    max-height: 280px;
  }

  .fk-popup-status {
    padding: var(--spacing-4) var(--spacing-3);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-family: var(--font-family-ui);
    text-align: center;
  }

  .fk-popup-status--error {
    color: var(--color-danger, #ef4444);
  }

  .fk-table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
  }

  .fk-table thead {
    position: sticky;
    top: 0;
    background: var(--color-bg-overlay);
    z-index: 1;
  }

  .fk-table th {
    padding: 4px var(--spacing-3);
    text-align: left;
    font-family: var(--font-family-ui);
    font-weight: var(--font-weight-semibold);
    font-size: 10px;
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    border-bottom: 1px solid var(--color-border);
    white-space: nowrap;
  }

  .fk-table th.fk-col-ref {
    color: var(--color-accent);
  }

  .fk-row {
    cursor: pointer;
    border: none;
  }

  .fk-row:hover td {
    background: var(--color-bg-active);
  }

  .fk-row:focus-visible td {
    background: var(--color-accent-subtle);
    outline: none;
  }

  .fk-table td {
    padding: 3px var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--color-text-primary);
    transition: background var(--transition-fast);
  }

  .fk-table td.fk-col-ref {
    color: var(--color-accent);
    font-weight: var(--font-weight-medium);
  }

  .fk-cell-null {
    color: var(--color-text-muted) !important;
    font-style: italic;
  }

  .fk-popup-footer {
    padding: 4px var(--spacing-3);
    font-size: 10px;
    color: var(--color-text-muted);
    font-family: var(--font-family-ui);
    border-top: 1px solid var(--color-border);
    text-align: center;
  }
</style>
