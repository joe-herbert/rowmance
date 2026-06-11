<!--
  DataTable — paginated, sortable, filterable, resizable, editable table.
  Phase 2: adds sorting, column filters, column resize, column visibility, and
  inline cell editing with pending-change tracking.
-->
<script lang="ts">
  import type { ColumnMeta } from '$lib/types';
  import CellEditor from './CellEditor.svelte';

  type CellValue = string | number | boolean | null;
  type SortDir = 'asc' | 'desc' | 'none';

  interface Props {
    columns: ColumnMeta[];
    rows: CellValue[][];
    pageSize?: number;
    editable?: boolean;
    hiddenColumns?: Set<string>;
    onChangePending?: (changes: Map<string, Map<string, CellValue>>) => void;
  }

  // ── Pure helper functions (exported for tests) ────────────────────────────

  export function sortRows(
    rows: CellValue[][],
    colIndex: number,
    dir: SortDir,
  ): CellValue[][] {
    if (dir === 'none' || colIndex < 0) return rows;
    const sorted = [...rows].sort((a, b) => {
      const av = a[colIndex];
      const bv = b[colIndex];
      // nulls sort last
      if (av === null && bv === null) return 0;
      if (av === null) return 1;
      if (bv === null) return -1;
      if (typeof av === 'number' && typeof bv === 'number') {
        return dir === 'asc' ? av - bv : bv - av;
      }
      const as = String(av).toLowerCase();
      const bs = String(bv).toLowerCase();
      if (as < bs) return dir === 'asc' ? -1 : 1;
      if (as > bs) return dir === 'asc' ? 1 : -1;
      return 0;
    });
    return sorted;
  }

  export function filterRows(
    rows: CellValue[][],
    filters: string[],
  ): CellValue[][] {
    const activeFilters = filters
      .map((f, i) => ({ i, term: f.trim().toLowerCase() }))
      .filter((f) => f.term !== '');

    if (activeFilters.length === 0) return rows;

    return rows.filter((row) =>
      activeFilters.every(({ i, term }) => {
        const cell = row[i];
        const cellStr = cell === null ? '' : String(cell).toLowerCase();
        return cellStr.includes(term);
      }),
    );
  }

  export function buildRowKey(
    row: CellValue[],
    columns: ColumnMeta[],
    rowIndex: number,
  ): string {
    const pkCols = columns
      .map((col, idx) => ({ col, idx }))
      .filter(({ col }) => col.isPrimaryKey);

    if (pkCols.length > 0) {
      return pkCols.map(({ idx }) => String(row[idx] ?? '')).join('|');
    }
    return String(rowIndex);
  }

  /**
   * Moves an item in `order` from display position `fromIdx` to `toIdx`.
   * Returns a new array; does not mutate the original.
   */
  export function reorderColumns(order: number[], fromIdx: number, toIdx: number): number[] {
    if (fromIdx === toIdx) return order;
    const next = [...order];
    const [item] = next.splice(fromIdx, 1);
    next.splice(toIdx, 0, item);
    return next;
  }

  // ── Props ─────────────────────────────────────────────────────────────────

  let {
    columns,
    rows,
    pageSize = 50,
    editable = false,
    hiddenColumns = new Set<string>(),
    onChangePending,
  }: Props = $props();

  // ── Column order (drag-to-reorder) ───────────────────────────────────────

  // Maps display position → original column index. Initialised from columns prop.
  let columnOrder = $state<number[]>(columns.map((_, i) => i));

  $effect(() => {
    // Sync when the columns array itself changes (new query result).
    if (columnOrder.length !== columns.length) {
      columnOrder = columns.map((_, i) => i);
    }
  });

  let draggingColName = $state<string | null>(null);
  let dragOverColName = $state<string | null>(null);

  function onHeaderDragStart(colName: string): void {
    draggingColName = colName;
  }

  function onHeaderDragOver(e: DragEvent, colName: string): void {
    e.preventDefault();
    dragOverColName = colName;
  }

  function onHeaderDrop(targetColName: string): void {
    if (!draggingColName || draggingColName === targetColName) {
      draggingColName = null;
      dragOverColName = null;
      return;
    }
    const fromOriginal = columns.findIndex((c) => c.name === draggingColName);
    const toOriginal = columns.findIndex((c) => c.name === targetColName);
    const fromDisplay = columnOrder.indexOf(fromOriginal);
    const toDisplay = columnOrder.indexOf(toOriginal);
    columnOrder = reorderColumns(columnOrder, fromDisplay, toDisplay);
    draggingColName = null;
    dragOverColName = null;
  }

  function onHeaderDragEnd(): void {
    draggingColName = null;
    dragOverColName = null;
  }

  // ── Visible columns (after ordering and hiding) ───────────────────────────

  const visibleColumns = $derived(
    columnOrder
      .map((originalIndex) => ({ col: columns[originalIndex], originalIndex }))
      .filter(({ col }) => col && !hiddenColumns.has(col.name)),
  );

  // ── Sort state ────────────────────────────────────────────────────────────

  let sortColIndex = $state(-1); // index into original columns array
  let sortDir = $state<SortDir>('none');

  function toggleSort(originalIndex: number): void {
    if (sortColIndex !== originalIndex) {
      sortColIndex = originalIndex;
      sortDir = 'asc';
    } else if (sortDir === 'asc') {
      sortDir = 'desc';
    } else if (sortDir === 'desc') {
      sortDir = 'none';
      sortColIndex = -1;
    }
    pageIndex = 0;
  }

  // ── Filter state ──────────────────────────────────────────────────────────

  // Keyed by original column index
  let filterValues = $state<string[]>(columns.map(() => ''));

  $effect(() => {
    // Resize filterValues when columns change
    const len = columns.length;
    if (filterValues.length !== len) {
      filterValues = columns.map(() => '');
    }
  });

  function clearFilter(originalIndex: number): void {
    filterValues[originalIndex] = '';
    pageIndex = 0;
  }

  // ── Column widths ─────────────────────────────────────────────────────────

  let colWidths = $state<number[]>(columns.map(() => 120));

  $effect(() => {
    const len = columns.length;
    if (colWidths.length !== len) {
      colWidths = columns.map(() => 120);
    }
  });

  let resizingColIndex = $state<number | null>(null);
  let resizeStartX = $state(0);
  let resizeStartWidth = $state(0);

  function onResizePointerDown(e: PointerEvent, originalIndex: number): void {
    e.preventDefault();
    e.stopPropagation();
    resizingColIndex = originalIndex;
    resizeStartX = e.clientX;
    resizeStartWidth = colWidths[originalIndex] ?? 120;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onResizePointerMove(e: PointerEvent): void {
    if (resizingColIndex === null) return;
    const delta = e.clientX - resizeStartX;
    colWidths[resizingColIndex] = Math.max(60, resizeStartWidth + delta);
  }

  function onResizePointerUp(): void {
    resizingColIndex = null;
  }

  // ── Derived: sorted + filtered rows ──────────────────────────────────────

  const processedRows = $derived.by(() => {
    const sorted = sortRows(rows, sortColIndex, sortDir);
    return filterRows(sorted, filterValues);
  });

  // ── Pagination ────────────────────────────────────────────────────────────

  let pageIndex = $state(0);

  const pageCount = $derived(Math.max(1, Math.ceil(processedRows.length / pageSize)));
  const pageRows = $derived(
    processedRows.slice(pageIndex * pageSize, (pageIndex + 1) * pageSize),
  );

  // Track offset of the current page into processedRows for row key generation
  const pageOffset = $derived(pageIndex * pageSize);

  // Reset to first page when rows or filters change
  $effect(() => {
    rows;
    filterValues.join('|');
    pageIndex = 0;
  });

  function previousPage(): void {
    if (pageIndex > 0) pageIndex--;
  }

  function nextPage(): void {
    if (pageIndex < pageCount - 1) pageIndex++;
  }

  // ── Pending changes ───────────────────────────────────────────────────────

  let pendingChanges = $state<Map<string, Map<string, CellValue>>>(new Map());

  function getPendingValue(
    rowKey: string,
    colName: string,
    originalValue: CellValue,
  ): CellValue {
    const rowMap = pendingChanges.get(rowKey);
    if (rowMap?.has(colName)) return rowMap.get(colName) as CellValue;
    return originalValue;
  }

  function hasPendingChange(rowKey: string, colName: string): boolean {
    return pendingChanges.get(rowKey)?.has(colName) ?? false;
  }

  // ── Cell editor state ─────────────────────────────────────────────────────

  interface EditTarget {
    rowKey: string;
    colName: string;
    colIndex: number; // original column index
    value: CellValue;
    dataType: string;
    top: number;
    left: number;
    width: number;
    height: number;
  }

  let editTarget = $state<EditTarget | null>(null);
  let tableContainerEl = $state<HTMLDivElement | null>(null);

  function handleCellDblClick(
    e: MouseEvent,
    row: CellValue[],
    processedRowIndex: number,
    originalColIndex: number,
  ): void {
    if (!editable) return;
    const col = columns[originalColIndex];
    if (!col) return;

    const rowKey = buildRowKey(row, columns, pageOffset + processedRowIndex);
    const currentValue = getPendingValue(rowKey, col.name, row[originalColIndex]);

    const td = e.currentTarget as HTMLTableCellElement;
    const containerRect = tableContainerEl!.getBoundingClientRect();
    const tdRect = td.getBoundingClientRect();

    editTarget = {
      rowKey,
      colName: col.name,
      colIndex: originalColIndex,
      value: currentValue,
      dataType: col.dataType,
      top: tdRect.top - containerRect.top,
      left: tdRect.left - containerRect.left,
      width: Math.max(tdRect.width, 160),
      height: tdRect.height,
    };
  }

  function confirmEdit(newValue: CellValue): void {
    if (!editTarget) return;
    const { rowKey, colName } = editTarget;

    const updated = new Map(pendingChanges);
    if (!updated.has(rowKey)) updated.set(rowKey, new Map());
    updated.get(rowKey)!.set(colName, newValue);

    pendingChanges = updated;
    onChangePending?.(pendingChanges);
    editTarget = null;
  }

  function cancelEdit(): void {
    editTarget = null;
  }

  // ── Cell formatting ───────────────────────────────────────────────────────

  function formatCell(value: CellValue): string {
    if (value === null) return '';
    return String(value);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  bind:this={tableContainerEl}
  class="data-table-wrapper"
  onpointermove={onResizePointerMove}
  onpointerup={onResizePointerUp}
>
  <div class="table-scroll">
    <table class="data-table">
      <colgroup>
        {#each visibleColumns as { originalIndex }}
          <col style="width: {colWidths[originalIndex]}px; min-width: 60px;" />
        {/each}
      </colgroup>

      <thead>
        <!-- Header row -->
        <tr class="header-row">
          {#each visibleColumns as { col, originalIndex }}
            {@const isSorted = sortColIndex === originalIndex}
            {@const isDragging = draggingColName === col.name}
            {@const isDragOver = dragOverColName === col.name && draggingColName !== col.name}
            <th
              class="header-cell"
              class:dragging={isDragging}
              class:drag-over={isDragOver}
              style="width: {colWidths[originalIndex]}px;"
              title="{col.name} ({col.dataType})"
              draggable={true}
              ondragstart={() => onHeaderDragStart(col.name)}
              ondragover={(e) => onHeaderDragOver(e, col.name)}
              ondrop={() => onHeaderDrop(col.name)}
              ondragend={onHeaderDragEnd}
            >
              <button
                class="header-btn"
                onclick={() => toggleSort(originalIndex)}
                aria-label="Sort by {col.name}"
                title="Sort by {col.name}"
              >
                <span class="header-name">{col.name}</span>
                <span class="header-type">{col.dataType}</span>
                {#if isSorted && sortDir !== 'none'}
                  <span class="sort-indicator" aria-label={sortDir === 'asc' ? 'ascending' : 'descending'}>
                    {sortDir === 'asc' ? '▲' : '▼'}
                  </span>
                {/if}
              </button>

              <div
                class="resize-handle"
                role="separator"
                aria-label="Resize {col.name} column"
                onpointerdown={(e) => onResizePointerDown(e, originalIndex)}
              ></div>
            </th>
          {/each}
        </tr>

        <!-- Filter row -->
        <tr class="filter-row">
          {#each visibleColumns as { col, originalIndex }}
            <th class="filter-cell">
              <div class="filter-cell-inner">
                <input
                  class="filter-input"
                  type="text"
                  placeholder="filter…"
                  value={filterValues[originalIndex]}
                  oninput={(e) => {
                    filterValues[originalIndex] = (e.target as HTMLInputElement).value;
                    pageIndex = 0;
                  }}
                  aria-label="Filter {col.name}"
                />
                {#if filterValues[originalIndex]}
                  <button
                    class="filter-clear"
                    onclick={() => clearFilter(originalIndex)}
                    aria-label="Clear filter for {col.name}"
                    title="Clear filter"
                  >
                    ✕
                  </button>
                {/if}
              </div>
            </th>
          {/each}
        </tr>
      </thead>

      <tbody>
        {#each pageRows as row, rowIndex (rowIndex)}
          {@const processedRowIndex = rowIndex}
          {@const rowKey = buildRowKey(row, columns, pageOffset + processedRowIndex)}
          <tr class="data-row" class:alt={rowIndex % 2 === 1}>
            {#each visibleColumns as { col, originalIndex }}
              {@const cellValue = getPendingValue(rowKey, col.name, row[originalIndex])}
              {@const isPending = hasPendingChange(rowKey, col.name)}
              <td
                class="data-cell"
                class:cell-pending={isPending}
                class:cell-editable={editable}
                ondblclick={(e) => handleCellDblClick(e, row, processedRowIndex, originalIndex)}
                title={editable ? 'Double-click to edit' : undefined}
              >
                {#if cellValue === null}
                  <span class="null-value">NULL</span>
                {:else if typeof cellValue === 'boolean'}
                  <span
                    class="bool-value"
                    class:bool-true={cellValue}
                    class:bool-false={!cellValue}
                  >
                    {cellValue ? '✓' : '✗'}
                  </span>
                {:else}
                  {formatCell(cellValue)}
                {/if}
              </td>
            {/each}
          </tr>
        {/each}

        {#if processedRows.length === 0}
          <tr>
            <td class="empty-cell" colspan={visibleColumns.length || 1}>
              {rows.length === 0 ? 'No rows to display.' : 'No rows match the current filters.'}
            </td>
          </tr>
        {/if}
      </tbody>
    </table>
  </div>

  <!-- Inline cell editor overlay -->
  {#if editTarget !== null}
    <CellEditor
      value={editTarget.value}
      dataType={editTarget.dataType}
      top={editTarget.top}
      left={editTarget.left}
      width={editTarget.width}
      height={editTarget.height}
      onConfirm={confirmEdit}
      onCancel={cancelEdit}
    />
  {/if}

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
      {processedRows.length.toLocaleString()}
      {processedRows.length !== rows.length
        ? ` / ${rows.length.toLocaleString()} rows (filtered)`
        : ` row${rows.length !== 1 ? 's' : ''}`}
    </span>
  </div>
</div>

<style>
  .data-table-wrapper {
    position: relative;
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
    border-collapse: collapse;
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    table-layout: fixed;
  }

  thead {
    position: sticky;
    top: 0;
    z-index: 1;
  }

  /* ── Header row ─────────────────────────────────────────────────────────── */

  .header-row {
    background: var(--color-table-header-bg);
    height: var(--table-header-height);
    border-bottom: 1px solid var(--color-border-strong);
  }

  .header-cell {
    position: relative;
    padding: 0;
    font-weight: var(--font-weight-medium);
    font-size: var(--font-size-xs);
    text-align: left;
    color: var(--color-text-secondary);
    background: var(--color-table-header-bg);
    border-right: 1px solid var(--color-border);
    white-space: nowrap;
    overflow: hidden;
    box-sizing: border-box;
    user-select: none;
  }

  .header-btn {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    width: calc(100% - 6px); /* leave room for resize handle */
    height: 100%;
    padding: var(--table-cell-padding-y) var(--table-cell-padding-x);
    background: transparent;
    border: none;
    cursor: pointer;
    text-align: left;
    color: inherit;
    font-size: inherit;
    font-weight: inherit;
    font-family: inherit;
    transition: background var(--transition-fast);
    min-height: var(--table-header-height);
    flex-wrap: nowrap;
  }

  .header-btn:hover {
    background: var(--color-bg-hover);
  }

  .header-name {
    font-weight: var(--font-weight-medium);
    color: var(--color-text-secondary);
    flex-shrink: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .header-type {
    font-weight: var(--font-weight-normal);
    color: var(--color-text-muted);
    font-size: 10px;
    flex-shrink: 2;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sort-indicator {
    font-size: 9px;
    color: var(--color-accent);
    flex-shrink: 0;
    line-height: 1;
  }

  .resize-handle {
    position: absolute;
    right: 0;
    top: 0;
    width: 6px;
    height: 100%;
    cursor: col-resize;
    background: transparent;
    transition: background var(--transition-fast);
    z-index: 2;
  }

  .resize-handle:hover {
    background: var(--color-accent);
    opacity: 0.4;
  }

  .header-cell.dragging {
    opacity: 0.4;
  }

  .header-cell.drag-over {
    box-shadow: inset 2px 0 0 var(--color-accent);
  }

  /* ── Filter row ─────────────────────────────────────────────────────────── */

  .filter-row {
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
  }

  .filter-cell {
    padding: var(--spacing-1) var(--spacing-1);
    background: var(--color-bg-secondary);
    border-right: 1px solid var(--color-border);
    box-sizing: border-box;
  }

  .filter-cell-inner {
    display: flex;
    align-items: center;
    position: relative;
  }

  .filter-input {
    width: 100%;
    padding: 2px var(--spacing-1);
    background: var(--color-bg-primary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-primary);
    outline: none;
    box-sizing: border-box;
    min-width: 0;
    transition: border-color var(--transition-fast);
  }

  .filter-input:focus {
    border-color: var(--color-accent);
  }

  .filter-input::placeholder {
    color: var(--color-text-muted);
    font-family: var(--font-family-ui);
    font-style: italic;
  }

  .filter-clear {
    position: absolute;
    right: 2px;
    top: 50%;
    transform: translateY(-50%);
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    padding: 0;
    background: var(--color-bg-tertiary);
    border: none;
    border-radius: 50%;
    font-size: 8px;
    color: var(--color-text-muted);
    cursor: pointer;
    line-height: 1;
    transition: background var(--transition-fast);
  }

  .filter-clear:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  /* ── Data rows ──────────────────────────────────────────────────────────── */

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
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    vertical-align: middle;
    box-sizing: border-box;
  }

  .data-cell.cell-editable {
    cursor: default;
  }

  .data-cell.cell-editable:hover {
    background: var(--color-bg-hover);
  }

  .data-cell.cell-pending {
    background: var(--color-warning-subtle) !important;
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

  /* ── Pagination bar ─────────────────────────────────────────────────────── */

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
