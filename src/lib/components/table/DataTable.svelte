<!--
  DataTable — paginated, sortable, filterable, resizable, editable table.
  Phase 2: adds sorting, column filters, column resize, column visibility, and
  inline cell editing with pending-change tracking.
-->
<script lang="ts">
  import { untrack, onMount, tick } from 'svelte';
  import type { ColumnMeta } from '$lib/types';
  import { portal } from '$lib/actions/portal';
  import CellEditor from './CellEditor.svelte';
  import CellEditorModal from './CellEditorModal.svelte';

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
    onChangePending?: (_changes: Map<string, Map<string, CellValue>>, _originalRows: Map<string, CellValue[]>) => void;
    onCellSelect?: (_originalColIndex: number, _row: CellValue[]) => void;
    onDeselect?: () => void;
    onAddRow?: () => void;
    addRowTrigger?: number;
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
    readOnly = false,
    hiddenColumns = new Set<string>(),
    totalRows: _totalRows = null,
    rowOffset = 0,
    onChangePending,
    onCellSelect,
    onDeselect,
    onAddRow,
    addRowTrigger = 0,
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

  function computeDefaultColWidths(cols: ColumnMeta[], dataRows: CellValue[][], container: HTMLElement): number[] {
    type FontStyle = { fontFamily: string; fontSize: string; fontWeight: string; fontStyle: string; letterSpacing: string };

    function getElFontStyle(selector: string): FontStyle | null {
      const el = container.querySelector<HTMLElement>(selector);
      if (!el) return null;
      const s = getComputedStyle(el);
      return { fontFamily: s.fontFamily, fontSize: s.fontSize, fontWeight: s.fontWeight, fontStyle: s.fontStyle, letterSpacing: s.letterSpacing };
    }

    function applyFont(span: HTMLSpanElement, fs: FontStyle): void {
      span.style.fontFamily = fs.fontFamily;
      span.style.fontSize = fs.fontSize;
      span.style.fontWeight = fs.fontWeight;
      span.style.fontStyle = fs.fontStyle;
      span.style.letterSpacing = fs.letterSpacing;
    }

    const nameStyle = getElFontStyle('.header-name');
    const typeStyle = getElFontStyle('.header-type');
    const textCellStyle = getElFontStyle('.data-cell:not(.cell-number):not(.cell-timestamp) .cell-content');
    const numCellStyle = getElFontStyle('.cell-number .cell-content');
    const tsCellStyle = getElFontStyle('.cell-timestamp .cell-content');

    const colCellStyles = cols.map(col => {
      const cat = getDataTypeCategory(col.dataType);
      return cat === 'number' ? numCellStyle : cat === 'timestamp' ? tsCellStyle : textCellStyle;
    });

    // Build all measurement spans in one batch, read widths after a single layout pass
    const probe = document.createElement('div');
    probe.style.cssText = 'position:absolute;visibility:hidden;white-space:nowrap;left:-9999px;top:-9999px;pointer-events:none;';
    document.body.appendChild(probe);

    type Entry = { span: HTMLSpanElement; col: number; kind: 'name' | 'type' | 'cell' };
    const entries: Entry[] = [];

    function addSpan(text: string, fs: FontStyle | null, col: number, kind: 'name' | 'type' | 'cell'): void {
      const span = document.createElement('span');
      span.style.whiteSpace = 'nowrap';
      if (fs) applyFont(span, fs);
      span.textContent = text;
      probe.appendChild(span);
      entries.push({ span, col, kind });
    }

    cols.forEach((col, i) => {
      addSpan(col.name, nameStyle, i, 'name');
      if (col.dataType) addSpan(col.dataType, typeStyle, i, 'type');
    });

    for (const row of dataRows) {
      for (let i = 0; i < cols.length; i++) {
        const val = row[i];
        if (val != null) addSpan(String(val), colCellStyles[i], i, 'cell');
      }
    }

    // Single layout pass: first getBoundingClientRect triggers layout for all
    const nameWidths = new Float64Array(cols.length);
    const typeWidths = new Float64Array(cols.length);
    const maxCellWidths = new Float64Array(cols.length);

    for (const { span, col, kind } of entries) {
      const w = span.getBoundingClientRect().width;
      if (kind === 'name') nameWidths[col] = w;
      else if (kind === 'type') typeWidths[col] = w;
      else if (w > maxCellWidths[col]) maxCellWidths[col] = w;
    }

    document.body.removeChild(probe);

    // Measure actual overhead from rendered elements rather than hardcoding
    let cellOverhead = 24;
    const dataCellEl = container.querySelector<HTMLElement>('.data-cell');
    if (dataCellEl) {
      const s = getComputedStyle(dataCellEl);
      cellOverhead = parseFloat(s.paddingLeft) + parseFloat(s.paddingRight);
    }

    // Measure base overhead from any header cell (cell width minus label width)
    // and separately measure the extra overhead a PK icon adds, by comparing
    // a PK header cell directly against a non-PK one.
    const allHeaderCells = Array.from(container.querySelectorAll<HTMLElement>('.header-cell'));
    const pkHeaderCell = allHeaderCells.find(el => el.querySelector('.pk-icon')) ?? null;
    const nonPkHeaderCell = allHeaderCells.find(el => !el.querySelector('.pk-icon')) ?? null;

    function headerOverheadOf(cell: HTMLElement | null): number {
      if (!cell) return 30;
      const label = cell.querySelector<HTMLElement>('.header-label');
      if (!label) return 30;
      return cell.getBoundingClientRect().width - label.getBoundingClientRect().width;
    }

    const headerBaseOverhead = headerOverheadOf(nonPkHeaderCell ?? pkHeaderCell);
    const pkHeaderOverhead = pkHeaderCell ? headerOverheadOf(pkHeaderCell) : headerBaseOverhead;
    const pkIconOverhead = pkHeaderOverhead - headerBaseOverhead;

    return cols.map((col, i) => {
      // ceil individual measurements so fractional pixels don't trigger ellipsis
      const nameW = Math.ceil(nameWidths[i]);
      const typeW = col.dataType ? Math.ceil(typeWidths[i]) + 4 : 0;
      const pkExtra = col.isPrimaryKey ? pkIconOverhead : 0;
      const headerWidth = Math.ceil(nameW + typeW + headerBaseOverhead + pkExtra);
      const contentWidth = Math.ceil(maxCellWidths[i] + cellOverhead);
      return Math.min(Math.max(contentWidth, headerWidth), 300);
    });
  }

  let colWidths = $state<number[]>(untrack(() => columns.map(() => 120)));

  $effect(() => {
    const cols = columns;
    const dataRows = rows;
    if (colWidths.length !== cols.length) {
      tick().then(() => {
        if (tableContainerEl) colWidths = computeDefaultColWidths(cols, dataRows, tableContainerEl);
      });
    }
  });

  onMount(async () => {
    if (!tableContainerEl) return;
    colWidths = computeDefaultColWidths(columns, rows, tableContainerEl);

    await tick();

    // Verify: if any header label is still overflowing after the computed widths are applied,
    // expand that column by exactly the overflow amount. This catches any subpixel or
    // font-rendering gap between probe measurements and actual browser layout.
    const headerCells = tableContainerEl.querySelectorAll<HTMLElement>('.header-cell');
    headerCells.forEach((cell, domIndex) => {
      const label = cell.querySelector<HTMLElement>('.header-label');
      if (!label || label.scrollWidth <= label.clientWidth) return;
      const origIdx = visibleColumns[domIndex]?.originalIndex ?? -1;
      if (origIdx < 0) return;
      colWidths[origIdx] = Math.min(colWidths[origIdx] + (label.scrollWidth - label.clientWidth) + 1, 300);
    });
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
  // Snapshot of each row's DB values at the time of its first edit — used to
  // build all-columns WHERE clauses for tables without a primary key.
  let originalRows = $state<Map<string, CellValue[]>>(new Map());

  let pendingNewRows = $state<{ key: string }[]>([]);
  let nextNewRowId = 0;

  $effect(() => {
    const trigger = addRowTrigger;
    if (trigger === 0) return;

    untrack(() => {
      const id = nextNewRowId++;
      const key = `__new__${id}`;
      pendingNewRows = [...pendingNewRows, { key }];

      const updated = new Map(pendingChanges);
      updated.set(key, new Map());
      pendingChanges = updated;
      onChangePending?.(pendingChanges, originalRows);

      tick().then(() => {
        if (!tableContainerEl) return;
        const scrollEl = tableContainerEl.querySelector('.table-scroll') as HTMLElement | null;
        if (scrollEl) scrollEl.scrollTop = scrollEl.scrollHeight;

        const firstVisCol = visibleColumns[0];
        if (!firstVisCol) return;

        const newRowEl = tableContainerEl.querySelector(`[data-new-row-key="${key}"]`);
        if (!newRowEl) return;
        const firstTd = newRowEl.querySelectorAll('.data-cell')[0] as HTMLTableCellElement | undefined;
        if (!firstTd) return;

        const containerRect = tableContainerEl.getBoundingClientRect();
        const tdRect = firstTd.getBoundingClientRect();

        focusedCell = null;
        editTarget = {
          rowKey: key,
          colName: firstVisCol.col.name,
          colIndex: firstVisCol.originalIndex,
          value: null,
          originalValue: null,
          dataType: firstVisCol.col.dataType,
          top: tdRect.top - containerRect.top,
          left: tdRect.left - containerRect.left,
          width: Math.max(tdRect.width, 160),
          height: tdRect.height,
          containerHeight: containerRect.height,
        };
      });
    });
  });

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
    originalValue: CellValue;
    dataType: string;
    top: number;
    left: number;
    width: number;
    height: number;
    containerHeight: number;
  }

  let editTarget = $state<EditTarget | null>(null);
  let modalTarget = $state<EditTarget | null>(null);
  let tableContainerEl = $state<HTMLDivElement | null>(null);

  function handleCellDblClick(
    e: MouseEvent,
    row: CellValue[],
    processedRowIndex: number,
    originalColIndex: number,
  ): void {
    if (!editable || readOnly) return;
    const col = columns[originalColIndex];
    if (!col) return;

    const rowKey = buildRowKey(row, columns, pageOffset + processedRowIndex);
    const currentValue = getPendingValue(rowKey, col.name, row[originalColIndex]);

    if (!originalRows.has(rowKey)) {
      const next = new Map(originalRows);
      next.set(rowKey, [...row]);
      originalRows = next;
    }

    const td = e.currentTarget as HTMLTableCellElement;
    const containerRect = tableContainerEl!.getBoundingClientRect();
    const tdRect = td.getBoundingClientRect();

    editTarget = {
      rowKey,
      colName: col.name,
      colIndex: originalColIndex,
      value: currentValue,
      originalValue: row[originalColIndex],
      dataType: col.dataType,
      top: tdRect.top - containerRect.top,
      left: tdRect.left - containerRect.left,
      width: Math.max(tdRect.width, 160),
      height: tdRect.height,
      containerHeight: containerRect.height,
    };
  }

  function openModalEditor(
    row: CellValue[],
    processedRowIndex: number,
    originalColIndex: number,
  ): void {
    if (!editable || readOnly) return;
    const col = columns[originalColIndex];
    if (!col) return;
    const rowKey = buildRowKey(row, columns, pageOffset + processedRowIndex);
    const currentValue = getPendingValue(rowKey, col.name, row[originalColIndex]);

    if (!originalRows.has(rowKey)) {
      const next = new Map(originalRows);
      next.set(rowKey, [...row]);
      originalRows = next;
    }

    modalTarget = {
      rowKey,
      colName: col.name,
      colIndex: originalColIndex,
      value: currentValue,
      originalValue: row[originalColIndex],
      dataType: col.dataType,
      top: 0,
      left: 0,
      width: 0,
      height: 0,
      containerHeight: 0,
    };
  }

  function cellValuesEqual(a: CellValue, b: CellValue): boolean {
    if (a === b) return true;
    if (a === null || b === null) return false;
    if (typeof a === 'number' && typeof b === 'string') {
      const n = Number(b);
      return b.trim() !== '' && !isNaN(n) && n === a;
    }
    if (typeof b === 'number' && typeof a === 'string') {
      const n = Number(a);
      return a.trim() !== '' && !isNaN(n) && n === b;
    }
    return false;
  }

  function confirmEdit(newValue: CellValue): void {
    if (!editTarget) return;
    const { rowKey, colName, originalValue } = editTarget;
    const isNewRow = rowKey.startsWith('__new__');

    const updated = new Map(pendingChanges);

    if (isNewRow) {
      if (!updated.has(rowKey)) updated.set(rowKey, new Map());
      updated.get(rowKey)!.set(colName, newValue);
    } else {
      if (cellValuesEqual(newValue, originalValue)) {
        const rowMap = updated.get(rowKey);
        if (rowMap) {
          rowMap.delete(colName);
          if (rowMap.size === 0) {
            updated.delete(rowKey);
            const nextOrig = new Map(originalRows);
            nextOrig.delete(rowKey);
            originalRows = nextOrig;
          }
        }
      } else {
        if (!updated.has(rowKey)) updated.set(rowKey, new Map());
        updated.get(rowKey)!.set(colName, newValue);
      }
    }

    pendingChanges = updated;
    onChangePending?.(pendingChanges, originalRows);
    editTarget = null;
    if (!isNewRow) refocusCell();
  }

  function cancelEdit(): void {
    editTarget = null;
    refocusCell();
  }

  function confirmModalEdit(newValue: CellValue): void {
    if (!modalTarget) return;
    const { rowKey, colName, originalValue } = modalTarget;
    const isNewRow = rowKey.startsWith('__new__');
    const updated = new Map(pendingChanges);
    if (isNewRow) {
      if (!updated.has(rowKey)) updated.set(rowKey, new Map());
      updated.get(rowKey)!.set(colName, newValue);
    } else {
      if (cellValuesEqual(newValue, originalValue)) {
        const rowMap = updated.get(rowKey);
        if (rowMap) {
          rowMap.delete(colName);
          if (rowMap.size === 0) {
            updated.delete(rowKey);
            const nextOrig = new Map(originalRows);
            nextOrig.delete(rowKey);
            originalRows = nextOrig;
          }
        }
      } else {
        if (!updated.has(rowKey)) updated.set(rowKey, new Map());
        updated.get(rowKey)!.set(colName, newValue);
      }
    }
    pendingChanges = updated;
    onChangePending?.(pendingChanges, originalRows);
    modalTarget = null;
    if (!isNewRow) refocusCell();
  }

  function cancelModalEdit(): void {
    modalTarget = null;
    refocusCell();
  }

  function refocusCell(): void {
    if (!focusedCell) return;
    scrollFocusedCellIntoView(focusedCell);
  }

  // ── Shortcut: TABLE_EDIT_IN_MODAL ────────────────────────────────────────

  onMount(() => {
    function handleShortcutAction(e: Event): void {
      const action = (e as CustomEvent<{ action: string }>).detail?.action;
      if (action !== 'TABLE_EDIT_IN_MODAL') return;
      if (!editable || readOnly) return;
      if (!focusedCell) return;
      if (!tableContainerEl?.contains(document.activeElement)) return;

      const { row, col } = focusedCell;
      const { originalIndex } = visibleColumns[col] ?? {};
      if (originalIndex === undefined) return;
      const row_data = pageRows[row];
      if (row_data) openModalEditor(row_data, row, originalIndex);
    }

    window.addEventListener('shortcut-action', handleShortcutAction);
    return () => window.removeEventListener('shortcut-action', handleShortcutAction);
  });

  // ── Keyboard cell navigation ──────────────────────────────────────────────

  let focusedCell = $state<{ row: number; col: number } | null>(null);
  let anchorCell = $state<{ row: number; col: number } | null>(null);
  let isDraggingSelection = $state(false);
  // Non-reactive flag: prevents onfocus from resetting anchor during programmatic focus
  let skipNextFocusReset = false;

  function isCellInSelection(row: number, col: number): boolean {
    if (!anchorCell || !focusedCell) return false;
    const minRow = Math.min(anchorCell.row, focusedCell.row);
    const maxRow = Math.max(anchorCell.row, focusedCell.row);
    const minCol = Math.min(anchorCell.col, focusedCell.col);
    const maxCol = Math.max(anchorCell.col, focusedCell.col);
    return row >= minRow && row <= maxRow && col >= minCol && col <= maxCol;
  }

  function handleTableKeydown(e: KeyboardEvent): void {
    if (editTarget !== null) return;
    const target = e.target as HTMLElement;
    if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA') return;

    const rowCount = pageRows.length;
    const colCount = visibleColumns.length;
    if (rowCount === 0 || colCount === 0) return;

    if ((e.metaKey || e.ctrlKey) && e.key === 'c') {
      e.preventDefault();
      copySelection();
      return;
    }
    if ((e.metaKey || e.ctrlKey) && e.key === 'x') {
      e.preventDefault();
      cutSelection();
      return;
    }
    if ((e.metaKey || e.ctrlKey) && e.key === 'v') {
      e.preventDefault();
      pasteFromClipboard();
      return;
    }

    if (['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Enter'].includes(e.key)) {
      e.preventDefault();
    }

    if (!focusedCell) {
      if (['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(e.key)) {
        const newCell = { row: 0, col: 0 };
        anchorCell = newCell;
        focusedCell = newCell;
        skipNextFocusReset = true;
        scrollFocusedCellIntoView(newCell);
      }
      return;
    }

    let { row, col } = focusedCell;
    const isArrow = ['ArrowDown', 'ArrowUp', 'ArrowRight', 'ArrowLeft'].includes(e.key);

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
      anchorCell = null;
      focusedCell = null;
      return;
    } else {
      return;
    }

    if (isArrow) {
      if (e.shiftKey) {
        if (!anchorCell) anchorCell = focusedCell;
      } else {
        anchorCell = { row, col };
      }
      focusedCell = { row, col };
      skipNextFocusReset = true;
      scrollFocusedCellIntoView(focusedCell);
    }
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
    if (/^bool/.test(dt) || dt === 'tinyint(1)') return 'boolean';
    if (/^json/.test(dt)) return 'json';
    return 'text';
  }

  // ── Row selection ─────────────────────────────────────────────────────────

  let selectedRowKeys = $state<Set<string>>(new Set());

  $effect(() => {
    if (anchorCell && focusedCell) {
      const minRow = Math.min(anchorCell.row, focusedCell.row);
      const maxRow = Math.max(anchorCell.row, focusedCell.row);
      const newKeys = new Set<string>();
      for (let r = minRow; r <= maxRow; r++) {
        const rowData = pageRows[r];
        if (rowData) newKeys.add(buildRowKey(rowData, columns, pageOffset + r));
      }
      selectedRowKeys = newKeys;
    } else {
      selectedRowKeys = new Set();
    }
  });

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
    colName: string | null;
    isNewRow?: boolean;
  }

  let contextMenu = $state<ContextMenu | null>(null);
  let contextMenuClipboardHasContent = $state(false);

  function handleRowContextMenu(e: MouseEvent, row: CellValue[], rowIndex: number, colName: string | null = null): void {
    e.preventDefault();
    const rowKey = buildRowKey(row, columns, pageOffset + rowIndex);
    contextMenu = { x: e.clientX, y: e.clientY, rowKey, row, colName };
    contextMenuClipboardHasContent = false;
    navigator.clipboard.readText().then((t) => { contextMenuClipboardHasContent = t.length > 0; }).catch(() => {});
  }

  function dismissContextMenu(): void {
    contextMenu = null;
  }

  function discardCellEdit(): void {
    if (!contextMenu?.colName) return;
    const { rowKey, colName } = contextMenu;
    const updated = new Map(pendingChanges);
    const rowMap = updated.get(rowKey);
    if (rowMap) {
      rowMap.delete(colName);
      if (rowMap.size === 0) {
        updated.delete(rowKey);
        const nextOrig = new Map(originalRows);
        nextOrig.delete(rowKey);
        originalRows = nextOrig;
      }
    }
    pendingChanges = updated;
    onChangePending?.(pendingChanges, originalRows);
    dismissContextMenu();
  }

  function openModalFromContextMenu(): void {
    if (!contextMenu?.colName) return;
    const { row, rowKey, colName } = contextMenu;
    const rowIndex = pageRows.findIndex(
      (r, i) => buildRowKey(r, columns, pageOffset + i) === rowKey,
    );
    if (rowIndex < 0) { dismissContextMenu(); return; }
    const originalColIndex = columns.findIndex((c) => c.name === colName);
    if (originalColIndex < 0) { dismissContextMenu(); return; }
    const currentValue = getPendingValue(rowKey, colName, row[originalColIndex]);
    const col = columns[originalColIndex];

    if (!originalRows.has(rowKey)) {
      const next = new Map(originalRows);
      next.set(rowKey, [...row]);
      originalRows = next;
    }

    modalTarget = {
      rowKey,
      colName,
      colIndex: originalColIndex,
      value: currentValue,
      originalValue: row[originalColIndex],
      dataType: col.dataType,
      top: 0, left: 0, width: 0, height: 0, containerHeight: 0,
    };
    dismissContextMenu();
  }

  function handleNewRowCellDblClick(
    e: MouseEvent,
    newRowKey: string,
    currentValue: CellValue,
    col: ColumnMeta,
    originalColIndex: number,
  ): void {
    if (!editable || readOnly) return;
    const td = e.currentTarget as HTMLTableCellElement;
    const containerRect = tableContainerEl!.getBoundingClientRect();
    const tdRect = td.getBoundingClientRect();

    focusedCell = null;
    editTarget = {
      rowKey: newRowKey,
      colName: col.name,
      colIndex: originalColIndex,
      value: currentValue,
      originalValue: currentValue,
      dataType: col.dataType,
      top: tdRect.top - containerRect.top,
      left: tdRect.left - containerRect.left,
      width: Math.max(tdRect.width, 160),
      height: tdRect.height,
      containerHeight: containerRect.height,
    };
  }

  function deleteNewRow(key: string): void {
    pendingNewRows = pendingNewRows.filter((r) => r.key !== key);
    const updated = new Map(pendingChanges);
    updated.delete(key);
    pendingChanges = updated;
    onChangePending?.(pendingChanges, originalRows);
    dismissContextMenu();
  }

  // ── Copy / Cut / Paste ───────────────────────────────────────────────────

  function getSelectionRange(): { minRow: number; maxRow: number; minCol: number; maxCol: number } | null {
    if (!focusedCell) return null;
    const anchor = anchorCell ?? focusedCell;
    return {
      minRow: Math.min(anchor.row, focusedCell.row),
      maxRow: Math.max(anchor.row, focusedCell.row),
      minCol: Math.min(anchor.col, focusedCell.col),
      maxCol: Math.max(anchor.col, focusedCell.col),
    };
  }

  function selectionIsMultiCell(): boolean {
    const r = getSelectionRange();
    if (!r) return false;
    return r.minRow !== r.maxRow || r.minCol !== r.maxCol;
  }

  function getSelectionText(): string {
    const range = getSelectionRange();
    if (!range) return '';
    const { minRow, maxRow, minCol, maxCol } = range;
    const lines: string[] = [];
    for (let r = minRow; r <= maxRow; r++) {
      const rowData = pageRows[r];
      if (!rowData) continue;
      const rowKey = buildRowKey(rowData, columns, pageOffset + r);
      const cells: string[] = [];
      for (let c = minCol; c <= maxCol; c++) {
        const { originalIndex } = visibleColumns[c];
        const col = columns[originalIndex];
        const val = getPendingValue(rowKey, col.name, rowData[originalIndex]);
        cells.push(val === null ? '' : String(val));
      }
      lines.push(cells.join('\t'));
    }
    return lines.join('\n');
  }

  function copySelection(): void {
    const text = getSelectionText();
    navigator.clipboard.writeText(text).catch(() => {});
  }

  function applyPendingChange(rowKey: string, colName: string, originalValue: CellValue, newValue: CellValue): void {
    const isNewRow = rowKey.startsWith('__new__');
    const updated = new Map(pendingChanges);
    if (isNewRow) {
      if (!updated.has(rowKey)) updated.set(rowKey, new Map());
      updated.get(rowKey)!.set(colName, newValue);
    } else if (cellValuesEqual(newValue, originalValue)) {
      const rowMap = updated.get(rowKey);
      if (rowMap) {
        rowMap.delete(colName);
        if (rowMap.size === 0) {
          updated.delete(rowKey);
          const nextOrig = new Map(originalRows);
          nextOrig.delete(rowKey);
          originalRows = nextOrig;
        }
      }
    } else {
      if (!updated.has(rowKey)) updated.set(rowKey, new Map());
      updated.get(rowKey)!.set(colName, newValue);
    }
    pendingChanges = updated;
  }

  function cutSelection(): void {
    if (!editable || readOnly) return;
    const range = getSelectionRange();
    if (!range) return;
    copySelection();
    const { minRow, maxRow, minCol, maxCol } = range;
    for (let r = minRow; r <= maxRow; r++) {
      const rowData = pageRows[r];
      if (!rowData) continue;
      const rowKey = buildRowKey(rowData, columns, pageOffset + r);
      if (!originalRows.has(rowKey)) {
        const next = new Map(originalRows);
        next.set(rowKey, [...rowData]);
        originalRows = next;
      }
      for (let c = minCol; c <= maxCol; c++) {
        const { originalIndex } = visibleColumns[c];
        const col = columns[originalIndex];
        if (col) applyPendingChange(rowKey, col.name, rowData[originalIndex], null);
      }
    }
    onChangePending?.(pendingChanges, originalRows);
  }

  async function pasteFromClipboard(): Promise<void> {
    if (!editable || readOnly) return;
    if (!focusedCell) return;
    let text: string;
    try {
      text = await navigator.clipboard.readText();
    } catch {
      return;
    }
    if (!text) return;
    const range = getSelectionRange();
    if (!range) return;
    // Flatten clipboard into a linear list, looped across selected cells
    const clipValues = text.split('\n').flatMap((line) => line.split('\t'));
    if (clipValues.length === 0) return;
    const { minRow, maxRow, minCol, maxCol } = range;
    let clipIndex = 0;
    for (let r = minRow; r <= maxRow; r++) {
      const rowData = pageRows[r];
      if (!rowData) continue;
      const rowKey = buildRowKey(rowData, columns, pageOffset + r);
      if (!originalRows.has(rowKey)) {
        const next = new Map(originalRows);
        next.set(rowKey, [...rowData]);
        originalRows = next;
      }
      for (let c = minCol; c <= maxCol; c++) {
        const { originalIndex } = visibleColumns[c];
        const col = columns[originalIndex];
        if (!col) continue;
        const pasteValue = clipValues[clipIndex % clipValues.length];
        clipIndex++;
        applyPendingChange(rowKey, col.name, rowData[originalIndex], pasteValue === '' ? null : pasteValue);
      }
    }
    onChangePending?.(pendingChanges, originalRows);
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

  function handleWindowClick(e: MouseEvent): void {
    if (!(e.target as Element | null)?.closest('.context-menu')) dismissContextMenu();
  }
</script>

<svelte:window onclick={handleWindowClick} onmouseup={() => { isDraggingSelection = false; }} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  bind:this={tableContainerEl}
  class="data-table-wrapper"
  onpointermove={onResizePointerMove}
  onpointerup={onResizePointerUp}
  onkeydown={(e) => { handleContextMenuKeydown(e); handleTableKeydown(e); }}
  onfocusout={(e) => {
    if (!tableContainerEl?.contains(e.relatedTarget as Node | null)) {
      anchorCell = null;
      focusedCell = null;
      isDraggingSelection = false;
    }
  }}
>

  <div class="table-scroll" class:selecting={isDraggingSelection} ondblclick={(e) => { if (editable && !readOnly && !(e.target as Element).closest('tr')) onAddRow?.(); }}>
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
                <span class="header-label">
                  <span class="header-name">{col.name}</span>
                  <span class="header-type">{col.dataType}</span>
                </span>
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
              {@const isRequiredEmpty = isPending && cellValue === null && !col.nullable && !col.isAutoIncrement && col.defaultValue == null}
              <td
                class="data-cell"
                class:cell-number={typeCategory === 'number'}
                class:cell-timestamp={typeCategory === 'timestamp'}
                class:cell-editable={editable && !readOnly}
                class:cell-selected={isCellInSelection(rowIndex, colIndex)}
                class:cell-focused={focusedCell?.row === rowIndex && focusedCell?.col === colIndex}
                class:cell-required-empty={isRequiredEmpty}
                style="width: {colWidths[originalIndex]}px; min-width: {colWidths[originalIndex]}px; max-width: {colWidths[originalIndex]}px;"
                tabindex="0"
                ondblclick={(e) => handleCellDblClick(e, row, processedRowIndex, originalIndex)}
                oncontextmenu={(e) => { e.stopPropagation(); if (!isCellInSelection(rowIndex, colIndex)) { anchorCell = { row: rowIndex, col: colIndex }; focusedCell = { row: rowIndex, col: colIndex }; } handleRowContextMenu(e, row, processedRowIndex, col.name); }}
                onmousedown={(e) => {
                  if (e.button === 2 || (e.button === 0 && e.ctrlKey)) {
                    if (isCellInSelection(rowIndex, colIndex)) skipNextFocusReset = true;
                    return;
                  }
                  if (e.button !== 0) return;
                  if (e.shiftKey && focusedCell) {
                    skipNextFocusReset = true;
                    focusedCell = { row: rowIndex, col: colIndex };
                  } else {
                    skipNextFocusReset = true;
                    anchorCell = { row: rowIndex, col: colIndex };
                    focusedCell = { row: rowIndex, col: colIndex };
                    isDraggingSelection = true;
                  }
                }}
                onmouseenter={() => {
                  if (isDraggingSelection) focusedCell = { row: rowIndex, col: colIndex };
                }}
                onfocus={() => {
                  if (skipNextFocusReset) {
                    skipNextFocusReset = false;
                    onCellSelect?.(originalIndex, row);
                    return;
                  }
                  anchorCell = { row: rowIndex, col: colIndex };
                  focusedCell = { row: rowIndex, col: colIndex };
                  onCellSelect?.(originalIndex, row);
                }}
              >
                <div class="cell-inner">
                  <span class="cell-content">
                    {#if cellValue === null}
                      <span class="null-value">NULL</span>
                    {:else if cellValue === ''}
                      <span class="empty-value">EMPTY</span>
                    {:else if typeCategory === 'boolean' && (typeof cellValue === 'boolean' || typeof cellValue === 'number')}
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
                  </span>
                  {#if isPending}
                    <span class="cell-dirty-dot" aria-label="Unsaved change"></span>
                  {/if}
                </div>
              </td>
            {/each}
          </tr>
        {/each}

        {#each pendingNewRows as newRow}
          <tr class="data-row new-row" data-new-row-key={newRow.key}>
            <td class="rownum-cell">
              <span class="new-row-indicator" aria-label="New row">+</span>
            </td>
            {#each visibleColumns as { col, originalIndex }}
              {@const isExplicitlySet = pendingChanges.get(newRow.key)?.has(col.name) ?? false}
              {@const currentValue = pendingChanges.get(newRow.key)?.get(col.name) ?? null}
              {@const isRequiredEmpty = !col.nullable && !col.isAutoIncrement && col.defaultValue == null && !isExplicitlySet}
              {@const typeCategory = getDataTypeCategory(col.dataType)}
              <td
                class="data-cell"
                class:cell-number={typeCategory === 'number'}
                class:cell-timestamp={typeCategory === 'timestamp'}
                class:cell-editable={editable && !readOnly}
                class:cell-required-empty={isRequiredEmpty}
                style="width: {colWidths[originalIndex]}px; min-width: {colWidths[originalIndex]}px; max-width: {colWidths[originalIndex]}px;"
                tabindex="0"
                ondblclick={(e) => handleNewRowCellDblClick(e, newRow.key, currentValue, col, originalIndex)}
                oncontextmenu={(e) => { e.preventDefault(); e.stopPropagation(); contextMenu = { x: e.clientX, y: e.clientY, rowKey: newRow.key, row: [], colName: col.name, isNewRow: true }; }}
                onfocus={() => { focusedCell = null; }}
              >
                <div class="cell-inner">
                  <span class="cell-content">
                    {#if currentValue === null}
                      <span class="null-value">NULL</span>
                    {:else if currentValue === ''}
                      <span class="empty-value">EMPTY</span>
                    {:else if typeCategory === 'boolean' && (typeof currentValue === 'boolean' || typeof currentValue === 'number')}
                      <span class="bool-value" class:bool-true={currentValue} class:bool-false={!currentValue}>
                        {currentValue ? '✓' : '✗'}
                      </span>
                    {:else}
                      {formatCell(currentValue)}
                    {/if}
                  </span>
                </div>
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
      use:portal
    >
      {#if contextMenu.isNewRow}
        <button
          class="context-menu-item context-menu-item-danger"
          role="menuitem"
          onclick={() => deleteNewRow(contextMenu!.rowKey)}
        >
          Delete new row
        </button>
      {:else}
        {#if contextMenu.colName && editable && !readOnly}
          <button
            class="context-menu-item"
            role="menuitem"
            onclick={() => openModalFromContextMenu()}
          >
            Edit in modal
          </button>
          <div class="context-menu-separator"></div>
        {/if}
        {#if contextMenu.colName && hasPendingChange(contextMenu.rowKey, contextMenu.colName)}
          <button
            class="context-menu-item context-menu-item-danger"
            role="menuitem"
            onclick={() => discardCellEdit()}
          >
            Discard edit
          </button>
          <div class="context-menu-separator"></div>
        {/if}
        {#if focusedCell !== null}
          <button
            class="context-menu-item"
            role="menuitem"
            onclick={() => { copySelection(); dismissContextMenu(); }}
          >
            {selectionIsMultiCell() ? 'Copy selection' : 'Copy cell'}
          </button>
          {#if editable && !readOnly}
            <button
              class="context-menu-item"
              role="menuitem"
              onclick={() => { cutSelection(); dismissContextMenu(); }}
            >
              {selectionIsMultiCell() ? 'Cut selection' : 'Cut cell'}
            </button>
            <button
              class="context-menu-item"
              role="menuitem"
              disabled={!contextMenuClipboardHasContent}
              onclick={() => { pasteFromClipboard(); dismissContextMenu(); }}
            >
              Paste
            </button>
          {/if}
          <div class="context-menu-separator"></div>
        {/if}
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
      {/if}
    </div>
  {/if}

  <!-- Inline cell editor overlay -->
  {#if editTarget !== null}
    <CellEditor
      value={editTarget.value}
      originalValue={editTarget.originalValue}
      dataType={editTarget.dataType}
      top={editTarget.top}
      left={editTarget.left}
      width={editTarget.width}
      height={editTarget.height}
      containerHeight={editTarget.containerHeight}
      onConfirm={confirmEdit}
      onCancel={cancelEdit}
    />
  {/if}

</div>

<!-- Modal cell editor (portal, full-screen) -->
{#if modalTarget !== null}
  <CellEditorModal
    value={modalTarget.value}
    originalValue={modalTarget.originalValue}
    colName={modalTarget.colName}
    dataType={modalTarget.dataType}
    onConfirm={confirmModalEdit}
    onCancel={cancelModalEdit}
  />
{/if}

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

  .header-label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .header-name {
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-secondary);
  }

  .header-type {
    font-family: var(--font-family-mono);
    font-size: 9.5px;
    color: var(--color-text-muted);
    font-weight: var(--font-weight-normal);
    margin-left: 4px;
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
    font-size: 12.5px;
    vertical-align: middle;
    box-sizing: border-box;
    max-width: 0;
    overflow: hidden;
  }

  .cell-inner {
    display: flex;
    align-items: center;
    overflow: hidden;
    height: 100%;
  }

  .cell-content {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: pre;
    min-width: 0;
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

  .data-cell.cell-selected {
    background-color: color-mix(in srgb, var(--color-accent) 12%, transparent);
  }

  .data-cell.cell-focused {
    background-color: color-mix(in srgb, var(--color-accent) 20%, transparent);
    outline: 1px solid color-mix(in srgb, var(--color-accent) 60%, transparent);
    outline-offset: -1px;
  }

  .table-scroll.selecting {
    user-select: none;
  }

  .cell-dirty-dot {
    flex-shrink: 0;
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--color-accent);
    margin-left: 5px;
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
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
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

  .context-menu-item-danger {
    color: var(--color-danger);
  }

  .context-menu-item-danger:hover {
    background: var(--color-danger-subtle);
  }

  .context-menu-separator {
    height: 1px;
    background: var(--color-border);
    margin: var(--spacing-1) 0;
  }

  /* ── New row (pending insert) ───────────────────────────────────────────── */

  .new-row {
    background: color-mix(in srgb, var(--color-success, #22c55e) 5%, transparent);
  }

  .new-row:hover {
    background: color-mix(in srgb, var(--color-success, #22c55e) 10%, transparent);
  }

  .new-row-indicator {
    font-size: 13px;
    font-weight: var(--font-weight-semibold);
    color: var(--color-success, #22c55e);
    user-select: none;
  }

  .cell-required-empty {
    background: color-mix(in srgb, var(--color-danger, #ef4444) 12%, transparent);
    box-shadow: inset 2px 0 0 var(--color-danger, #ef4444);
  }
</style>
