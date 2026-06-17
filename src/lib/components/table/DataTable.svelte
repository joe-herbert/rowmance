<!--
  DataTable — paginated, sortable, filterable, resizable, editable table.
  Phase 2: adds sorting, column filters, column resize, column visibility, and
  inline cell editing with pending-change tracking.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import type { ColumnMeta } from '$lib/types';
  import CellEditor from './CellEditor.svelte';

  type CellValue = string | number | boolean | null;
  type SortDir = 'asc' | 'desc' | 'none';

  export interface PageInfo {
    pageIndex: number;
    pageCount: number;
    pageOffset: number;
    pageRowsLength: number;
    processedRowsLength: number;
  }

  interface Props {
    columns: ColumnMeta[];
    rows: CellValue[][];
    pageSize?: number;
    pageIndex?: number;
    editable?: boolean;
    readOnly?: boolean;
    hiddenColumns?: Set<string>;
    totalRows?: number | null;
    rowOffset?: number;
    onChangePending?: (_changes: Map<string, Map<string, CellValue>>) => void;
    onCellSelect?: (_originalColIndex: number, _row: CellValue[]) => void;
    onDeselect?: () => void;
    onAddRow?: () => void;
    onPageInfo?: (_info: PageInfo) => void;
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
    pageIndex = $bindable(0),
    editable = false,
    readOnly: _readOnly = false,
    hiddenColumns = new Set<string>(),
    totalRows: _totalRows = null,
    rowOffset = 0,
    onChangePending,
    onCellSelect,
    onDeselect,
    onAddRow: _onAddRow,
    onPageInfo,
  }: Props = $props();

  // ── Column order (drag-to-reorder) ───────────────────────────────────────

  let columnOrder = $state<number[]>(untrack(() => columns.map((_, i) => i)));

  $effect(() => {
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

  let sortColIndex = $state(-1);
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

  let filterValues = $state<string[]>(untrack(() => columns.map(() => '')));

  $effect(() => {
    const len = columns.length;
    if (filterValues.length !== len) {
      filterValues = columns.map(() => '');
    }
  });

  // ── Column widths ─────────────────────────────────────────────────────────

  let colWidths = $state<number[]>(untrack(() => columns.map(() => 120)));

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

  const totalCount = $derived(_totalRows ?? processedRows.length);
  const pageCount = $derived(Math.max(1, Math.ceil(totalCount / pageSize)));
  const pageRows = $derived(
    processedRows.slice(pageIndex * pageSize, (pageIndex + 1) * pageSize),
  );

  const pageOffset = $derived(pageIndex * pageSize);

  $effect(() => {
    rows;
    filterValues.join('|');
    pageIndex = 0;
  });

  $effect(() => {
    onPageInfo?.({
      pageIndex,
      pageCount,
      pageOffset,
      pageRowsLength: pageRows.length,
      processedRowsLength: totalCount,
    });
  });

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

  function isRowPending(rowKey: string): boolean {
    const rowMap = pendingChanges.get(rowKey);
    return rowMap ? rowMap.size > 0 : false;
  }

  // ── Cell editor state ─────────────────────────────────────────────────────

  interface EditTarget {
    rowKey: string;
    colName: string;
    colIndex: number;
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

  // ── Keyboard cell navigation ──────────────────────────────────────────────

  let focusedCell = $state<{ row: number; col: number } | null>(null);

  function handleTableKeydown(e: KeyboardEvent): void {
    if (editTarget !== null) return;
    const target = e.target as HTMLElement;
    if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA') return;

    const rowCount = pageRows.length;
    const colCount = visibleColumns.length;
    if (rowCount === 0 || colCount === 0) return;

    if (['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Enter'].includes(e.key)) {
      e.preventDefault();
    }

    if (!focusedCell) {
      if (['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(e.key)) {
        focusedCell = { row: 0, col: 0 };
        scrollFocusedCellIntoView(focusedCell);
      }
      return;
    }

    let { row, col } = focusedCell;

    if (e.key === 'ArrowDown') {
      row = Math.min(row + 1, rowCount - 1);
    } else if (e.key === 'ArrowUp') {
      row = Math.max(row - 1, 0);
    } else if (e.key === 'ArrowRight') {
      col = Math.min(col + 1, colCount - 1);
    } else if (e.key === 'ArrowLeft') {
      col = Math.max(col - 1, 0);
    } else if (e.key === 'Enter') {
      if (editable) {
        const { originalIndex } = visibleColumns[col];
        const row_data = pageRows[row];
        if (row_data) {
          const fakeEvent = { currentTarget: getFocusedCellEl(row, col) } as unknown as MouseEvent;
          handleCellDblClick(fakeEvent, row_data, row, originalIndex);
        }
      }
      return;
    } else if (e.key === 'Escape') {
      cancelEdit();
      focusedCell = null;
      return;
    } else {
      return;
    }

    focusedCell = { row, col };
    scrollFocusedCellIntoView(focusedCell);
  }

  function getFocusedCellEl(row: number, col: number): HTMLTableCellElement | null {
    if (!tableContainerEl) return null;
    const tr = tableContainerEl.querySelector<HTMLTableRowElement>(
      `tbody tr:nth-child(${row + 1})`,
    );
    if (!tr) return null;
    // +2 to skip the row-number column
    return tr.querySelector<HTMLTableCellElement>(`td:nth-child(${col + 2})`);
  }

  function scrollFocusedCellIntoView(cell: { row: number; col: number }): void {
    requestAnimationFrame(() => {
      const el = getFocusedCellEl(cell.row, cell.col);
      el?.focus();
      el?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    });
  }

  // ── Cell formatting ───────────────────────────────────────────────────────

  function formatCell(value: CellValue): string {
    if (value === null) return '';
    return String(value);
  }

  // ── Data type categorisation ──────────────────────────────────────────────

  function getDataTypeCategory(dataType: string): 'number' | 'timestamp' | 'boolean' | 'json' | 'text' {
    const dt = dataType.toLowerCase();
    if (/^(int|int2|int4|int8|int16|bigint|smallint|float|float4|float8|real|double|numeric|decimal|serial|bigserial|money)/.test(dt)) return 'number';
    if (/^(timestamp|date|time|interval)/.test(dt)) return 'timestamp';
    if (/^bool/.test(dt)) return 'boolean';
    if (/^json/.test(dt)) return 'json';
    return 'text';
  }

  // ── Row selection ─────────────────────────────────────────────────────────

  let selectedRowKeys = $state<Set<string>>(new Set());

  function toggleRowSelection(rowKey: string): void {
    const next = new Set(selectedRowKeys);
    if (next.has(rowKey)) next.delete(rowKey);
    else next.add(rowKey);
    selectedRowKeys = next;
  }

  function handleRowClick(e: MouseEvent, rowKey: string): void {
    if (e.metaKey || e.ctrlKey) {
      toggleRowSelection(rowKey);
    } else {
      selectedRowKeys = new Set([rowKey]);
    }
  }

  // ── Context menu ──────────────────────────────────────────────────────────

  interface ContextMenu {
    x: number;
    y: number;
    rowKey: string;
    row: CellValue[];
  }

  let contextMenu = $state<ContextMenu | null>(null);

  function handleRowContextMenu(e: MouseEvent, row: CellValue[], rowIndex: number): void {
    e.preventDefault();
    const rowKey = buildRowKey(row, columns, pageOffset + rowIndex);
    contextMenu = { x: e.clientX, y: e.clientY, rowKey, row };
  }

  function dismissContextMenu(): void {
    contextMenu = null;
  }

  function copyRowTabSeparated(row: CellValue[]): void {
    const text = visibleColumns
      .map(({ originalIndex }) => {
        const val = row[originalIndex];
        return val === null ? '' : String(val);
      })
      .join('\t');
    navigator.clipboard.writeText(text).catch(() => {});
    dismissContextMenu();
  }

  function copySelectedRowsTabSeparated(): void {
    const lines: string[] = [];
    for (const key of selectedRowKeys) {
      const found = pageRows.find(
        (r, i) => buildRowKey(r, columns, pageOffset + i) === key,
      );
      if (found) {
        const text = visibleColumns
          .map(({ originalIndex }) => {
            const val = found[originalIndex];
            return val === null ? '' : String(val);
          })
          .join('\t');
        lines.push(text);
      }
    }
    navigator.clipboard.writeText(lines.join('\n')).catch(() => {});
    dismissContextMenu();
  }

  function handleContextMenuKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape') dismissContextMenu();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  bind:this={tableContainerEl}
  class="data-table-wrapper"
  onpointermove={onResizePointerMove}
  onpointerup={onResizePointerUp}
  onclick={dismissContextMenu}
  onkeydown={(e) => { handleContextMenuKeydown(e); handleTableKeydown(e); }}
>

  <div class="table-scroll">
    <table class="data-table">
      <thead>
        <tr class="header-row">
          <!-- Row number column header -->
          <th
            class="rownum-header-cell"
            onclick={() => { focusedCell = null; onDeselect?.(); }}
          >#</th>
          {#each visibleColumns as { col, originalIndex }}
            {@const isSorted = sortColIndex === originalIndex}
            {@const isDragging = draggingColName === col.name}
            {@const isDragOver = dragOverColName === col.name && draggingColName !== col.name}
            <th
              class="header-cell"
              class:dragging={isDragging}
              class:drag-over={isDragOver}
              style="width: {colWidths[originalIndex]}px; min-width: {colWidths[originalIndex]}px; max-width: {colWidths[originalIndex]}px;"
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
                {#if col.isPrimaryKey}
                  <svg class="pk-icon" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <circle cx="8" cy="9" r="4"></circle>
                    <path d="M11 12l7 7"></path>
                    <path d="M16 17l2-2"></path>
                  </svg>
                {/if}
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
                draggable={false}
                ondragstart={(e) => e.stopPropagation()}
                onpointerdown={(e) => onResizePointerDown(e, originalIndex)}
              ></div>
            </th>
          {/each}
        </tr>
      </thead>

      <tbody>
        {#each pageRows as row, rowIndex (rowIndex)}
          {@const processedRowIndex = rowIndex}
          {@const rowKey = buildRowKey(row, columns, pageOffset + processedRowIndex)}
          {@const isSelected = selectedRowKeys.has(rowKey)}
          {@const rowDirty = isRowPending(rowKey)}
          <tr
            class="data-row"
            class:row-selected={isSelected}
            onclick={(e) => handleRowClick(e, rowKey)}
            oncontextmenu={(e) => handleRowContextMenu(e, row, processedRowIndex)}
          >
            <!-- Row number / dirty indicator -->
            <td
              class="rownum-cell"
              onclick={() => { focusedCell = null; onDeselect?.(); }}
            >
              {#if rowDirty}
                <span class="row-dirty-dot" aria-label="Row has unsaved changes"></span>
              {:else}
                <span class="rownum">{rowOffset + pageOffset + rowIndex + 1}</span>
              {/if}
            </td>

            {#each visibleColumns as { col, originalIndex }, colIndex}
              {@const cellValue = getPendingValue(rowKey, col.name, row[originalIndex])}
              {@const isPending = hasPendingChange(rowKey, col.name)}
              {@const typeCategory = getDataTypeCategory(col.dataType)}
              <td
                class="data-cell"
                class:cell-number={typeCategory === 'number'}
                class:cell-timestamp={typeCategory === 'timestamp'}
                class:cell-editable={editable}
                class:cell-focused={focusedCell?.row === rowIndex && focusedCell?.col === colIndex}
                style="width: {colWidths[originalIndex]}px; min-width: {colWidths[originalIndex]}px; max-width: {colWidths[originalIndex]}px;"
                tabindex="0"
                ondblclick={(e) => handleCellDblClick(e, row, processedRowIndex, originalIndex)}
                onfocus={() => {
                  focusedCell = { row: rowIndex, col: colIndex };
                  selectedRowKeys = new Set([rowKey]);
                  onCellSelect?.(originalIndex, row);
                }}
              >
                {#if cellValue === null}
                  <span class="null-value">NULL</span>
                {:else if cellValue === ''}
                  <span class="empty-value">EMPTY</span>
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
                {#if isPending}
                  <span class="cell-dirty-dot" aria-label="Unsaved change"></span>
                {/if}
              </td>
            {/each}
          </tr>
        {/each}

        {#if processedRows.length === 0}
          <tr>
            <td class="empty-cell" colspan={visibleColumns.length + 1}>
              {rows.length === 0 ? 'No rows to display.' : 'No rows match the current filters.'}
            </td>
          </tr>
        {/if}
      </tbody>
    </table>
  </div>

  <!-- Context menu -->
  {#if contextMenu !== null}
    <div
      class="context-menu"
      role="menu"
      tabindex="-1"
      style="left: {contextMenu.x}px; top: {contextMenu.y}px;"
      onclick={(e) => e.stopPropagation()}
      onkeydown={handleContextMenuKeydown}
    >
      {#if selectedRowKeys.size > 1}
        <button
          class="context-menu-item"
          role="menuitem"
          onclick={() => copySelectedRowsTabSeparated()}
        >
          Copy {selectedRowKeys.size} selected rows (tab-separated)
        </button>
      {:else}
        <button
          class="context-menu-item"
          role="menuitem"
          onclick={() => copyRowTabSeparated(contextMenu!.row)}
        >
          Copy row (tab-separated)
        </button>
      {/if}
    </div>
  {/if}

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

</div>

<style>
  .data-table-wrapper {
    position: relative;
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    overflow: hidden;
  }

  /* ── Table scroll ────────────────────────────────────────────────────────── */

  .table-scroll {
    flex: 1;
    overflow: auto;
    scrollbar-color: var(--color-scrollbar-thumb) var(--color-scrollbar-track);
    scrollbar-width: thin;
  }

  .table-scroll::-webkit-scrollbar {
    width: 11px;
    height: 11px;
  }

  .table-scroll::-webkit-scrollbar-thumb {
    background: var(--color-scrollbar-thumb);
    border-radius: 9px;
    border: 3px solid transparent;
    background-clip: content-box;
  }

  .table-scroll::-webkit-scrollbar-track {
    background: transparent;
  }

  /* border-collapse: separate + border-spacing: 0 lets box-shadow work on <tr> */
  .data-table {
    border-collapse: separate;
    border-spacing: 0;
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    width: max-content;
    min-width: 100%;
  }

  /* ── Header ─────────────────────────────────────────────────────────────── */

  thead {
    position: sticky;
    top: 0;
    z-index: 2;
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
  }

  .header-row {
    height: 34px;
    background: var(--color-table-header-bg);
    border-bottom: 1px solid var(--color-border-strong);
    font-size: 11px;
    color: var(--color-text-secondary);
  }

  /* No border between # and first column — no border-right anywhere */
  .rownum-header-cell {
    width: 42px;
    min-width: 42px;
    max-width: 42px;
    text-align: center;
    font-size: 11px;
    color: var(--color-text-muted);
    font-weight: var(--font-weight-medium);
    vertical-align: middle;
    padding: 0;
    border-bottom: 1px solid var(--color-border-strong);
    box-sizing: border-box;
    cursor: pointer;
  }

  .rownum-header-cell:hover {
    color: var(--color-text-primary);
  }

  .header-cell {
    position: relative;
    padding: 0;
    font-size: 11px;
    text-align: left;
    color: var(--color-text-secondary);
    background: transparent;
    border-bottom: 1px solid var(--color-border-strong);
    white-space: nowrap;
    overflow: hidden;
    box-sizing: border-box;
    user-select: none;
  }

  .header-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    width: calc(100% - 6px);
    height: 34px;
    padding: 0 12px;
    background: transparent;
    border: none;
    cursor: pointer;
    text-align: left;
    color: inherit;
    font-size: inherit;
    font-family: inherit;
    transition: background var(--transition-fast);
    flex-wrap: nowrap;
  }

  .header-btn:hover {
    background: var(--color-bg-hover);
  }

  .pk-icon {
    color: var(--color-accent);
    flex-shrink: 0;
  }

  .header-name {
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-secondary);
    flex-shrink: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .header-type {
    font-family: var(--font-family-mono);
    font-size: 9.5px;
    color: var(--color-text-muted);
    font-weight: var(--font-weight-normal);
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

  /* ── Data rows ──────────────────────────────────────────────────────────── */

  .data-row {
    cursor: default;
    transition: background var(--transition-fast);
  }

  .data-row:hover {
    background: var(--color-bg-hover);
  }

  .data-row:hover .rownum {
    color: var(--color-text-primary);
  }

  /* box-shadow works on <tr> with border-collapse: separate */
  .data-row.row-selected {
    background: var(--color-accent-subtle);
  }

  .data-row.row-selected .rownum-cell {
    border-left: 2px solid var(--color-accent);
  }

  .data-row.row-selected:hover {
    background: var(--color-accent-subtle);
  }

  /* ── Row number column ──────────────────────────────────────────────────── */

  .rownum-cell {
    width: 42px;
    min-width: 42px;
    max-width: 42px;
    height: 38px;
    text-align: center;
    vertical-align: middle;
    border-bottom: 1px solid var(--color-border);
    box-sizing: border-box;
    padding: 0;
    cursor: pointer;
  }

  .rownum {
    font-size: 11px;
    font-family: var(--font-family-mono);
    color: var(--color-text-muted);
    user-select: none;
  }

  .row-dirty-dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--color-accent);
  }

  /* ── Data cells ─────────────────────────────────────────────────────────── */

  .data-cell {
    padding: 0 12px;
    height: 38px;
    border-bottom: 1px solid var(--color-border);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 12.5px;
    vertical-align: middle;
    box-sizing: border-box;
    max-width: 0;
  }

  .data-cell.cell-number {
    font-family: var(--font-family-mono);
    color: var(--color-editor-number);
  }

  .data-cell.cell-timestamp {
    font-family: var(--font-family-mono);
    color: var(--color-text-secondary);
    font-size: 12px;
  }

  .data-cell.cell-editable {
    cursor: pointer;
  }

  .data-cell:focus {
    outline: none;
  }

  .data-cell.cell-focused {
    background-color: color-mix(in srgb, var(--color-accent) 15%, transparent);
  }

  .cell-dirty-dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--color-accent);
    flex-shrink: 0;
    margin-left: 6px;
    vertical-align: middle;
  }

  .null-value {
    color: var(--color-null);
    font-style: italic;
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
  }

  .empty-value {
    color: var(--color-text-muted);
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

  /* ── Context menu ───────────────────────────────────────────────────────── */

  .context-menu {
    position: fixed;
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-overlay);
    z-index: 400;
    min-width: 220px;
    padding: var(--spacing-1) 0;
  }

  .context-menu-item {
    display: block;
    width: 100%;
    padding: var(--spacing-2) var(--spacing-3);
    background: transparent;
    border: none;
    text-align: left;
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    color: var(--color-text-primary);
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .context-menu-item:hover {
    background: var(--color-bg-hover);
  }
</style>
