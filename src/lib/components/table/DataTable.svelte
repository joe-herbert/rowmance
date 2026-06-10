<!--
  DataTable — read-only paginated table for Phase 1.
  Receives columns and rows from the query result; pagination is client-side
  over the page already fetched from the backend.
  Sorting, filtering, and column resize are Phase 2.
-->
<script lang="ts">
  import type { ColumnMeta } from '$lib/types';

  type CellValue = string | number | boolean | null;

  interface Props {
    columns: ColumnMeta[];
    rows: CellValue[][];
    pageSize?: number;
  }

  let { columns, rows, pageSize = 50 }: Props = $props();

  let pageIndex = $state(0);

  const pageCount = $derived(Math.max(1, Math.ceil(rows.length / pageSize)));
  const pageRows = $derived(rows.slice(pageIndex * pageSize, (pageIndex + 1) * pageSize));

  // Reset to first page when rows change (new query result).
  $effect(() => {
    rows;
    pageIndex = 0;
  });

  function previousPage() {
    if (pageIndex > 0) pageIndex--;
  }

  function nextPage() {
    if (pageIndex < pageCount - 1) pageIndex++;
  }

  function formatCell(value: CellValue): string {
    if (value === null) return '';
    return String(value);
  }
</script>

<div class="data-table-wrapper">
  <div class="table-scroll">
    <table class="data-table">
      <thead>
        <tr class="header-row">
          {#each columns as col (col.name)}
            <th class="header-cell" title="{col.name} ({col.dataType})">
              <span class="header-name">{col.name}</span>
              <span class="header-type">{col.dataType}</span>
            </th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#each pageRows as row, rowIndex (rowIndex)}
          <tr class="data-row" class:alt={rowIndex % 2 === 1}>
            {#each row as value, colIndex (colIndex)}
              <td class="data-cell">
                {#if value === null}
                  <span class="null-value">NULL</span>
                {:else if typeof value === 'boolean'}
                  <span class="bool-value" class:bool-true={value} class:bool-false={!value}>
                    {value ? '✓' : '✗'}
                  </span>
                {:else}
                  {formatCell(value)}
                {/if}
              </td>
            {/each}
          </tr>
        {/each}

        {#if rows.length === 0}
          <tr>
            <td class="empty-cell" colspan={columns.length || 1}>No rows to display.</td>
          </tr>
        {/if}
      </tbody>
    </table>
  </div>

  <div class="pagination-bar">
    <button
      class="page-btn"
      onclick={previousPage}
      disabled={pageIndex === 0}
      aria-label="Previous page"
    >
      ←
    </button>

    <span class="page-label">
      Page {pageIndex + 1} of {pageCount}
    </span>

    <button
      class="page-btn"
      onclick={nextPage}
      disabled={pageIndex >= pageCount - 1}
      aria-label="Next page"
    >
      →
    </button>

    <span class="row-count">
      {rows.length.toLocaleString()} row{rows.length !== 1 ? 's' : ''}
    </span>
  </div>
</div>

<style>
  .data-table-wrapper {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: var(--color-bg-primary);
  }

  .table-scroll {
    flex: 1;
    overflow: auto;
    scrollbar-color: var(--color-scrollbar-thumb) var(--color-scrollbar-track);
    scrollbar-width: thin;
  }

  .table-scroll::-webkit-scrollbar {
    width: 8px;
    height: 8px;
  }

  .table-scroll::-webkit-scrollbar-thumb {
    background: var(--color-scrollbar-thumb);
    border-radius: var(--radius-sm);
  }

  .table-scroll::-webkit-scrollbar-thumb:hover {
    background: var(--color-scrollbar-thumb-hover);
  }

  .data-table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
  }

  thead {
    position: sticky;
    top: 0;
    z-index: 1;
  }

  .header-row {
    background: var(--color-table-header-bg);
    height: var(--table-header-height);
    border-bottom: 1px solid var(--color-border-strong);
  }

  .header-cell {
    padding: var(--table-cell-padding-y) var(--table-cell-padding-x);
    font-weight: var(--font-weight-medium);
    font-size: var(--font-size-xs);
    text-align: left;
    color: var(--color-text-secondary);
    background: var(--color-table-header-bg);
    border-right: 1px solid var(--color-border);
    white-space: nowrap;
    min-width: 80px;
  }

  .header-name {
    display: block;
  }

  .header-type {
    display: block;
    font-weight: var(--font-weight-normal);
    color: var(--color-text-muted);
    font-size: 10px;
    margin-top: 1px;
  }

  .data-row {
    height: var(--table-row-height);
    border-bottom: 1px solid var(--color-border);
    transition: background var(--transition-fast);
  }

  .data-row:hover {
    background: var(--color-table-row-hover);
  }

  .data-row.alt {
    background: var(--color-table-row-alt);
  }

  .data-row.alt:hover {
    background: var(--color-table-row-hover);
  }

  .data-cell {
    padding: var(--table-cell-padding-y) var(--table-cell-padding-x);
    border-right: 1px solid var(--color-border);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 300px;
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    vertical-align: middle;
  }

  .null-value {
    color: var(--color-null);
    font-style: italic;
    font-size: var(--font-size-xs);
  }

  .bool-value {
    font-weight: var(--font-weight-medium);
  }

  .bool-true {
    color: var(--color-success);
  }

  .bool-false {
    color: var(--color-danger);
  }

  .empty-cell {
    padding: var(--spacing-8) var(--spacing-4);
    text-align: center;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .pagination-bar {
    flex: 0 0 var(--statusbar-height);
    min-height: var(--statusbar-height);
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: 0 var(--spacing-3);
    background: var(--color-bg-secondary);
    border-top: 1px solid var(--color-border);
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
  }

  .page-btn {
    padding: 0 var(--spacing-2);
    height: calc(var(--statusbar-height) - var(--spacing-1) * 2);
    background: var(--color-bg-tertiary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    color: var(--color-text-primary);
    cursor: pointer;
    transition: background var(--transition-fast);
    line-height: 1;
  }

  .page-btn:hover:not(:disabled) {
    background: var(--color-bg-active);
  }

  .page-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .page-label {
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
  }

  .row-count {
    margin-left: auto;
    color: var(--color-text-muted);
    font-variant-numeric: tabular-nums;
  }
</style>
