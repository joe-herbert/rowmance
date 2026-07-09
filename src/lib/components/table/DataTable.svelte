<!--
  DataTable — paginated, sortable, filterable, resizable, editable table.
  Phase 2: adds sorting, column filters, column resize, column visibility, and
  inline cell editing with pending-change tracking.
-->
<script lang="ts">
  import { untrack, onMount, tick } from 'svelte';
  import type { ColumnMeta } from '$lib/types';
  import CtxMenuContainer from '$lib/components/ui/ContextMenu.svelte';
  import CtxItem from '$lib/components/ui/CtxItem.svelte';
  import CtxSep from '$lib/components/ui/CtxSep.svelte';
  import CellEditor from './CellEditor.svelte';
  import CellEditorModal from './CellEditorModal.svelte';
  import CellViewModal from './CellViewModal.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import { useShortcuts, keyEventToString } from '$lib/stores/shortcuts.svelte';
  import { executeQuery } from '$lib/tauri/query';

  type CellValue = string | number | boolean | null;
  type SortDir = 'asc' | 'desc' | 'none';

  export interface QuickViewData {
    tableName: string;
    refColumn: string;
    refValue: CellValue;
    columns: ColumnMeta[];
    row: CellValue[] | null;
  }

  // Ensures only one context menu is open across all DataTable instances at a time.
  let activeMenuDismiss: (() => void) | null = null;

  export interface PageInfo {
    pageIndex: number;
    pageCount: number | null;
    pageOffset: number;
    pageRowsLength: number;
    processedRowsLength: number | null;
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
    onChangePending?: (
      _changes: Map<string, Map<string, CellValue>>,
      _originalRows: Map<string, CellValue[]>,
    ) => void;
    onCellSelect?: (_originalColIndex: number, _row: CellValue[]) => void;
    onRowSelect?: (_row: CellValue[], _columns: ColumnMeta[]) => void;
    onDeselect?: () => void;
    onAddRow?: () => void;
    addRowTrigger?: number;
    onPageInfo?: (_info: PageInfo) => void;
    tableName?: string;
    onDeleteRow?: (_row: CellValue[], _rowKey: string) => void;
    onCloneRow?: (_row: CellValue[]) => void;
    onDeleteRowsPending?: (_deletedRows: Map<string, CellValue[]>) => void;
    onForeignKeyClick?: (_colName: string, _value: CellValue) => void;
    onForeignKeyQuickView?: (_colName: string, _value: CellValue) => Promise<QuickViewData | null>;
    onConnectColumn?: (_colName: string) => void;
    initialColWidths?: Record<string, number>;
    initialColumnOrder?: string[];
    columnOrderOverride?: string[];
    onColWidthsChange?: (_widths: Record<string, number>) => void;
    onColumnOrderChange?: (_order: string[]) => void;
    initialPendingChanges?: Map<string, Map<string, CellValue>>;
    initialOriginalRows?: Map<string, CellValue[]>;
    initialDeletedRows?: Map<string, CellValue[]>;
    connectionId?: string;
    database?: string | null;
    columnRenames?: Record<string, string>;
    onRenameColumn?: (_colName: string, _label: string) => void;
    searchTerm?: string;
  }

  // ── Pure helper functions (exported for tests) ────────────────────────────

  export function sortRows(rows: CellValue[][], colIndex: number, dir: SortDir): CellValue[][] {
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

  export function filterRows(rows: CellValue[][], filters: string[]): CellValue[][] {
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

  export function buildRowKey(row: CellValue[], columns: ColumnMeta[], rowIndex: number): string {
    const pkCols = columns.map((col, idx) => ({ col, idx })).filter(({ col }) => col.isPrimaryKey);

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
    onRowSelect,
    onDeselect,
    onAddRow,
    addRowTrigger = 0,
    onPageInfo,
    tableName,
    onDeleteRow: _onDeleteRow,
    onCloneRow,
    onDeleteRowsPending,
    onForeignKeyClick,
    onForeignKeyQuickView,
    onConnectColumn,
    initialColWidths,
    initialColumnOrder,
    columnOrderOverride,
    onColWidthsChange,
    onColumnOrderChange,
    initialPendingChanges,
    initialOriginalRows,
    initialDeletedRows,
    connectionId,
    database,
    columnRenames = {},
    onRenameColumn,
    searchTerm = '',
  }: Props = $props();

  // ── Column order (drag-to-reorder) ───────────────────────────────────────

  let columnOrder = $state<number[]>(untrack(() => columns.map((_, i) => i)));

  $effect(() => {
    if (columnOrder.length !== columns.length) {
      columnOrder = columns.map((_, i) => i);
    }
  });

  $effect(() => {
    const override = columnOrderOverride;
    if (!override || override.length === 0) return;
    const nameToIdx = new Map(columns.map((c, i) => [c.name, i] as const));
    const order: number[] = [];
    for (const name of override) {
      const idx = nameToIdx.get(name);
      if (idx !== undefined) order.push(idx);
    }
    for (let i = 0; i < columns.length; i++) {
      if (!order.includes(i)) order.push(i);
    }
    if (order.length === columns.length) columnOrder = order;
  });

  let colDragName = $state<string | null>(null);
  let colIsDragging = $state(false);
  let colDropTarget = $state<{ name: string; position: 'before' | 'after' } | null>(null);
  let colPointerStartX = 0;

  $effect(() => {
    if (!colDragName) return;

    let lastPointerX = colPointerStartX;
    let scrollRafId: number | null = null;

    function scrollStep() {
      if (colIsDragging && tableScrollEl) {
        const EDGE = 60;
        const MAX_SPEED = 12;
        const rect = tableScrollEl.getBoundingClientRect();
        const distLeft = lastPointerX - rect.left;
        const distRight = rect.right - lastPointerX;
        if (distLeft < EDGE && distLeft >= 0) {
          tableScrollEl.scrollLeft -= Math.round(MAX_SPEED * (1 - distLeft / EDGE));
        } else if (distRight < EDGE && distRight >= 0) {
          tableScrollEl.scrollLeft += Math.round(MAX_SPEED * (1 - distRight / EDGE));
        }
      }
      scrollRafId = requestAnimationFrame(scrollStep);
    }
    scrollRafId = requestAnimationFrame(scrollStep);

    function onMove(e: PointerEvent) {
      if (!colIsDragging && Math.abs(e.clientX - colPointerStartX) > 4) {
        colIsDragging = true;
      }
      if (!colIsDragging) return;

      lastPointerX = e.clientX;

      const el = document.elementFromPoint(e.clientX, e.clientY);
      const th = el?.closest<HTMLElement>('[data-col-name]');
      const targetName = th?.dataset.colName;

      if (!targetName || targetName === colDragName) {
        colDropTarget = null;
        return;
      }

      const rect = th!.getBoundingClientRect();
      const position = e.clientX < rect.left + rect.width / 2 ? 'before' : 'after';
      colDropTarget = { name: targetName, position };
    }

    function onUp() {
      if (colIsDragging && colDropTarget) {
        const fromOriginal = columns.findIndex((c) => c.name === colDragName);
        const toOriginal = columns.findIndex((c) => c.name === colDropTarget!.name);
        const fromDisplay = columnOrder.indexOf(fromOriginal);
        const toDisplay = columnOrder.indexOf(toOriginal);
        let toIdx = toDisplay + (colDropTarget.position === 'after' ? 1 : 0);
        if (fromDisplay < toDisplay) toIdx -= 1;
        columnOrder = reorderColumns(columnOrder, fromDisplay, toIdx);
        if (onColumnOrderChange) {
          onColumnOrderChange(columnOrder.map((i) => columns[i].name));
        }
      }
      colDragName = null;
      colIsDragging = false;
      colDropTarget = null;
    }

    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);

    return () => {
      if (scrollRafId !== null) cancelAnimationFrame(scrollRafId);
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };
  });

  function onColHeaderPointerDown(e: PointerEvent, colName: string): void {
    if (e.button !== 0) return;
    if ((e.target as HTMLElement).closest('.resize-handle')) return;
    colPointerStartX = e.clientX;
    colDragName = colName;
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

  function computeDefaultColWidths(
    cols: ColumnMeta[],
    dataRows: CellValue[][],
    container: HTMLElement,
  ): number[] {
    type FontStyle = {
      fontFamily: string;
      fontSize: string;
      fontWeight: string;
      fontStyle: string;
      letterSpacing: string;
    };

    function getElFontStyle(selector: string): FontStyle | null {
      const el = container.querySelector<HTMLElement>(selector);
      if (!el) return null;
      const s = getComputedStyle(el);
      return {
        fontFamily: s.fontFamily,
        fontSize: s.fontSize,
        fontWeight: s.fontWeight,
        fontStyle: s.fontStyle,
        letterSpacing: s.letterSpacing,
      };
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
    const textCellStyle = getElFontStyle(
      '.data-cell:not(.cell-number):not(.cell-timestamp) .cell-content',
    );
    const numCellStyle = getElFontStyle('.cell-number .cell-content');
    const tsCellStyle = getElFontStyle('.cell-timestamp .cell-content');

    const colCellStyles = cols.map((col) => {
      const cat = getDataTypeCategory(col.dataType);
      return cat === 'number' ? numCellStyle : cat === 'timestamp' ? tsCellStyle : textCellStyle;
    });

    // Build all measurement spans in one batch, read widths after a single layout pass
    const probe = document.createElement('div');
    probe.style.cssText =
      'position:absolute;visibility:hidden;white-space:nowrap;left:-9999px;top:-9999px;pointer-events:none;';
    document.body.appendChild(probe);

    type Entry = { span: HTMLSpanElement; col: number; kind: 'name' | 'type' | 'cell' };
    const entries: Entry[] = [];

    function addSpan(
      text: string,
      fs: FontStyle | null,
      col: number,
      kind: 'name' | 'type' | 'cell',
    ): void {
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
    const pkHeaderCell = allHeaderCells.find((el) => el.querySelector('.pk-icon')) ?? null;
    const nonPkHeaderCell = allHeaderCells.find((el) => !el.querySelector('.pk-icon')) ?? null;

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

    // Apply saved column widths (override computed defaults per column)
    if (initialColWidths) {
      colWidths = colWidths.map((w, i) => {
        const name = columns[i]?.name;
        return name !== undefined && initialColWidths![name] != null ? initialColWidths![name] : w;
      });
    }

    // Apply saved column order
    if (initialColumnOrder && initialColumnOrder.length > 0) {
      const nameToIdx = new Map(columns.map((c, i) => [c.name, i] as const));
      const order: number[] = [];
      for (const name of initialColumnOrder) {
        const idx = nameToIdx.get(name);
        if (idx !== undefined) order.push(idx);
      }
      for (let i = 0; i < columns.length; i++) {
        if (!order.includes(i)) order.push(i);
      }
      if (order.length === columns.length) columnOrder = order;
    }

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
      colWidths[origIdx] = Math.min(
        colWidths[origIdx] + (label.scrollWidth - label.clientWidth) + 1,
        300,
      );
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
    if (resizingColIndex !== null && onColWidthsChange) {
      const widths: Record<string, number> = {};
      columns.forEach((col, i) => {
        widths[col.name] = colWidths[i];
      });
      onColWidthsChange(widths);
    }
    resizingColIndex = null;
  }

  // ── Derived: sorted + filtered rows ──────────────────────────────────────

  const processedRows = $derived.by(() => {
    const sorted = sortRows(rows, sortColIndex, sortDir);
    return filterRows(sorted, filterValues);
  });

  // ── Pagination ────────────────────────────────────────────────────────────

  // Use raw rows.length (server page size) to detect last page, not processedRows which
  // can be smaller due to client-side filtering even when more server pages exist.
  const totalCount = $derived<number | null>(
    _totalRows !== null ? _totalRows : rows.length < pageSize ? rowOffset + rows.length : null,
  );
  const pageCount = $derived<number | null>(
    totalCount !== null ? Math.max(1, Math.ceil(totalCount / pageSize)) : null,
  );
  const pageRows = $derived(processedRows.slice(pageIndex * pageSize, (pageIndex + 1) * pageSize));

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

  let pendingChanges = $state<Map<string, Map<string, CellValue>>>(
    untrack(() => (initialPendingChanges ? new Map(initialPendingChanges) : new Map())),
  );
  // Snapshot of each row's DB values at the time of its first edit — used to
  // build all-columns WHERE clauses for tables without a primary key.
  let originalRows = $state<Map<string, CellValue[]>>(
    untrack(() => (initialOriginalRows ? new Map(initialOriginalRows) : new Map())),
  );

  let pendingNewRows = $state<{ key: string }[]>(
    untrack(() =>
      initialPendingChanges
        ? [...initialPendingChanges.keys()]
            .filter((k) => k.startsWith('__new__'))
            .map((key) => ({ key }))
        : [],
    ),
  );
  let nextNewRowId = 0;

  // rowKey → original row values snapshot for building DELETE WHERE clauses
  let pendingDeletedRows = $state<Map<string, CellValue[]>>(
    untrack(() => (initialDeletedRows ? new Map(initialDeletedRows) : new Map())),
  );

  // When rows reload, clear pending changes whose value now matches the DB value.
  // This handles the case where the same edit was saved via another panel (e.g. query
  // editor), making the table view's pending change redundant.
  $effect(() => {
    const currentProcessedRows = processedRows;
    const currentColumns = columns;
    untrack(() => {
      if (pendingChanges.size === 0) return;

      let changed = false;
      const updatedChanges = new Map(pendingChanges);
      const updatedOriginals = new Map(originalRows);

      for (let j = 0; j < currentProcessedRows.length; j++) {
        const row = currentProcessedRows[j];
        const rowKey = buildRowKey(row, currentColumns, j);
        const colMap = updatedChanges.get(rowKey);
        if (!colMap) continue;

        const colsToRemove: string[] = [];
        for (const [colName, pendingVal] of colMap) {
          const colIdx = currentColumns.findIndex((c) => c.name === colName);
          if (colIdx < 0) continue;
          const dbVal = row[colIdx];
          if (cellValuesEqual(pendingVal, dbVal)) {
            colsToRemove.push(colName);
          }
        }

        for (const colName of colsToRemove) {
          colMap.delete(colName);
          changed = true;
        }
        if (colMap.size === 0) {
          updatedChanges.delete(rowKey);
          updatedOriginals.delete(rowKey);
        }
      }

      if (changed) {
        pendingChanges = updatedChanges;
        originalRows = updatedOriginals;
        onChangePending?.(pendingChanges, originalRows);
      }
    });
  });

  $effect(() => {
    const trigger = addRowTrigger;
    if (trigger === 0) return;

    untrack(() => {
      const id = nextNewRowId++;
      const key = `__new__${id}`;
      const addAtTop = settings.newRowPosition === 'top';
      pendingNewRows = addAtTop ? [{ key }, ...pendingNewRows] : [...pendingNewRows, { key }];

      const updated = new Map(pendingChanges);
      const newRowMap = new Map<string, CellValue>();
      for (const col of columns) {
        if (col.isAutoIncrement) continue;
        if (col.defaultValue != null) {
          newRowMap.set(col.name, parseDefaultValue(col.defaultValue, col.dataType));
        } else if (!col.nullable) {
          newRowMap.set(col.name, '');
        }
      }
      updated.set(key, newRowMap);
      pendingChanges = updated;
      onChangePending?.(pendingChanges, originalRows);

      tick().then(() => {
        if (!tableContainerEl) return;
        const scrollEl = tableContainerEl.querySelector('.table-scroll') as HTMLElement | null;
        if (scrollEl) {
          if (addAtTop) scrollEl.scrollTop = 0;
          else scrollEl.scrollTop = scrollEl.scrollHeight;
        }

        const firstVisCol = visibleColumns[0];
        if (!firstVisCol) return;

        const newRowEl = tableContainerEl.querySelector(`[data-new-row-key="${key}"]`);
        if (!newRowEl) return;
        const firstTd = newRowEl.querySelectorAll('.data-cell')[0] as
          | HTMLTableCellElement
          | undefined;
        if (!firstTd) return;

        const tdRect = firstTd.getBoundingClientRect();

        focusedCell = null;
        editTarget = {
          rowKey: key,
          colName: firstVisCol.col.name,
          colIndex: firstVisCol.originalIndex,
          value: null,
          originalValue: null,
          dataType: firstVisCol.col.dataType,
          nullable: firstVisCol.col.nullable,
          initialViewportTop: tdRect.top,
          initialViewportLeft: tdRect.left,
          width: Math.max(tdRect.width, 160),
          height: tdRect.height,
        };
      });
    });
  });

  function getPendingValue(rowKey: string, colName: string, originalValue: CellValue): CellValue {
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
    nullable: boolean;
    initialViewportTop: number;
    initialViewportLeft: number;
    width: number;
    height: number;
  }

  let editTarget = $state<EditTarget | null>(null);
  let modalTarget = $state<EditTarget | null>(null);

  interface ViewTarget {
    colName: string;
    value: CellValue;
    dataType: string;
  }
  let viewModalTarget = $state<ViewTarget | null>(null);
  let cellEditorInstance = $state<{ cycle: (_dir: 1 | -1) => void } | null>(null);
  let tableContainerEl = $state<HTMLDivElement | null>(null);
  let tableScrollEl = $state<HTMLDivElement | null>(null);
  let tableScrollWidth = $state(0);

  $effect(() => {
    if (!tableScrollEl) return;
    tableScrollWidth = tableScrollEl.clientWidth;
    const ro = new ResizeObserver(() => {
      tableScrollWidth = tableScrollEl!.clientWidth;
    });
    ro.observe(tableScrollEl);
    return () => ro.disconnect();
  });

  // ── Quick view state ──────────────────────────────────────────────────────

  let quickViewState = $state<{
    triggerRowKey: string;
    triggerColName: string;
    triggerCellValue: CellValue;
    loading: boolean;
    data: QuickViewData | null;
  } | null>(null);

  async function triggerQuickView(
    colName: string,
    cellValue: CellValue,
    rowKey: string,
  ): Promise<void> {
    if (!onForeignKeyQuickView) return;
    if (quickViewState?.triggerRowKey === rowKey && quickViewState?.triggerColName === colName) {
      quickViewState = null;
      return;
    }
    quickViewState = {
      triggerRowKey: rowKey,
      triggerColName: colName,
      triggerCellValue: cellValue,
      loading: true,
      data: null,
    };
    try {
      const data = await onForeignKeyQuickView(colName, cellValue);
      if (quickViewState?.triggerRowKey === rowKey && quickViewState?.triggerColName === colName) {
        quickViewState = {
          triggerRowKey: rowKey,
          triggerColName: colName,
          triggerCellValue: cellValue,
          loading: false,
          data,
        };
      }
    } catch {
      if (quickViewState?.triggerRowKey === rowKey && quickViewState?.triggerColName === colName) {
        quickViewState = {
          triggerRowKey: rowKey,
          triggerColName: colName,
          triggerCellValue: cellValue,
          loading: false,
          data: null,
        };
      }
    }
  }

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
    const tdRect = td.getBoundingClientRect();

    editTarget = {
      rowKey,
      colName: col.name,
      colIndex: originalColIndex,
      value: currentValue,
      originalValue: row[originalColIndex],
      dataType: col.dataType,
      nullable: col.nullable,
      initialViewportTop: tdRect.top,
      initialViewportLeft: tdRect.left,
      width: Math.max(tdRect.width, 160),
      height: tdRect.height,
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
      nullable: col.nullable,
      initialViewportTop: 0,
      initialViewportLeft: 0,
      width: 0,
      height: 0,
    };
  }

  function cellValuesEqual(a: CellValue, b: CellValue): boolean {
    if (a === b) return true;
    if (a === null || b === null) return false;
    // boolean/number equivalence: true≡1, false≡0
    if (typeof a === 'boolean' && typeof b === 'number') return (a ? 1 : 0) === b;
    if (typeof b === 'boolean' && typeof a === 'number') return (b ? 1 : 0) === a;
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
    if (isNewRow) refocusNewRowCell();
    else refocusCell();
  }

  function cancelEdit(): void {
    const wasEditing = editTarget !== null;
    const wasNewRow = editTarget?.rowKey.startsWith('__new__') ?? false;
    editTarget = null;
    if (wasEditing) {
      if (wasNewRow) refocusNewRowCell();
      else refocusCell();
    }
  }

  function openInlineAsModal(currentValue: CellValue): void {
    if (!editTarget) return;
    const target = editTarget;
    editTarget = null;
    modalTarget = {
      rowKey: target.rowKey,
      colName: target.colName,
      colIndex: target.colIndex,
      value: currentValue,
      originalValue: target.originalValue,
      dataType: target.dataType,
      nullable: target.nullable,
      initialViewportTop: 0,
      initialViewportLeft: 0,
      width: 0,
      height: 0,
    };
  }

  function handleTabFromEditor(shiftKey: boolean): void {
    if (!focusedCell) return;
    const rowCount = pageRows.length;
    const colCount = visibleColumns.length;
    let { row, col } = focusedCell;
    if (shiftKey) {
      col -= 1;
      if (col < 0) {
        col = colCount - 1;
        row = Math.max(row - 1, 0);
      }
    } else {
      col += 1;
      if (col >= colCount) {
        col = 0;
        row = Math.min(row + 1, rowCount - 1);
      }
    }
    anchorCell = { row, col };
    focusedCell = { row, col };
    skipNextFocusReset = true;
    scrollFocusedCellIntoView(focusedCell);
  }

  // Called from CellEditor when Tab is pressed. Reads editTarget BEFORE confirmEdit
  // clears it, so the current cell position is still available.
  function handleTabConfirm(newValue: CellValue, shiftKey: boolean): void {
    if (!editTarget) return;

    // Find current visual position from editTarget
    const visColIndex = visibleColumns.findIndex((vc) => vc.originalIndex === editTarget!.colIndex);
    const { rowKey, colName, originalValue } = editTarget;
    const isNewRow = rowKey.startsWith('__new__');

    if (isNewRow) {
      if (visColIndex < 0) {
        confirmEdit(newValue);
        return;
      }
      const colCount = visibleColumns.length;
      const newRowIdx = pendingNewRows.findIndex((r) => r.key === rowKey);
      if (newRowIdx < 0) {
        confirmEdit(newValue);
        return;
      }

      let nextCol = visColIndex;
      let nextNewRowIdx = newRowIdx;
      if (shiftKey) {
        nextCol -= 1;
        if (nextCol < 0) {
          nextCol = colCount - 1;
          nextNewRowIdx = Math.max(nextNewRowIdx - 1, 0);
        }
      } else {
        nextCol += 1;
        if (nextCol >= colCount) {
          nextCol = 0;
          nextNewRowIdx = Math.min(nextNewRowIdx + 1, pendingNewRows.length - 1);
        }
      }

      // Save current edit
      const updated = new Map(pendingChanges);
      if (!updated.has(rowKey)) updated.set(rowKey, new Map());
      updated.get(rowKey)!.set(colName, newValue);
      pendingChanges = updated;
      onChangePending?.(pendingChanges, originalRows);
      editTarget = null;

      const nextRowKey = pendingNewRows[nextNewRowIdx].key;
      newRowAnchorCell = { rowKey: nextRowKey, col: nextCol };
      newRowFocusedCell = { rowKey: nextRowKey, col: nextCol };
      skipNextFocusReset = true;

      requestAnimationFrame(() => {
        const el = getNewRowCellEl(nextRowKey, nextCol);
        if (!el) return;
        el.scrollIntoView({ block: 'nearest', inline: 'nearest' });
        const { originalIndex } = visibleColumns[nextCol] ?? {};
        const colDef = columns[originalIndex];
        if (!colDef) return;
        const currentValue = pendingChanges.get(nextRowKey)?.get(colDef.name) ?? null;
        const tdRect = el.getBoundingClientRect();
        editTarget = {
          rowKey: nextRowKey,
          colName: colDef.name,
          colIndex: originalIndex,
          value: currentValue,
          originalValue: currentValue,
          dataType: colDef.dataType,
          nullable: colDef.nullable,
          initialViewportTop: tdRect.top,
          initialViewportLeft: tdRect.left,
          width: Math.max(tdRect.width, 160),
          height: tdRect.height,
        };
      });
      return;
    }

    const rowIndex = pageRows.findIndex(
      (r, i) => buildRowKey(r, columns, pageOffset + i) === rowKey,
    );
    if (visColIndex < 0 || rowIndex < 0) {
      confirmEdit(newValue);
      return;
    }

    // Compute next cell
    const rowCount = pageRows.length;
    const colCount = visibleColumns.length;
    let nextRow = rowIndex;
    let nextCol = visColIndex;
    if (shiftKey) {
      nextCol -= 1;
      if (nextCol < 0) {
        nextCol = colCount - 1;
        nextRow = Math.max(nextRow - 1, 0);
      }
    } else {
      nextCol += 1;
      if (nextCol >= colCount) {
        nextCol = 0;
        nextRow = Math.min(nextRow + 1, rowCount - 1);
      }
    }

    // Confirm current edit (without refocusCell — we'll open the next editor instead)
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

    // Update selection to reflect the next cell
    anchorCell = { row: nextRow, col: nextCol };
    focusedCell = { row: nextRow, col: nextCol };
    skipNextFocusReset = true;

    // Open the next cell's editor after Svelte has unmounted the current CellEditor
    requestAnimationFrame(() => {
      const el = getFocusedCellEl(nextRow, nextCol);
      if (!el) return;
      el.scrollIntoView({ block: 'nearest', inline: 'nearest' });
      const { originalIndex } = visibleColumns[nextCol] ?? {};
      const rowData = pageRows[nextRow];
      if (rowData === undefined || originalIndex === undefined) return;
      const colDef = columns[originalIndex];
      if (!colDef) return;
      const nextRowKey = buildRowKey(rowData, columns, pageOffset + nextRow);
      if (!originalRows.has(nextRowKey)) {
        const next = new Map(originalRows);
        next.set(nextRowKey, [...rowData]);
        originalRows = next;
      }
      const tdRect = el.getBoundingClientRect();
      editTarget = {
        rowKey: nextRowKey,
        colName: colDef.name,
        colIndex: originalIndex,
        value: getPendingValue(nextRowKey, colDef.name, rowData[originalIndex]),
        originalValue: rowData[originalIndex],
        dataType: colDef.dataType,
        nullable: colDef.nullable,
        initialViewportTop: tdRect.top,
        initialViewportLeft: tdRect.left,
        width: Math.max(tdRect.width, 160),
        height: tdRect.height,
      };
    });
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
    const cell = focusedCell;
    requestAnimationFrame(() => {
      focusedCell = cell;
      skipNextFocusReset = true;
      const el = getFocusedCellEl(cell.row, cell.col);
      el?.focus();
      el?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    });
  }

  function refocusNewRowCell(): void {
    if (!newRowFocusedCell) return;
    const { rowKey, col } = newRowFocusedCell;
    requestAnimationFrame(() => {
      skipNextFocusReset = true;
      const el = getNewRowCellEl(rowKey, col);
      el?.focus();
      el?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    });
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
  let isDraggingRowSelect = $state(false);
  let newRowFocusedCell = $state<{ rowKey: string; col: number } | null>(null);
  let newRowAnchorCell = $state<{ rowKey: string; col: number } | null>(null);
  let isDraggingNewRowSelection = $state(false);
  let justFinishedRowDrag = false;
  let rowSelectionMode = $state(false);
  let rowAnchor = $state<number | null>(null);
  let rowFocus = $state<number | null>(null);
  let additionalSelectedRowIndices = $state<Set<number>>(new Set());
  // Non-reactive flag: prevents onfocus from resetting anchor during programmatic focus
  let skipNextFocusReset = false;
  // Non-contiguous cells selected via option/alt+click, keyed as "row,col"
  let additionalSelectedCells = $state<Set<string>>(new Set());

  function cellKey(row: number, col: number): string {
    return `${row},${col}`;
  }

  function isCellInSelection(row: number, col: number): boolean {
    if (rowSelectionMode) return false;
    if (additionalSelectedCells.size > 0) return additionalSelectedCells.has(cellKey(row, col));
    if (!anchorCell || !focusedCell) return false;
    const minRow = Math.min(anchorCell.row, focusedCell.row);
    const maxRow = Math.max(anchorCell.row, focusedCell.row);
    const minCol = Math.min(anchorCell.col, focusedCell.col);
    const maxCol = Math.max(anchorCell.col, focusedCell.col);
    return row >= minRow && row <= maxRow && col >= minCol && col <= maxCol;
  }

  function newRowPositionIndex(rowKey: string): number {
    return pendingNewRows.findIndex((r) => r.key === rowKey);
  }

  function isNewRowCellInSelection(rowKey: string, col: number): boolean {
    if (!newRowAnchorCell || !newRowFocusedCell) return false;
    const anchorIdx = newRowPositionIndex(newRowAnchorCell.rowKey);
    const focusIdx = newRowPositionIndex(newRowFocusedCell.rowKey);
    const thisIdx = newRowPositionIndex(rowKey);
    const minRow = Math.min(anchorIdx, focusIdx);
    const maxRow = Math.max(anchorIdx, focusIdx);
    const minCol = Math.min(newRowAnchorCell.col, newRowFocusedCell.col);
    const maxCol = Math.max(newRowAnchorCell.col, newRowFocusedCell.col);
    return thisIdx >= minRow && thisIdx <= maxRow && col >= minCol && col <= maxCol;
  }

  function newRowSelectionIsMultiCell(): boolean {
    if (!newRowFocusedCell) return false;
    const anchor = newRowAnchorCell ?? newRowFocusedCell;
    return anchor.rowKey !== newRowFocusedCell.rowKey || anchor.col !== newRowFocusedCell.col;
  }

  function getNewRowSelectionText(): string {
    if (!newRowFocusedCell) return '';
    const anchor = newRowAnchorCell ?? newRowFocusedCell;
    const anchorIdx = newRowPositionIndex(anchor.rowKey);
    const focusIdx = newRowPositionIndex(newRowFocusedCell.rowKey);
    const minRow = Math.min(anchorIdx, focusIdx);
    const maxRow = Math.max(anchorIdx, focusIdx);
    const minCol = Math.min(anchor.col, newRowFocusedCell.col);
    const maxCol = Math.max(anchor.col, newRowFocusedCell.col);
    const lines: string[] = [];
    for (let r = minRow; r <= maxRow; r++) {
      const rKey = pendingNewRows[r]?.key;
      if (!rKey) continue;
      const cells: string[] = [];
      for (let c = minCol; c <= maxCol; c++) {
        const { originalIndex } = visibleColumns[c];
        const colDef = columns[originalIndex];
        const val = pendingChanges.get(rKey)?.get(colDef.name) ?? null;
        cells.push(val === null ? '' : String(val));
      }
      lines.push(cells.join('\t'));
    }
    return lines.join('\n');
  }

  function getAltSelectedCells(): { row: number; col: number }[] {
    return [...additionalSelectedCells]
      .map((k) => {
        const [r, c] = k.split(',').map(Number);
        return { row: r, col: c };
      })
      .sort((a, b) => (a.row !== b.row ? a.row - b.row : a.col - b.col));
  }

  function handleTableKeydown(e: KeyboardEvent): void {
    if (editTarget !== null) {
      if (e.key === 'Escape') {
        e.preventDefault();
        cancelEdit();
      }
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        cellEditorInstance?.cycle(e.key === 'ArrowDown' ? 1 : -1);
      } else if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
        e.preventDefault();
      }
      return;
    }
    const target = e.target as HTMLElement;
    if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA') return;

    const rowCount = pageRows.length;
    const colCount = visibleColumns.length;
    if (rowCount === 0 || colCount === 0) return;

    if ((e.metaKey || e.ctrlKey) && e.key === 'c') {
      if (window.getSelection()?.toString()) return;
      e.preventDefault();
      if (rowSelectionMode) {
        if (selectedRowKeys.size > 1) copySelectedRowsTabSeparated();
        else {
          const idx = rowFocus ?? rowAnchor;
          if (idx !== null) {
            const r = pageRows[idx];
            if (r) copyRowTabSeparated(r);
          }
        }
      } else {
        copySelection();
      }
      return;
    }
    if ((e.metaKey || e.ctrlKey) && e.key === 'x') {
      e.preventDefault();
      if (rowSelectionMode) {
        if (!editable || readOnly) return;
        if (selectedRowKeys.size > 1) copySelectedRowsTabSeparated();
        else {
          const idx = rowFocus ?? rowAnchor;
          if (idx !== null) {
            const r = pageRows[idx];
            if (r) copyRowTabSeparated(r);
          }
        }
        const next = new Map(pendingDeletedRows);
        for (let r = 0; r < pageRows.length; r++) {
          const rowData = pageRows[r];
          const key = buildRowKey(rowData, columns, pageOffset + r);
          if (selectedRowKeys.has(key)) next.set(key, [...rowData]);
        }
        pendingDeletedRows = next;
        onDeleteRowsPending?.(pendingDeletedRows);
        return;
      }
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

    if ((e.key === 'Delete' || e.key === 'Backspace') && editable && !readOnly) {
      e.preventDefault();
      // Collect rows to act on: either the full selection or just the focused row
      const targets: { key: string; data: CellValue[] }[] = [];
      if (selectedRowKeys.size > 0) {
        for (let r = 0; r < pageRows.length; r++) {
          const rowData = pageRows[r];
          const rowKey = buildRowKey(rowData, columns, pageOffset + r);
          if (selectedRowKeys.has(rowKey)) targets.push({ key: rowKey, data: rowData });
        }
      } else if (focusedCell) {
        const rowData = pageRows[focusedCell.row];
        if (rowData) {
          const rowKey = buildRowKey(rowData, columns, pageOffset + focusedCell.row);
          targets.push({ key: rowKey, data: rowData });
        }
      }
      if (targets.length > 0) {
        const allDeleted = targets.every((t) => pendingDeletedRows.has(t.key));
        const next = new Map(pendingDeletedRows);
        for (const { key, data } of targets) {
          if (allDeleted) {
            next.delete(key);
          } else {
            next.set(key, [...data]);
          }
        }
        pendingDeletedRows = next;
        onDeleteRowsPending?.(pendingDeletedRows);
      }
      return;
    }

    if (rowSelectionMode) {
      if (e.key === 'Escape') {
        rowSelectionMode = false;
        rowAnchor = null;
        rowFocus = null;
        additionalSelectedRowIndices = new Set();
        return;
      }
      if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
        e.preventDefault();
        const current = rowFocus ?? rowAnchor ?? 0;
        const next =
          e.key === 'ArrowUp' ? Math.max(0, current - 1) : Math.min(rowCount - 1, current + 1);
        additionalSelectedRowIndices = new Set();
        if (e.shiftKey) {
          if (rowAnchor === null) rowAnchor = current;
          rowFocus = next;
        } else {
          rowAnchor = next;
          rowFocus = next;
        }
        scrollRowIntoView(next);
        return;
      }
      if (e.key === 'ArrowRight' && rowAnchor !== null) {
        e.preventDefault();
        const anchor = rowAnchor;
        const focus = rowFocus ?? rowAnchor;
        const nonContiguous = additionalSelectedRowIndices;
        rowSelectionMode = false;
        rowAnchor = null;
        rowFocus = null;
        additionalSelectedRowIndices = new Set();
        if (nonContiguous.size > 0) {
          additionalSelectedCells = new Set([...nonContiguous].map((r) => cellKey(r, 0)));
          const firstIdx = Math.min(...nonContiguous);
          anchorCell = { row: firstIdx, col: 0 };
          focusedCell = { row: firstIdx, col: 0 };
        } else {
          anchorCell = { row: Math.min(anchor, focus), col: 0 };
          focusedCell = { row: Math.max(anchor, focus), col: 0 };
        }
        skipNextFocusReset = true;
        scrollFocusedCellIntoView(anchorCell);
        return;
      }
      return;
    }

    if (!focusedCell && newRowFocusedCell) {
      const isArrowKey = ['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(e.key);
      if (!isArrowKey && e.key !== 'Enter' && e.key !== 'Escape') return;
      e.preventDefault();
      const colCount = visibleColumns.length;
      let { rowKey, col } = newRowFocusedCell;
      let rowIdx = newRowPositionIndex(rowKey);

      if (e.key === 'Escape') {
        getNewRowCellEl(rowKey, col)?.blur();
        newRowFocusedCell = null;
        newRowAnchorCell = null;
        return;
      }

      if (e.key === 'Enter') {
        if (editable) {
          const { originalIndex } = visibleColumns[col];
          const colDef = columns[originalIndex];
          const currentValue = pendingChanges.get(rowKey)?.get(colDef.name) ?? null;
          const td = getNewRowCellEl(rowKey, col);
          if (td && colDef)
            handleNewRowCellDblClick(
              { currentTarget: td } as unknown as MouseEvent,
              rowKey,
              currentValue,
              colDef,
              originalIndex,
            );
        }
        return;
      }

      if (e.key === 'ArrowLeft') {
        col = Math.max(col - 1, 0);
      } else if (e.key === 'ArrowRight') {
        col = Math.min(col + 1, colCount - 1);
      } else if (e.key === 'ArrowUp') {
        if (rowIdx > 0) {
          rowIdx--;
          rowKey = pendingNewRows[rowIdx].key;
        } else if (settings.newRowPosition !== 'top' && pageRows.length > 0 && !e.shiftKey) {
          // new rows at bottom: cross up into last regular row
          const targetRow = pageRows.length - 1;
          newRowFocusedCell = null;
          newRowAnchorCell = null;
          anchorCell = { row: targetRow, col };
          focusedCell = { row: targetRow, col };
          skipNextFocusReset = true;
          scrollFocusedCellIntoView({ row: targetRow, col });
          return;
        }
      } else if (e.key === 'ArrowDown') {
        if (rowIdx < pendingNewRows.length - 1) {
          rowIdx++;
          rowKey = pendingNewRows[rowIdx].key;
        } else if (settings.newRowPosition === 'top' && pageRows.length > 0 && !e.shiftKey) {
          // new rows at top: cross down into first regular row
          newRowFocusedCell = null;
          newRowAnchorCell = null;
          anchorCell = { row: 0, col };
          focusedCell = { row: 0, col };
          skipNextFocusReset = true;
          scrollFocusedCellIntoView({ row: 0, col });
          return;
        }
      }

      if (e.shiftKey) {
        newRowFocusedCell = { rowKey, col };
      } else {
        newRowAnchorCell = { rowKey, col };
        newRowFocusedCell = { rowKey, col };
      }
      skipNextFocusReset = true;
      requestAnimationFrame(() => {
        const el = getNewRowCellEl(rowKey, col);
        el?.focus();
        el?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
      });
      return;
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

    const allInCol0 =
      additionalSelectedCells.size > 0
        ? [...additionalSelectedCells].every((k) => k.endsWith(',0'))
        : focusedCell.col === 0 && (anchorCell?.col ?? 0) === 0;
    if (e.key === 'ArrowLeft' && !e.shiftKey && allInCol0) {
      e.preventDefault();
      let firstRow: number;
      if (additionalSelectedCells.size > 0) {
        const rowIndices = [...additionalSelectedCells].map((k) => parseInt(k.split(',')[0]));
        firstRow = Math.min(...rowIndices);
        anchorCell = null;
        focusedCell = null;
        additionalSelectedCells = new Set();
        rowSelectionMode = true;
        rowAnchor = firstRow;
        rowFocus = firstRow;
        additionalSelectedRowIndices = new Set(rowIndices);
      } else {
        const minRow = Math.min(anchorCell?.row ?? focusedCell.row, focusedCell.row);
        const maxRow = Math.max(anchorCell?.row ?? focusedCell.row, focusedCell.row);
        firstRow = minRow;
        anchorCell = null;
        focusedCell = null;
        additionalSelectedCells = new Set();
        rowSelectionMode = true;
        rowAnchor = minRow;
        rowFocus = maxRow;
        additionalSelectedRowIndices = new Set();
      }
      requestAnimationFrame(() => {
        const tr = tableContainerEl?.querySelector<HTMLTableRowElement>(
          `tbody tr:nth-child(${firstRow + 1})`,
        );
        tr?.querySelector<HTMLElement>('.rownum-cell')?.focus();
      });
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
    } else if (keyEventToString(e) === shortcutsStore.getShortcut('TABLE_QUICK_VIEW_RELATIONS')) {
      const { col: colMeta, originalIndex } = visibleColumns[col];
      if (colMeta?.isForeignKey) {
        const rowData = pageRows[row];
        if (rowData) {
          const rowKey = buildRowKey(rowData, columns, pageOffset + row);
          const cellValue = getPendingValue(rowKey, colMeta.name, rowData[originalIndex]);
          if (cellValue !== null) {
            triggerQuickView(colMeta.name, cellValue, rowKey);
          }
        }
      }
      return;
    } else if (keyEventToString(e) === shortcutsStore.getShortcut('TABLE_VIEW_RELATIONS')) {
      const { col: colMeta, originalIndex } = visibleColumns[col];
      if (colMeta?.isForeignKey) {
        const rowData = pageRows[row];
        if (rowData) {
          const rowKey = buildRowKey(rowData, columns, pageOffset + row);
          const cellValue = getPendingValue(rowKey, colMeta.name, rowData[originalIndex]);
          if (cellValue !== null) {
            onForeignKeyClick?.(colMeta.name, cellValue);
          }
        }
      }
      return;
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
      if (quickViewState) {
        quickViewState = null;
        return;
      }
      cancelEdit();
      anchorCell = null;
      if (focusedCell) {
        getFocusedCellEl(focusedCell.row, focusedCell.col)?.blur();
      }
      focusedCell = null;
      additionalSelectedCells = new Set();
      rowSelectionMode = false;
      rowAnchor = null;
      rowFocus = null;
      additionalSelectedRowIndices = new Set();
      return;
    } else {
      return;
    }

    if (isArrow && !e.shiftKey && pendingNewRows.length > 0) {
      const origRow = focusedCell!.row;
      if (e.key === 'ArrowUp' && row === 0 && origRow === 0 && settings.newRowPosition === 'top') {
        // Cross up into last new row at top
        const targetKey = pendingNewRows[pendingNewRows.length - 1].key;
        newRowAnchorCell = { rowKey: targetKey, col };
        newRowFocusedCell = { rowKey: targetKey, col };
        anchorCell = null;
        focusedCell = null;
        skipNextFocusReset = true;
        requestAnimationFrame(() => {
          const el = getNewRowCellEl(targetKey, col);
          el?.focus();
          el?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
        });
        return;
      }
      if (
        e.key === 'ArrowDown' &&
        row === rowCount - 1 &&
        origRow === rowCount - 1 &&
        settings.newRowPosition !== 'top'
      ) {
        // Cross down into first new row at bottom
        const targetKey = pendingNewRows[0].key;
        newRowAnchorCell = { rowKey: targetKey, col };
        newRowFocusedCell = { rowKey: targetKey, col };
        anchorCell = null;
        focusedCell = null;
        skipNextFocusReset = true;
        requestAnimationFrame(() => {
          const el = getNewRowCellEl(targetKey, col);
          el?.focus();
          el?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
        });
        return;
      }
    }

    if (isArrow) {
      additionalSelectedCells = new Set();
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

  function getNewRowCellEl(rowKey: string, colIndex: number): HTMLTableCellElement | null {
    if (!tableContainerEl) return null;
    const tr = tableContainerEl.querySelector<HTMLElement>(`[data-new-row-key="${rowKey}"]`);
    return tr?.querySelectorAll<HTMLTableCellElement>('.data-cell')[colIndex] ?? null;
  }

  function scrollFocusedCellIntoView(cell: { row: number; col: number }): void {
    requestAnimationFrame(() => {
      const el = getFocusedCellEl(cell.row, cell.col);
      el?.focus();
      el?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    });
  }

  function scrollRowIntoView(rowIdx: number): void {
    requestAnimationFrame(() => {
      const tr = tableContainerEl?.querySelector<HTMLTableRowElement>(
        `tbody tr:nth-child(${rowIdx + 1})`,
      );
      tr?.scrollIntoView({ block: 'nearest' });
    });
  }

  // ── Cell formatting ───────────────────────────────────────────────────────

  function escapeHtml(s: string): string {
    return s
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
  }

  function highlightText(text: string, term: string): string {
    const lowerText = text.toLowerCase();
    const lowerTerm = term.toLowerCase();
    const parts: string[] = [];
    let i = 0;
    while (i < text.length) {
      const idx = lowerText.indexOf(lowerTerm, i);
      if (idx === -1) {
        parts.push(escapeHtml(text.slice(i)));
        break;
      }
      if (idx > i) parts.push(escapeHtml(text.slice(i, idx)));
      parts.push(
        `<mark class="search-highlight">${escapeHtml(text.slice(idx, idx + term.length))}</mark>`,
      );
      i = idx + term.length;
    }
    return parts.join('');
  }

  // ── Data type categorisation ──────────────────────────────────────────────

  function getDataTypeCategory(
    dataType: string,
  ): 'number' | 'timestamp' | 'boolean' | 'json' | 'text' {
    const dt = dataType.toLowerCase();
    if (
      /^(int|int2|int4|int8|int16|bigint|smallint|float|float4|float8|real|double|numeric|decimal|serial|bigserial|money)/.test(
        dt,
      )
    )
      return 'number';
    if (/^(timestamp|date|time|interval)/.test(dt)) return 'timestamp';
    if (/^bool/.test(dt) || dt === 'tinyint(1)') return 'boolean';
    if (/^json/.test(dt)) return 'json';
    return 'text';
  }

  function parseDefaultValue(value: string, dataType: string): CellValue {
    if (value.toUpperCase() === 'NULL') return null;
    const category = getDataTypeCategory(dataType);
    if (category === 'boolean') {
      const truthy = value === '1' || value.toLowerCase() === 'true';
      return dataType.toLowerCase() === 'tinyint(1)' ? (truthy ? 1 : 0) : truthy;
    }
    return value;
  }

  function coercePasteValue(raw: string, col: { dataType: string }): CellValue {
    if (raw === '') return null;
    if (getDataTypeCategory(col.dataType) === 'boolean') {
      const truthy = raw === '1' || raw.toLowerCase() === 'true';
      return col.dataType.toLowerCase() === 'tinyint(1)' ? (truthy ? 1 : 0) : truthy;
    }
    return raw;
  }

  // ── Row selection ─────────────────────────────────────────────────────────

  let selectedRowKeys = $state<Set<string>>(new Set());

  $effect(() => {
    if (rowSelectionMode) {
      if (rowAnchor === null) {
        selectedRowKeys = new Set();
        return;
      }
      const focus = rowFocus ?? rowAnchor;
      if (additionalSelectedRowIndices.size > 0) {
        const newKeys = new Set<string>();
        for (const idx of additionalSelectedRowIndices) {
          const rowData = pageRows[idx];
          if (rowData) newKeys.add(buildRowKey(rowData, columns, pageOffset + idx));
        }
        selectedRowKeys = newKeys;
      } else {
        const minRow = Math.min(rowAnchor, focus);
        const maxRow = Math.max(rowAnchor, focus);
        const newKeys = new Set<string>();
        for (let r = minRow; r <= maxRow; r++) {
          const rowData = pageRows[r];
          if (rowData) newKeys.add(buildRowKey(rowData, columns, pageOffset + r));
        }
        selectedRowKeys = newKeys;
      }
    } else {
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
    }
  });

  $effect(() => {
    if (!rowSelectionMode || selectedRowKeys.size !== 1) return;
    const selectedKey = [...selectedRowKeys][0];
    for (let i = 0; i < pageRows.length; i++) {
      if (buildRowKey(pageRows[i], columns, pageOffset + i) === selectedKey) {
        onRowSelect?.(pageRows[i], columns);
        break;
      }
    }
  });

  function toggleRowSelection(rowKey: string): void {
    const next = new Set(selectedRowKeys);
    if (next.has(rowKey)) next.delete(rowKey);
    else next.add(rowKey);
    selectedRowKeys = next;
  }

  function handleRowClick(e: MouseEvent, rowKey: string): void {
    if (rowSelectionMode) return;
    if (e.metaKey || e.ctrlKey) {
      toggleRowSelection(rowKey);
    } else if (!e.shiftKey) {
      selectedRowKeys = new Set([rowKey]);
    }
  }

  // ── Header context menu / rename ─────────────────────────────────────────

  let headerContextMenu = $state<{ x: number; y: number; colName: string } | null>(null);
  let renamingHeader = $state<{ colName: string; value: string } | null>(null);
  let renameHeaderInputEl = $state<HTMLInputElement | null>(null);

  function openHeaderContextMenu(e: MouseEvent, colName: string): void {
    e.preventDefault();
    e.stopPropagation();
    activeMenuDismiss?.();
    headerContextMenu = { x: e.clientX, y: e.clientY, colName };
    activeMenuDismiss = () => {
      headerContextMenu = null;
    };
  }

  function startHeaderRename(colName: string): void {
    headerContextMenu = null;
    renamingHeader = { colName, value: columnRenames[colName] ?? colName };
    tick().then(() => renameHeaderInputEl?.select());
  }

  function commitHeaderRename(): void {
    if (!renamingHeader) return;
    const { colName } = renamingHeader;
    const trimmed = renamingHeader.value.trim();
    renamingHeader = null;
    if (!onRenameColumn) return;
    onRenameColumn(colName, trimmed === '' ? colName : trimmed);
  }

  function cancelHeaderRename(): void {
    renamingHeader = null;
  }

  function handleHeaderRenameKeydown(e: KeyboardEvent): void {
    if (e.key === 'Enter') {
      e.preventDefault();
      commitHeaderRename();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      cancelHeaderRename();
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
  // Snapshots of selection state at menu-open time, so re-renders from focus/blur don't alter the menu.
  let contextMenuSnapshotHasFocus = $state(false);
  let contextMenuSnapshotIsMultiCell = $state(false);
  let contextMenuSnapshotIsMultiCol = $state(false);
  let contextMenuSnapshotIsRowSelection = $state(false);

  function handleRowContextMenu(
    e: MouseEvent,
    row: CellValue[],
    rowIndex: number,
    colName: string | null = null,
  ): void {
    e.preventDefault();
    activeMenuDismiss?.();
    const rowKey = buildRowKey(row, columns, pageOffset + rowIndex);
    if (rowSelectionMode && !selectedRowKeys.has(rowKey)) {
      rowAnchor = rowIndex;
      rowFocus = rowIndex;
      additionalSelectedRowIndices = new Set();
    }
    contextMenu = { x: e.clientX, y: e.clientY, rowKey, row, colName };
    contextMenuSnapshotHasFocus = focusedCell !== null;
    contextMenuSnapshotIsMultiCell = selectionIsMultiCell();
    contextMenuSnapshotIsRowSelection = rowSelectionMode;
    const range = getSelectionRange();
    contextMenuSnapshotIsMultiCol = range ? range.minCol !== range.maxCol : false;
    activeMenuDismiss = () => {
      contextMenu = null;
    };
    contextMenuClipboardHasContent = false;
    navigator.clipboard
      .readText()
      .then((t) => {
        contextMenuClipboardHasContent = t.length > 0;
      })
      .catch(() => {});
  }

  function dismissContextMenu(): void {
    contextMenu = null;
    if (activeMenuDismiss) activeMenuDismiss = null;
  }

  const settingsStore = useSettings();
  const settings = $derived(settingsStore.settings);
  const shortcutsStore = useShortcuts();

  function formatCell(value: CellValue): string {
    if (value === null) return '';
    const nl = settings.newlineReplacement ?? '↵';
    return String(value).replace(/\r\n|\r|\n/g, nl);
  }

  function isDatetimeishType(dt: string): boolean {
    const lower = dt.toLowerCase();
    return (
      lower.includes('date') ||
      lower.includes('timestamp') ||
      (lower.includes('time') && !lower.includes('timestamp'))
    );
  }

  function getDatetimeInputType(dt: string): 'date' | 'time' | 'datetime-local' {
    const lower = dt.toLowerCase();
    if ((lower.includes('date') && lower.includes('time')) || lower.includes('timestamp'))
      return 'datetime-local';
    if (lower.includes('date')) return 'date';
    return 'time';
  }

  const contextMenuColDataType = $derived(
    contextMenu?.colName
      ? (columns.find((c) => c.name === contextMenu!.colName)?.dataType ?? '')
      : '',
  );
  const contextMenuColIsDatetime = $derived(isDatetimeishType(contextMenuColDataType));

  function formatNow(d: Date, type: 'date' | 'time' | 'datetime-local'): string {
    const p = (n: number) => String(n).padStart(2, '0');
    const date = `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
    const time = `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
    if (type === 'date') return date;
    if (type === 'time') return time;
    return `${date} ${time}`;
  }

  function parseDbNow(raw: string, type: 'date' | 'time' | 'datetime-local'): string {
    const normalized = String(raw)
      .replace('T', ' ')
      .replace(/\.\d+/, '')
      .replace(/[+-]\d{2}:\d{2}$/, '')
      .trim();
    const [datePart = '', timePart = '00:00:00'] = normalized.split(' ');
    if (type === 'date') return datePart;
    if (type === 'time') return timePart;
    return `${datePart} ${timePart}`;
  }

  async function setNowFromContextMenu(): Promise<void> {
    if (!contextMenu?.colName || !editable || readOnly) return;
    const { rowKey, row, colName } = contextMenu;
    const col = columns.find((c) => c.name === colName);
    if (!col) return;
    const type = getDatetimeInputType(col.dataType);

    let nowValue: string;
    if (settings.nowTimeSource === 'database' && connectionId) {
      try {
        const result = await executeQuery(connectionId, 'SELECT NOW()', 1, 1, database ?? null);
        if (!result.error && result.rows[0]?.[0] != null) {
          nowValue = parseDbNow(String(result.rows[0][0]), type);
        } else {
          nowValue = formatNow(new Date(), type);
        }
      } catch {
        nowValue = formatNow(new Date(), type);
      }
    } else {
      nowValue = formatNow(new Date(), type);
    }

    const originalColIndex = columns.findIndex((c) => c.name === colName);
    if (!originalRows.has(rowKey)) {
      const next = new Map(originalRows);
      next.set(rowKey, [...row]);
      originalRows = next;
    }
    applyPendingChange(rowKey, colName, row[originalColIndex], nowValue);
    onChangePending?.(pendingChanges, originalRows);
    dismissContextMenu();
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

  function openInlineEditFromContextMenu(): void {
    if (!contextMenu?.colName) return;
    const { row, rowKey, colName } = contextMenu;
    const rowIndex = pageRows.findIndex(
      (r, i) => buildRowKey(r, columns, pageOffset + i) === rowKey,
    );
    if (rowIndex < 0) {
      dismissContextMenu();
      return;
    }
    const originalColIndex = columns.findIndex((c) => c.name === colName);
    if (originalColIndex < 0) {
      dismissContextMenu();
      return;
    }
    const visColIndex = visibleColumns.findIndex((vc) => vc.originalIndex === originalColIndex);
    if (visColIndex < 0) {
      dismissContextMenu();
      return;
    }

    dismissContextMenu();
    const fakeEvent = {
      currentTarget: getFocusedCellEl(rowIndex, visColIndex),
    } as unknown as MouseEvent;
    handleCellDblClick(fakeEvent, row, rowIndex, originalColIndex);
  }

  function openModalFromContextMenu(): void {
    if (!contextMenu?.colName) return;
    const { row, rowKey, colName } = contextMenu;
    const rowIndex = pageRows.findIndex(
      (r, i) => buildRowKey(r, columns, pageOffset + i) === rowKey,
    );
    if (rowIndex < 0) {
      dismissContextMenu();
      return;
    }
    const originalColIndex = columns.findIndex((c) => c.name === colName);
    if (originalColIndex < 0) {
      dismissContextMenu();
      return;
    }
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
      nullable: col.nullable,
      initialViewportTop: 0,
      initialViewportLeft: 0,
      width: 0,
      height: 0,
    };
    dismissContextMenu();
  }

  function openViewModalFromContextMenu(): void {
    if (!contextMenu?.colName) return;
    const { row, rowKey, colName } = contextMenu;
    const originalColIndex = columns.findIndex((c) => c.name === colName);
    if (originalColIndex < 0) {
      dismissContextMenu();
      return;
    }
    const currentValue = getPendingValue(rowKey, colName, row[originalColIndex]);
    const col = columns[originalColIndex];

    viewModalTarget = {
      colName,
      value: currentValue,
      dataType: col.dataType,
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
    const tdRect = td.getBoundingClientRect();

    focusedCell = null;
    editTarget = {
      rowKey: newRowKey,
      colName: col.name,
      colIndex: originalColIndex,
      value: currentValue,
      originalValue: currentValue,
      dataType: col.dataType,
      nullable: col.nullable,
      initialViewportTop: tdRect.top,
      initialViewportLeft: tdRect.left,
      width: Math.max(tdRect.width, 160),
      height: tdRect.height,
    };
  }

  function openNewRowInlineEditFromContextMenu(): void {
    if (!contextMenu?.colName) return;
    const { rowKey, colName } = contextMenu;
    const originalColIndex = columns.findIndex((c) => c.name === colName);
    if (originalColIndex < 0) {
      dismissContextMenu();
      return;
    }
    const visColIndex = visibleColumns.findIndex((vc) => vc.originalIndex === originalColIndex);
    if (visColIndex < 0) {
      dismissContextMenu();
      return;
    }
    const newRowEl = tableContainerEl?.querySelector<HTMLElement>(`[data-new-row-key="${rowKey}"]`);
    const td = newRowEl?.querySelectorAll<HTMLTableCellElement>('.data-cell')[visColIndex];
    if (!td) {
      dismissContextMenu();
      return;
    }
    const currentValue = pendingChanges.get(rowKey)?.get(colName) ?? null;
    const col = columns[originalColIndex];
    dismissContextMenu();
    const tdRect = td.getBoundingClientRect();
    focusedCell = null;
    editTarget = {
      rowKey,
      colName,
      colIndex: originalColIndex,
      value: currentValue,
      originalValue: currentValue,
      dataType: col.dataType,
      nullable: col.nullable,
      initialViewportTop: tdRect.top,
      initialViewportLeft: tdRect.left,
      width: Math.max(tdRect.width, 160),
      height: tdRect.height,
    };
  }

  function openNewRowModalFromContextMenu(): void {
    if (!contextMenu?.colName) return;
    const { rowKey, colName } = contextMenu;
    const originalColIndex = columns.findIndex((c) => c.name === colName);
    if (originalColIndex < 0) {
      dismissContextMenu();
      return;
    }
    const currentValue = pendingChanges.get(rowKey)?.get(colName) ?? null;
    const col = columns[originalColIndex];
    modalTarget = {
      rowKey,
      colName,
      colIndex: originalColIndex,
      value: currentValue,
      originalValue: currentValue,
      dataType: col.dataType,
      nullable: col.nullable,
      initialViewportTop: 0,
      initialViewportLeft: 0,
      width: 0,
      height: 0,
    };
    dismissContextMenu();
  }

  function openNewRowViewModalFromContextMenu(): void {
    if (!contextMenu?.colName) return;
    const { rowKey, colName } = contextMenu;
    const originalColIndex = columns.findIndex((c) => c.name === colName);
    if (originalColIndex < 0) {
      dismissContextMenu();
      return;
    }
    const currentValue = pendingChanges.get(rowKey)?.get(colName) ?? null;
    const col = columns[originalColIndex];
    viewModalTarget = { colName, value: currentValue, dataType: col.dataType };
    dismissContextMenu();
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

  function getSelectionRange(): {
    minRow: number;
    maxRow: number;
    minCol: number;
    maxCol: number;
  } | null {
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
    if (additionalSelectedCells.size > 0) return additionalSelectedCells.size > 1;
    const r = getSelectionRange();
    if (!r) return false;
    return r.minRow !== r.maxRow || r.minCol !== r.maxCol;
  }

  function getSelectionText(): string {
    if (additionalSelectedCells.size > 0) {
      const cells = getAltSelectedCells();
      const rowMap = new Map<number, number[]>();
      for (const { row, col } of cells) {
        if (!rowMap.has(row)) rowMap.set(row, []);
        rowMap.get(row)!.push(col);
      }
      const lines: string[] = [];
      for (const [r, cols] of [...rowMap.entries()].sort((a, b) => a[0] - b[0])) {
        const rowData = pageRows[r];
        if (!rowData) continue;
        const rowKey = buildRowKey(rowData, columns, pageOffset + r);
        const cellTexts = cols
          .sort((a, b) => a - b)
          .map((c) => {
            const { originalIndex } = visibleColumns[c];
            const col = columns[originalIndex];
            const val = getPendingValue(rowKey, col.name, rowData[originalIndex]);
            return val === null ? '' : String(val);
          });
        lines.push(cellTexts.join('\t'));
      }
      return lines.join('\n');
    }
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
    if (newRowFocusedCell && !focusedCell) {
      navigator.clipboard.writeText(getNewRowSelectionText()).catch(() => {});
      return;
    }
    const text = getSelectionText();
    navigator.clipboard.writeText(text).catch(() => {});
  }

  function applyPendingChange(
    rowKey: string,
    colName: string,
    originalValue: CellValue,
    newValue: CellValue,
  ): void {
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
    if (newRowFocusedCell && !focusedCell) {
      navigator.clipboard.writeText(getNewRowSelectionText()).catch(() => {});
      const anchor = newRowAnchorCell ?? newRowFocusedCell;
      const anchorIdx = newRowPositionIndex(anchor.rowKey);
      const focusIdx = newRowPositionIndex(newRowFocusedCell.rowKey);
      const minRow = Math.min(anchorIdx, focusIdx);
      const maxRow = Math.max(anchorIdx, focusIdx);
      const minCol = Math.min(anchor.col, newRowFocusedCell.col);
      const maxCol = Math.max(anchor.col, newRowFocusedCell.col);
      for (let r = minRow; r <= maxRow; r++) {
        const rKey = pendingNewRows[r]?.key;
        if (!rKey) continue;
        for (let c = minCol; c <= maxCol; c++) {
          const { originalIndex } = visibleColumns[c];
          const colDef = columns[originalIndex];
          if (colDef) applyPendingChange(rKey, colDef.name, null, null);
        }
      }
      onChangePending?.(pendingChanges, originalRows);
      return;
    }
    copySelection();
    if (additionalSelectedCells.size > 0) {
      for (const { row: r, col: c } of getAltSelectedCells()) {
        const rowData = pageRows[r];
        if (!rowData) continue;
        const rowKey = buildRowKey(rowData, columns, pageOffset + r);
        if (!originalRows.has(rowKey)) {
          const next = new Map(originalRows);
          next.set(rowKey, [...rowData]);
          originalRows = next;
        }
        const { originalIndex } = visibleColumns[c];
        const col = columns[originalIndex];
        if (col) applyPendingChange(rowKey, col.name, rowData[originalIndex], null);
      }
    } else {
      const range = getSelectionRange();
      if (!range) return;
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
    }
    onChangePending?.(pendingChanges, originalRows);
  }

  async function pasteFromClipboard(): Promise<void> {
    if (!editable || readOnly) return;
    if (rowSelectionMode) {
      let text: string;
      try {
        text = await navigator.clipboard.readText();
      } catch {
        return;
      }
      if (!text) return;
      const clipLines = text.split('\n').filter((l) => l !== '');
      if (clipLines.length === 0) return;
      const selectedIndices: number[] = [];
      for (let r = 0; r < pageRows.length; r++) {
        const key = buildRowKey(pageRows[r], columns, pageOffset + r);
        if (selectedRowKeys.has(key)) selectedIndices.push(r);
      }
      for (let i = 0; i < selectedIndices.length; i++) {
        const r = selectedIndices[i];
        const rowData = pageRows[r];
        if (!rowData) continue;
        const rowKey = buildRowKey(rowData, columns, pageOffset + r);
        if (!originalRows.has(rowKey)) {
          const next = new Map(originalRows);
          next.set(rowKey, [...rowData]);
          originalRows = next;
        }
        const lineValues = clipLines[i % clipLines.length].split('\t');
        visibleColumns.forEach(({ col, originalIndex }, c) => {
          const pasteValue = lineValues[c] ?? '';
          applyPendingChange(
            rowKey,
            col.name,
            rowData[originalIndex],
            coercePasteValue(pasteValue, col),
          );
        });
      }
      onChangePending?.(pendingChanges, originalRows);
      return;
    }
    if (!focusedCell && newRowFocusedCell) {
      let newRowPasteText: string;
      try {
        newRowPasteText = await navigator.clipboard.readText();
      } catch {
        return;
      }
      if (!newRowPasteText) return;
      const anchor = newRowAnchorCell ?? newRowFocusedCell;
      const anchorIdx = newRowPositionIndex(anchor.rowKey);
      const focusIdx = newRowPositionIndex(newRowFocusedCell.rowKey);
      const minRow = Math.min(anchorIdx, focusIdx);
      const maxRow = Math.max(anchorIdx, focusIdx);
      const minCol = Math.min(anchor.col, newRowFocusedCell.col);
      const maxCol = Math.max(anchor.col, newRowFocusedCell.col);
      const clipRows = newRowPasteText.split('\n').map((line) => line.split('\t'));
      let clipRowIdx = 0;
      for (let r = minRow; r <= maxRow; r++) {
        const rKey = pendingNewRows[r]?.key;
        if (!rKey) continue;
        const clipCols = clipRows[clipRowIdx % clipRows.length] ?? [];
        let clipColIdx = 0;
        for (let c = minCol; c <= maxCol; c++) {
          const { originalIndex } = visibleColumns[c];
          const colDef = columns[originalIndex];
          if (!colDef) continue;
          const pasteValue = clipCols[clipColIdx % Math.max(clipCols.length, 1)] ?? '';
          clipColIdx++;
          applyPendingChange(rKey, colDef.name, null, coercePasteValue(pasteValue, colDef));
        }
        clipRowIdx++;
      }
      onChangePending?.(pendingChanges, originalRows);
      return;
    }
    if (!focusedCell) return;
    let text: string;
    try {
      text = await navigator.clipboard.readText();
    } catch {
      return;
    }
    if (!text) return;
    // Flatten clipboard into a linear list, looped across selected cells
    const clipValues = text.split('\n').flatMap((line) => line.split('\t'));
    if (clipValues.length === 0) return;
    if (additionalSelectedCells.size > 0) {
      let clipIndex = 0;
      for (const { row: r, col: c } of getAltSelectedCells()) {
        const rowData = pageRows[r];
        if (!rowData) continue;
        const rowKey = buildRowKey(rowData, columns, pageOffset + r);
        if (!originalRows.has(rowKey)) {
          const next = new Map(originalRows);
          next.set(rowKey, [...rowData]);
          originalRows = next;
        }
        const { originalIndex } = visibleColumns[c];
        const col = columns[originalIndex];
        if (!col) continue;
        const pasteValue = clipValues[clipIndex % clipValues.length];
        clipIndex++;
        applyPendingChange(
          rowKey,
          col.name,
          rowData[originalIndex],
          coercePasteValue(pasteValue, col),
        );
      }
    } else {
      const range = getSelectionRange();
      if (!range) return;
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
          applyPendingChange(
            rowKey,
            col.name,
            rowData[originalIndex],
            coercePasteValue(pasteValue, col),
          );
        }
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
      const found = pageRows.find((r, i) => buildRowKey(r, columns, pageOffset + i) === key);
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

  function setSelectionNull(): void {
    if (!editable || readOnly) return;
    if (newRowFocusedCell && !focusedCell) {
      const anchor = newRowAnchorCell ?? newRowFocusedCell;
      const anchorIdx = newRowPositionIndex(anchor.rowKey);
      const focusIdx = newRowPositionIndex(newRowFocusedCell.rowKey);
      const minRow = Math.min(anchorIdx, focusIdx);
      const maxRow = Math.max(anchorIdx, focusIdx);
      const minCol = Math.min(anchor.col, newRowFocusedCell.col);
      const maxCol = Math.max(anchor.col, newRowFocusedCell.col);
      for (let r = minRow; r <= maxRow; r++) {
        const rKey = pendingNewRows[r]?.key;
        if (!rKey) continue;
        for (let c = minCol; c <= maxCol; c++) {
          const { originalIndex } = visibleColumns[c];
          const colDef = columns[originalIndex];
          if (colDef) applyPendingChange(rKey, colDef.name, null, null);
        }
      }
      onChangePending?.(pendingChanges, originalRows);
      dismissContextMenu();
      return;
    }
    if (additionalSelectedCells.size > 0) {
      for (const { row: r, col: c } of getAltSelectedCells()) {
        const rowData = pageRows[r];
        if (!rowData) continue;
        const rowKey = buildRowKey(rowData, columns, pageOffset + r);
        if (!originalRows.has(rowKey)) {
          const next = new Map(originalRows);
          next.set(rowKey, [...rowData]);
          originalRows = next;
        }
        const { originalIndex } = visibleColumns[c];
        const col = columns[originalIndex];
        if (col) applyPendingChange(rowKey, col.name, rowData[originalIndex], null);
      }
    } else {
      const range = getSelectionRange();
      if (!range) return;
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
    }
    onChangePending?.(pendingChanges, originalRows);
    dismissContextMenu();
  }

  function getContextRows(): { row: CellValue[]; rowKey: string }[] {
    if (selectedRowKeys.size > 1) {
      return pageRows
        .map((r, i) => ({ row: r, rowKey: buildRowKey(r, columns, pageOffset + i) }))
        .filter(({ rowKey }) => selectedRowKeys.has(rowKey));
    }
    if (!contextMenu) return [];
    const { row, rowKey } = contextMenu;
    if (rowKey.startsWith('__new__')) {
      const pendingRow = columns.map((col) => pendingChanges.get(rowKey)?.get(col.name) ?? null);
      return [{ row: pendingRow, rowKey }];
    }
    return [{ row, rowKey }];
  }

  function getContextColRange(): { originalIndex: number; col: ColumnMeta }[] {
    const range = getSelectionRange();
    if (range) {
      const cols: { originalIndex: number; col: ColumnMeta }[] = [];
      for (let c = range.minCol; c <= range.maxCol; c++) {
        const { originalIndex } = visibleColumns[c];
        cols.push({ originalIndex, col: columns[originalIndex] });
      }
      return cols;
    }
    if (newRowFocusedCell) {
      const anchor = newRowAnchorCell ?? newRowFocusedCell;
      const minCol = Math.min(anchor.col, newRowFocusedCell.col);
      const maxCol = Math.max(anchor.col, newRowFocusedCell.col);
      const cols: { originalIndex: number; col: ColumnMeta }[] = [];
      for (let c = minCol; c <= maxCol; c++) {
        const { originalIndex } = visibleColumns[c];
        cols.push({ originalIndex, col: columns[originalIndex] });
      }
      return cols;
    }
    return visibleColumns.map(({ originalIndex }) => ({
      originalIndex,
      col: columns[originalIndex],
    }));
  }

  function copyColumnNames(): void {
    const range = getSelectionRange();
    let names: string[];
    if (range) {
      names = [];
      for (let c = range.minCol; c <= range.maxCol; c++) {
        names.push(visibleColumns[c].col.name);
      }
    } else if (newRowFocusedCell) {
      const anchor = newRowAnchorCell ?? newRowFocusedCell;
      const minCol = Math.min(anchor.col, newRowFocusedCell.col);
      const maxCol = Math.max(anchor.col, newRowFocusedCell.col);
      names = [];
      for (let c = minCol; c <= maxCol; c++) {
        names.push(visibleColumns[c].col.name);
      }
    } else {
      names = visibleColumns.map(({ col }) => col.name);
    }
    navigator.clipboard.writeText(names.join(', ')).catch(() => {});
    dismissContextMenu();
  }

  function sqlEscape(val: CellValue): string {
    if (val === null) return 'NULL';
    if (typeof val === 'number' || typeof val === 'boolean') return String(val);
    return `'${String(val).replace(/'/g, "''")}'`;
  }

  function copyAsJson(): void {
    const contextRows = getContextRows();
    const cols = getContextColRange();
    const objects = contextRows.map(({ row, rowKey }) =>
      Object.fromEntries(
        cols.map(({ originalIndex, col }) => [
          col.name,
          getPendingValue(rowKey, col.name, row[originalIndex]),
        ]),
      ),
    );
    navigator.clipboard
      .writeText(JSON.stringify(objects.length === 1 ? objects[0] : objects, null, 2))
      .catch(() => {});
    dismissContextMenu();
  }

  function copyAsSql(): void {
    const contextRows = getContextRows();
    const cols = getContextColRange();
    const tbl = tableName ?? 'table_name';
    const colList = cols.map(({ col }) => `"${col.name}"`).join(', ');
    const statements = contextRows.map(({ row, rowKey }) => {
      const vals = cols
        .map(({ originalIndex, col }) =>
          sqlEscape(getPendingValue(rowKey, col.name, row[originalIndex])),
        )
        .join(', ');
      return `INSERT INTO "${tbl}" (${colList}) VALUES (${vals});`;
    });
    navigator.clipboard.writeText(statements.join('\n')).catch(() => {});
    dismissContextMenu();
  }

  function copyAsCsv(): void {
    const contextRows = getContextRows();
    const cols = getContextColRange();
    const csvEscape = (v: CellValue) => {
      if (v === null) return '';
      const s = String(v);
      return s.includes(',') || s.includes('"') || s.includes('\n')
        ? `"${s.replace(/"/g, '""')}"`
        : s;
    };
    const header = cols.map(({ col }) => csvEscape(col.name)).join(',');
    const dataLines = contextRows.map(({ row, rowKey }) =>
      cols
        .map(({ originalIndex, col }) =>
          csvEscape(getPendingValue(rowKey, col.name, row[originalIndex])),
        )
        .join(','),
    );
    navigator.clipboard.writeText([header, ...dataLines].join('\n')).catch(() => {});
    dismissContextMenu();
  }

  function cloneRow(): void {
    if (!contextMenu) return;
    const { row, rowKey: srcKey } = contextMenu;
    const id = nextNewRowId++;
    const key = `__new__${id}`;
    pendingNewRows = [...pendingNewRows, { key }];
    const rowMap = new Map<string, CellValue>();
    const isSrcNewRow = srcKey.startsWith('__new__');
    columns.forEach((col, i) => {
      const sourceValue = isSrcNewRow
        ? (pendingChanges.get(srcKey)?.get(col.name) ?? null)
        : (row[i] ?? null);
      rowMap.set(col.name, col.isPrimaryKey || col.isUnique ? null : sourceValue);
    });
    const updated = new Map(pendingChanges);
    updated.set(key, rowMap);
    pendingChanges = updated;
    onChangePending?.(pendingChanges, originalRows);
    onCloneRow?.(row);
    dismissContextMenu();
  }

  function deleteRow(): void {
    if (!contextMenu || !editable || readOnly) return;
    const next = new Map(pendingDeletedRows);
    if (contextMenuSnapshotIsRowSelection && selectedRowKeys.size > 1) {
      const allDeleted = [...selectedRowKeys].every((k) => next.has(k));
      for (let r = 0; r < pageRows.length; r++) {
        const rowData = pageRows[r];
        const key = buildRowKey(rowData, columns, pageOffset + r);
        if (!selectedRowKeys.has(key)) continue;
        if (allDeleted) next.delete(key);
        else next.set(key, [...rowData]);
      }
    } else {
      const { row, rowKey } = contextMenu;
      if (next.has(rowKey)) next.delete(rowKey);
      else next.set(rowKey, [...row]);
    }
    pendingDeletedRows = next;
    onDeleteRowsPending?.(pendingDeletedRows);
    dismissContextMenu();
  }

  function _markRowDeleted(rowKey: string, row: CellValue[]): void {
    const next = new Map(pendingDeletedRows);
    if (next.has(rowKey)) {
      next.delete(rowKey);
    } else {
      next.set(rowKey, [...row]);
    }
    pendingDeletedRows = next;
    onDeleteRowsPending?.(pendingDeletedRows);
  }

  function handleContextMenuKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape') dismissContextMenu();
  }

  function handleWindowClick(e: MouseEvent): void {
    if (!(e.target as Element | null)?.closest('.data-table-wrapper')) {
      rowSelectionMode = false;
      rowAnchor = null;
      rowFocus = null;
      additionalSelectedRowIndices = new Set();
    }
  }
</script>

<svelte:window
  onclick={handleWindowClick}
  onmouseup={() => {
    isDraggingSelection = false;
    isDraggingNewRowSelection = false;
    if (isDraggingRowSelect) justFinishedRowDrag = true;
    isDraggingRowSelect = false;
  }}
/>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  bind:this={tableContainerEl}
  class="data-table-wrapper"
  class:cell-multiline={settings.cellMaxLines > 1}
  style:--cell-max-lines={settings.cellMaxLines}
  onpointermove={onResizePointerMove}
  onpointerup={onResizePointerUp}
  onkeydown={(e) => {
    handleContextMenuKeydown(e);
    handleTableKeydown(e);
  }}
  onfocusout={(e) => {
    if (!tableContainerEl?.contains(e.relatedTarget as Node | null) && editTarget === null) {
      anchorCell = null;
      focusedCell = null;
      isDraggingSelection = false;
      additionalSelectedCells = new Set();
      newRowFocusedCell = null;
      newRowAnchorCell = null;
      isDraggingNewRowSelection = false;
    }
  }}
>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="table-scroll"
    class:selecting={isDraggingSelection}
    bind:this={tableScrollEl}
    ondblclick={(e) => {
      if (editable && !readOnly && !(e.target as Element).closest('tr')) onAddRow?.();
    }}
    onclick={(e) => {
      if (justFinishedRowDrag) {
        justFinishedRowDrag = false;
        return;
      }
      if (rowSelectionMode && !(e.target as Element).closest('tr')) {
        rowSelectionMode = false;
        rowAnchor = null;
        rowFocus = null;
        additionalSelectedRowIndices = new Set();
      }
    }}
  >
    <table class="data-table">
      <thead>
        <tr class="header-row">
          <!-- Row number column header -->
          <th
            class="rownum-header-cell"
            onclick={() => {
              focusedCell = null;
              onDeselect?.();
            }}>#</th
          >
          {#each visibleColumns as { col, originalIndex }, i}
            {@const isSorted = sortColIndex === originalIndex}
            {@const isDragging = colDragName === col.name && colIsDragging}
            {@const prevColName = i > 0 ? visibleColumns[i - 1].col.name : null}
            {@const isDropIndicator =
              (colDropTarget?.name === col.name && colDropTarget.position === 'before') ||
              (prevColName !== null &&
                colDropTarget?.name === prevColName &&
                colDropTarget.position === 'after')}
            {@const isDropAfterLast =
              i === visibleColumns.length - 1 &&
              colDropTarget?.name === col.name &&
              colDropTarget.position === 'after'}
            {@const colLabel = columnRenames[col.name] ?? col.name}
            {@const colIsRenamed =
              columnRenames[col.name] !== undefined && columnRenames[col.name] !== col.name}
            {@const isRenamingThis = renamingHeader?.colName === col.name}
            <th
              class="header-cell"
              class:dragging={isDragging}
              class:drop-indicator={isDropIndicator}
              class:drop-after-last={isDropAfterLast}
              data-col-name={col.name}
              style="width: {colWidths[originalIndex]}px; min-width: {colWidths[
                originalIndex
              ]}px; max-width: {colWidths[originalIndex]}px;"
              title="{colLabel} ({col.dataType}){colIsRenamed ? ` — original: ${col.name}` : ''}"
              onpointerdown={(e) => {
                if (!isRenamingThis) onColHeaderPointerDown(e, col.name);
              }}
              oncontextmenu={(e) => {
                if (onRenameColumn) openHeaderContextMenu(e, col.name);
              }}
            >
              {#if isRenamingThis}
                <input
                  bind:this={renameHeaderInputEl}
                  class="header-rename-input"
                  type="text"
                  bind:value={renamingHeader!.value}
                  onkeydown={handleHeaderRenameKeydown}
                  onblur={commitHeaderRename}
                  onclick={(e) => e.stopPropagation()}
                  aria-label="Rename column {col.name}"
                />
              {:else}
                <button
                  class="header-btn"
                  onclick={() => toggleSort(originalIndex)}
                  aria-label="Sort by {colLabel}"
                  title="Sort by {colLabel}"
                >
                  {#if col.isPrimaryKey}
                    <svg
                      class="pk-icon"
                      width="11"
                      height="11"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="2"
                    >
                      <circle cx="8" cy="9" r="4"></circle>
                      <path d="M11 12l7 7"></path>
                      <path d="M16 17l2-2"></path>
                    </svg>
                  {/if}
                  <span class="header-label">
                    <span class="header-name" class:header-name--renamed={colIsRenamed}
                      >{colLabel}</span
                    >
                    <span class="header-type">{col.dataType}</span>
                  </span>
                  {#if isSorted && sortDir !== 'none'}
                    <span
                      class="sort-indicator"
                      aria-label={sortDir === 'asc' ? 'ascending' : 'descending'}
                    >
                      {sortDir === 'asc' ? '▲' : '▼'}
                    </span>
                  {/if}
                </button>
              {/if}

              <div
                class="resize-handle"
                role="separator"
                aria-label="Resize {col.name} column"
                onpointerdown={(e) => onResizePointerDown(e, originalIndex)}
              ></div>
            </th>
          {/each}
          <th class="filler-header-cell"></th>
        </tr>
      </thead>

      <tbody>
        {#if settings.newRowPosition === 'top'}
          {#each pendingNewRows as newRow}
            <tr class="data-row new-row" data-new-row-key={newRow.key}>
              <td
                class="rownum-cell"
                oncontextmenu={(e) => {
                  e.preventDefault();
                  activeMenuDismiss?.();
                  contextMenu = {
                    x: e.clientX,
                    y: e.clientY,
                    rowKey: newRow.key,
                    row: [],
                    colName: null,
                    isNewRow: true,
                  };
                  contextMenuSnapshotHasFocus = newRowFocusedCell !== null;
                  contextMenuSnapshotIsMultiCell = newRowSelectionIsMultiCell();
                  contextMenuSnapshotIsMultiCol =
                    newRowSelectionIsMultiCell() &&
                    (newRowAnchorCell?.col ?? newRowFocusedCell?.col ?? 0) !==
                      (newRowFocusedCell?.col ?? 0);
                  activeMenuDismiss = () => {
                    contextMenu = null;
                  };
                }}
              >
                <span class="new-row-indicator" aria-label="New row">+</span>
              </td>
              {#each visibleColumns as { col, originalIndex }, colIndex}
                {@const currentValue = pendingChanges.get(newRow.key)?.get(col.name) ?? null}
                {@const isRequiredEmpty =
                  !col.nullable &&
                  !col.isAutoIncrement &&
                  col.defaultValue == null &&
                  (currentValue === null || currentValue === '')}
                {@const typeCategory = getDataTypeCategory(col.dataType)}
                <td
                  class="data-cell"
                  class:cell-number={typeCategory === 'number'}
                  class:cell-timestamp={typeCategory === 'timestamp'}
                  class:cell-editable={editable && !readOnly}
                  class:cell-selected={isNewRowCellInSelection(newRow.key, colIndex)}
                  class:cell-focused={newRowFocusedCell?.rowKey === newRow.key &&
                    newRowFocusedCell?.col === colIndex}
                  class:cell-required-empty={isRequiredEmpty}
                  style="width: {colWidths[originalIndex]}px; min-width: {colWidths[
                    originalIndex
                  ]}px; max-width: {colWidths[originalIndex]}px;"
                  tabindex="0"
                  ondblclick={(e) =>
                    handleNewRowCellDblClick(e, newRow.key, currentValue, col, originalIndex)}
                  oncontextmenu={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    if (!isNewRowCellInSelection(newRow.key, colIndex)) {
                      focusedCell = null;
                      anchorCell = null;
                      newRowAnchorCell = { rowKey: newRow.key, col: colIndex };
                      newRowFocusedCell = { rowKey: newRow.key, col: colIndex };
                    }
                    activeMenuDismiss?.();
                    contextMenu = {
                      x: e.clientX,
                      y: e.clientY,
                      rowKey: newRow.key,
                      row: [],
                      colName: col.name,
                      isNewRow: true,
                    };
                    contextMenuSnapshotHasFocus = true;
                    contextMenuSnapshotIsMultiCell = newRowSelectionIsMultiCell();
                    const _minCol = Math.min(
                      newRowAnchorCell?.col ?? colIndex,
                      newRowFocusedCell?.col ?? colIndex,
                    );
                    const _maxCol = Math.max(
                      newRowAnchorCell?.col ?? colIndex,
                      newRowFocusedCell?.col ?? colIndex,
                    );
                    contextMenuSnapshotIsMultiCol = _minCol !== _maxCol;
                    activeMenuDismiss = () => {
                      contextMenu = null;
                    };
                  }}
                  onmousedown={(e) => {
                    if (e.button === 2 || (e.button === 0 && e.ctrlKey)) {
                      if (isNewRowCellInSelection(newRow.key, colIndex)) skipNextFocusReset = true;
                      return;
                    }
                    if (e.button !== 0) return;
                    focusedCell = null;
                    anchorCell = null;
                    rowSelectionMode = false;
                    skipNextFocusReset = true;
                    if (e.shiftKey && newRowFocusedCell) {
                      newRowFocusedCell = { rowKey: newRow.key, col: colIndex };
                    } else {
                      newRowAnchorCell = { rowKey: newRow.key, col: colIndex };
                      newRowFocusedCell = { rowKey: newRow.key, col: colIndex };
                      isDraggingNewRowSelection = true;
                    }
                  }}
                  onmouseenter={() => {
                    if (isDraggingNewRowSelection)
                      newRowFocusedCell = { rowKey: newRow.key, col: colIndex };
                  }}
                  onfocus={() => {
                    focusedCell = null;
                    anchorCell = null;
                    rowSelectionMode = false;
                    if (skipNextFocusReset) {
                      skipNextFocusReset = false;
                      return;
                    }
                    newRowAnchorCell = { rowKey: newRow.key, col: colIndex };
                    newRowFocusedCell = { rowKey: newRow.key, col: colIndex };
                  }}
                >
                  <div class="cell-inner">
                    <span class="cell-content">
                      {#if currentValue === null}
                        <span class="null-value">NULL</span>
                      {:else if currentValue === ''}
                        <span class="empty-value">EMPTY</span>
                      {:else if typeCategory === 'boolean' && (typeof currentValue === 'boolean' || typeof currentValue === 'number')}
                        <span
                          class="bool-value"
                          class:bool-true={currentValue}
                          class:bool-false={!currentValue}
                        >
                          {#if currentValue}<svg
                              width="12"
                              height="12"
                              viewBox="0 0 24 24"
                              fill="none"
                              stroke="currentColor"
                              stroke-width="2.5"
                              stroke-linecap="round"
                              stroke-linejoin="round"
                              aria-hidden="true"><polyline points="20 6 9 17 4 12" /></svg
                            >{:else}<svg
                              width="12"
                              height="12"
                              viewBox="0 0 24 24"
                              fill="none"
                              stroke="currentColor"
                              stroke-width="2.5"
                              stroke-linecap="round"
                              aria-hidden="true"
                              ><line x1="18" y1="6" x2="6" y2="18" /><line
                                x1="6"
                                y1="6"
                                x2="18"
                                y2="18"
                              /></svg
                            >{/if}
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
        {/if}

        {#each pageRows as row, rowIndex (rowIndex)}
          {@const processedRowIndex = rowIndex}
          {@const rowKey = buildRowKey(row, columns, pageOffset + processedRowIndex)}
          {@const isSelected = selectedRowKeys.has(rowKey)}
          {@const rowDirty = isRowPending(rowKey)}
          {@const isDeleted = pendingDeletedRows.has(rowKey)}
          <tr
            class="data-row"
            class:row-selected={isSelected}
            class:row-deleted={isDeleted}
            onclick={(e) => handleRowClick(e, rowKey)}
            oncontextmenu={(e) => handleRowContextMenu(e, row, processedRowIndex)}
          >
            <!-- Row number / dirty indicator -->
            <td
              class="rownum-cell"
              tabindex="-1"
              onmousedown={(e) => {
                if (e.button !== 0) return;
                e.preventDefault();
                anchorCell = null;
                focusedCell = null;
                additionalSelectedCells = new Set();
                if (e.altKey) {
                  if (!rowSelectionMode) {
                    rowSelectionMode = true;
                    additionalSelectedRowIndices = new Set([rowIndex]);
                    rowAnchor = rowIndex;
                    rowFocus = rowIndex;
                  } else {
                    const next = new Set(additionalSelectedRowIndices);
                    if (next.size === 0 && rowAnchor !== null) {
                      const lo = Math.min(rowAnchor, rowFocus ?? rowAnchor);
                      const hi = Math.max(rowAnchor, rowFocus ?? rowAnchor);
                      for (let i = lo; i <= hi; i++) next.add(i);
                    }
                    if (next.has(rowIndex)) next.delete(rowIndex);
                    else next.add(rowIndex);
                    additionalSelectedRowIndices = next;
                  }
                } else if (e.shiftKey && rowSelectionMode && rowAnchor !== null) {
                  additionalSelectedRowIndices = new Set();
                  rowFocus = rowIndex;
                } else {
                  rowSelectionMode = true;
                  rowAnchor = rowIndex;
                  rowFocus = rowIndex;
                  additionalSelectedRowIndices = new Set();
                  isDraggingRowSelect = true;
                }
                onDeselect?.();
                (e.currentTarget as HTMLElement).focus();
              }}
              onclick={(e) => e.stopPropagation()}
              onmouseenter={() => {
                if (isDraggingRowSelect && rowAnchor !== null) rowFocus = rowIndex;
              }}
            >
              {#if isDeleted}
                <span class="row-deleted-indicator" aria-label="Row pending deletion">–</span>
              {:else if rowDirty}
                <span class="row-dirty-dot" aria-label="Row has unsaved changes"></span>
              {:else}
                <span class="rownum">{rowOffset + pageOffset + rowIndex + 1}</span>
              {/if}
            </td>

            {#each visibleColumns as { col, originalIndex }, colIndex}
              {@const cellValue = getPendingValue(rowKey, col.name, row[originalIndex])}
              {@const isPending = hasPendingChange(rowKey, col.name)}
              {@const typeCategory = getDataTypeCategory(col.dataType)}
              {@const isRequiredEmpty =
                isPending &&
                (cellValue === null || cellValue === '') &&
                !col.nullable &&
                !col.isAutoIncrement &&
                col.defaultValue == null}
              {@const cellMatches =
                !!searchTerm &&
                cellValue !== null &&
                String(cellValue).toLowerCase().includes(searchTerm.toLowerCase())}
              <td
                class="data-cell"
                class:cell-number={typeCategory === 'number'}
                class:cell-timestamp={typeCategory === 'timestamp'}
                class:cell-editable={editable && !readOnly}
                class:cell-selected={isCellInSelection(rowIndex, colIndex)}
                class:cell-focused={focusedCell?.row === rowIndex && focusedCell?.col === colIndex}
                class:cell-required-empty={isRequiredEmpty}
                class:cell-fk={col.isForeignKey && cellValue !== null && !!onForeignKeyClick}
                class:cell-search-match={cellMatches}
                style="width: {colWidths[originalIndex]}px; min-width: {colWidths[
                  originalIndex
                ]}px; max-width: {colWidths[originalIndex]}px;"
                tabindex="0"
                ondblclick={(e) => handleCellDblClick(e, row, processedRowIndex, originalIndex)}
                oncontextmenu={(e) => {
                  e.stopPropagation();
                  if (!rowSelectionMode && !isCellInSelection(rowIndex, colIndex)) {
                    anchorCell = { row: rowIndex, col: colIndex };
                    focusedCell = { row: rowIndex, col: colIndex };
                  }
                  handleRowContextMenu(e, row, processedRowIndex, col.name);
                }}
                onmousedown={(e) => {
                  if (
                    e.button === 0 &&
                    e.metaKey &&
                    e.shiftKey &&
                    col.isForeignKey &&
                    cellValue !== null
                  ) {
                    e.stopPropagation();
                    triggerQuickView(col.name, cellValue, rowKey);
                    return;
                  }
                  if (e.button === 0 && e.metaKey && col.isForeignKey && cellValue !== null) {
                    e.stopPropagation();
                    onForeignKeyClick?.(col.name, cellValue);
                    return;
                  }
                  if (e.button === 2 || (e.button === 0 && e.ctrlKey)) {
                    if (isCellInSelection(rowIndex, colIndex)) skipNextFocusReset = true;
                    return;
                  }
                  if (e.button !== 0) return;
                  if (e.altKey && !e.shiftKey && !e.metaKey) {
                    const key = cellKey(rowIndex, colIndex);
                    const next = new Set(additionalSelectedCells);
                    if (next.size === 0 && anchorCell)
                      next.add(cellKey(anchorCell.row, anchorCell.col));
                    if (next.has(key)) {
                      next.delete(key);
                    } else {
                      next.add(key);
                    }
                    additionalSelectedCells = next;
                    skipNextFocusReset = true;
                    anchorCell = { row: rowIndex, col: colIndex };
                    focusedCell = { row: rowIndex, col: colIndex };
                    return;
                  }
                  if (e.shiftKey && focusedCell) {
                    additionalSelectedCells = new Set();
                    rowSelectionMode = false;
                    rowAnchor = null;
                    rowFocus = null;
                    additionalSelectedRowIndices = new Set();
                    skipNextFocusReset = true;
                    focusedCell = { row: rowIndex, col: colIndex };
                  } else {
                    additionalSelectedCells = new Set();
                    rowSelectionMode = false;
                    rowAnchor = null;
                    rowFocus = null;
                    additionalSelectedRowIndices = new Set();
                    skipNextFocusReset = true;
                    anchorCell = { row: rowIndex, col: colIndex };
                    focusedCell = { row: rowIndex, col: colIndex };
                    isDraggingSelection = true;
                  }
                }}
                onmouseenter={() => {
                  if (isDraggingSelection) focusedCell = { row: rowIndex, col: colIndex };
                  if (isDraggingRowSelect) rowFocus = rowIndex;
                }}
                onfocus={() => {
                  newRowFocusedCell = null;
                  newRowAnchorCell = null;
                  if (skipNextFocusReset) {
                    skipNextFocusReset = false;
                    onCellSelect?.(originalIndex, row);
                    return;
                  }
                  if (rowSelectionMode) return;
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
                        {#if settings.booleanDisplay === 'true-false'}
                          {cellValue ? 'True' : 'False'}
                        {:else if settings.booleanDisplay === '1-0'}
                          {cellValue ? '1' : '0'}
                        {:else if settings.booleanDisplay === 'as-saved'}
                          {String(cellValue)}
                        {:else if cellValue}
                          <svg
                            width="12"
                            height="12"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2.5"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            aria-hidden="true"><polyline points="20 6 9 17 4 12" /></svg
                          >
                        {:else}
                          <svg
                            width="12"
                            height="12"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2.5"
                            stroke-linecap="round"
                            aria-hidden="true"
                            ><line x1="18" y1="6" x2="6" y2="18" /><line
                              x1="6"
                              y1="6"
                              x2="18"
                              y2="18"
                            /></svg
                          >
                        {/if}
                      </span>
                    {:else if cellMatches}
                      {@html highlightText(formatCell(cellValue), searchTerm)}
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
            <td class="filler-cell"></td>
          </tr>
          {#if quickViewState?.triggerRowKey === rowKey}
            <tr class="quick-view-row">
              <td class="quick-view-cell" colspan={visibleColumns.length + 2} tabindex="-1">
                {#if quickViewState.loading}
                  <div class="quick-view-panel" style="width:{tableScrollWidth}px">
                    <div class="quick-view-header">
                      <span class="quick-view-title">Loading…</span>
                      <button
                        class="quick-view-close"
                        onclick={() => (quickViewState = null)}
                        aria-label="Close quick view"
                        ><svg
                          width="11"
                          height="11"
                          viewBox="0 0 24 24"
                          fill="none"
                          stroke="currentColor"
                          stroke-width="2.5"
                          stroke-linecap="round"
                          aria-hidden="true"
                          ><line x1="18" y1="6" x2="6" y2="18" /><line
                            x1="6"
                            y1="6"
                            x2="18"
                            y2="18"
                          /></svg
                        ></button
                      >
                    </div>
                  </div>
                {:else if quickViewState.data !== null}
                  {@const qd = quickViewState.data}
                  <div class="quick-view-panel" style="width:{tableScrollWidth}px">
                    <div class="quick-view-header">
                      <span class="quick-view-title">
                        <span class="quick-view-table-name">{qd.tableName}</span>
                        <span class="quick-view-sep"> · </span>
                        <span class="quick-view-filter">{qd.refColumn} = {qd.refValue}</span>
                      </span>
                      <div class="quick-view-actions">
                        {#if onForeignKeyClick}
                          <button
                            class="quick-view-go"
                            onclick={() => {
                              onForeignKeyClick!(
                                quickViewState!.triggerColName,
                                quickViewState!.triggerCellValue,
                              );
                              quickViewState = null;
                            }}
                            aria-label="Go to row in table"
                            ><svg
                              width="11"
                              height="11"
                              viewBox="0 0 24 24"
                              fill="none"
                              stroke="currentColor"
                              stroke-width="2.5"
                              stroke-linecap="round"
                              stroke-linejoin="round"
                              aria-hidden="true"
                              ><path d="M5 12h14" /><path d="M13 6l6 6-6 6" /></svg
                            ></button
                          >
                        {/if}
                        <!-- svelte-ignore a11y_autofocus -->
                        <button
                          class="quick-view-close"
                          autofocus
                          onclick={() => (quickViewState = null)}
                          aria-label="Close quick view"
                          ><svg
                            width="11"
                            height="11"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2.5"
                            stroke-linecap="round"
                            aria-hidden="true"
                            ><line x1="18" y1="6" x2="6" y2="18" /><line
                              x1="6"
                              y1="6"
                              x2="18"
                              y2="18"
                            /></svg
                          ></button
                        >
                      </div>
                    </div>
                    {#if qd.row === null}
                      <div class="quick-view-empty">No matching row found</div>
                    {:else}
                      <div class="quick-view-scroll">
                        <table class="quick-view-table">
                          <thead>
                            <tr>
                              {#each qd.columns as qcol}
                                <th
                                  class="quick-view-th"
                                  class:quick-view-th-pk={qcol.isPrimaryKey}
                                >
                                  {#if qcol.isPrimaryKey}
                                    <svg
                                      class="quick-view-pk"
                                      width="9"
                                      height="9"
                                      viewBox="0 0 24 24"
                                      fill="none"
                                      stroke="currentColor"
                                      stroke-width="2"
                                    >
                                      <circle cx="8" cy="9" r="4"></circle><path d="M11 12l7 7"
                                      ></path><path d="M16 17l2-2"></path>
                                    </svg>
                                  {/if}
                                  {qcol.name}
                                </th>
                              {/each}
                            </tr>
                          </thead>
                          <tbody>
                            <tr>
                              {#each qd.columns as qcol, qi}
                                <td
                                  class="quick-view-td"
                                  class:quick-view-td-number={getDataTypeCategory(qcol.dataType) ===
                                    'number'}
                                >
                                  {#if qd.row[qi] === null}
                                    <span class="null-value">NULL</span>
                                  {:else if qd.row[qi] === ''}
                                    <span class="empty-value">EMPTY</span>
                                  {:else}
                                    {qd.row[qi]}
                                  {/if}
                                </td>
                              {/each}
                            </tr>
                          </tbody>
                        </table>
                      </div>
                    {/if}
                  </div>
                {/if}
              </td>
            </tr>
          {/if}
        {/each}

        {#if settings.newRowPosition !== 'top'}
          {#each pendingNewRows as newRow}
            <tr class="data-row new-row" data-new-row-key={newRow.key}>
              <td
                class="rownum-cell"
                oncontextmenu={(e) => {
                  e.preventDefault();
                  activeMenuDismiss?.();
                  contextMenu = {
                    x: e.clientX,
                    y: e.clientY,
                    rowKey: newRow.key,
                    row: [],
                    colName: null,
                    isNewRow: true,
                  };
                  contextMenuSnapshotHasFocus = newRowFocusedCell !== null;
                  contextMenuSnapshotIsMultiCell = newRowSelectionIsMultiCell();
                  contextMenuSnapshotIsMultiCol =
                    newRowSelectionIsMultiCell() &&
                    (newRowAnchorCell?.col ?? newRowFocusedCell?.col ?? 0) !==
                      (newRowFocusedCell?.col ?? 0);
                  activeMenuDismiss = () => {
                    contextMenu = null;
                  };
                }}
              >
                <span class="new-row-indicator" aria-label="New row">+</span>
              </td>
              {#each visibleColumns as { col, originalIndex }, colIndex}
                {@const currentValue = pendingChanges.get(newRow.key)?.get(col.name) ?? null}
                {@const isRequiredEmpty =
                  !col.nullable &&
                  !col.isAutoIncrement &&
                  col.defaultValue == null &&
                  (currentValue === null || currentValue === '')}
                {@const typeCategory = getDataTypeCategory(col.dataType)}
                <td
                  class="data-cell"
                  class:cell-number={typeCategory === 'number'}
                  class:cell-timestamp={typeCategory === 'timestamp'}
                  class:cell-editable={editable && !readOnly}
                  class:cell-selected={isNewRowCellInSelection(newRow.key, colIndex)}
                  class:cell-focused={newRowFocusedCell?.rowKey === newRow.key &&
                    newRowFocusedCell?.col === colIndex}
                  class:cell-required-empty={isRequiredEmpty}
                  style="width: {colWidths[originalIndex]}px; min-width: {colWidths[
                    originalIndex
                  ]}px; max-width: {colWidths[originalIndex]}px;"
                  tabindex="0"
                  ondblclick={(e) =>
                    handleNewRowCellDblClick(e, newRow.key, currentValue, col, originalIndex)}
                  oncontextmenu={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    if (!isNewRowCellInSelection(newRow.key, colIndex)) {
                      focusedCell = null;
                      anchorCell = null;
                      newRowAnchorCell = { rowKey: newRow.key, col: colIndex };
                      newRowFocusedCell = { rowKey: newRow.key, col: colIndex };
                    }
                    activeMenuDismiss?.();
                    contextMenu = {
                      x: e.clientX,
                      y: e.clientY,
                      rowKey: newRow.key,
                      row: [],
                      colName: col.name,
                      isNewRow: true,
                    };
                    contextMenuSnapshotHasFocus = true;
                    contextMenuSnapshotIsMultiCell = newRowSelectionIsMultiCell();
                    const _minCol = Math.min(
                      newRowAnchorCell?.col ?? colIndex,
                      newRowFocusedCell?.col ?? colIndex,
                    );
                    const _maxCol = Math.max(
                      newRowAnchorCell?.col ?? colIndex,
                      newRowFocusedCell?.col ?? colIndex,
                    );
                    contextMenuSnapshotIsMultiCol = _minCol !== _maxCol;
                    activeMenuDismiss = () => {
                      contextMenu = null;
                    };
                  }}
                  onmousedown={(e) => {
                    if (e.button === 2 || (e.button === 0 && e.ctrlKey)) {
                      if (isNewRowCellInSelection(newRow.key, colIndex)) skipNextFocusReset = true;
                      return;
                    }
                    if (e.button !== 0) return;
                    focusedCell = null;
                    anchorCell = null;
                    rowSelectionMode = false;
                    skipNextFocusReset = true;
                    if (e.shiftKey && newRowFocusedCell) {
                      newRowFocusedCell = { rowKey: newRow.key, col: colIndex };
                    } else {
                      newRowAnchorCell = { rowKey: newRow.key, col: colIndex };
                      newRowFocusedCell = { rowKey: newRow.key, col: colIndex };
                      isDraggingNewRowSelection = true;
                    }
                  }}
                  onmouseenter={() => {
                    if (isDraggingNewRowSelection)
                      newRowFocusedCell = { rowKey: newRow.key, col: colIndex };
                  }}
                  onfocus={() => {
                    focusedCell = null;
                    anchorCell = null;
                    rowSelectionMode = false;
                    if (skipNextFocusReset) {
                      skipNextFocusReset = false;
                      return;
                    }
                    newRowAnchorCell = { rowKey: newRow.key, col: colIndex };
                    newRowFocusedCell = { rowKey: newRow.key, col: colIndex };
                  }}
                >
                  <div class="cell-inner">
                    <span class="cell-content">
                      {#if currentValue === null}
                        <span class="null-value">NULL</span>
                      {:else if currentValue === ''}
                        <span class="empty-value">EMPTY</span>
                      {:else if typeCategory === 'boolean' && (typeof currentValue === 'boolean' || typeof currentValue === 'number')}
                        <span
                          class="bool-value"
                          class:bool-true={currentValue}
                          class:bool-false={!currentValue}
                        >
                          {#if currentValue}<svg
                              width="12"
                              height="12"
                              viewBox="0 0 24 24"
                              fill="none"
                              stroke="currentColor"
                              stroke-width="2.5"
                              stroke-linecap="round"
                              stroke-linejoin="round"
                              aria-hidden="true"><polyline points="20 6 9 17 4 12" /></svg
                            >{:else}<svg
                              width="12"
                              height="12"
                              viewBox="0 0 24 24"
                              fill="none"
                              stroke="currentColor"
                              stroke-width="2.5"
                              stroke-linecap="round"
                              aria-hidden="true"
                              ><line x1="18" y1="6" x2="6" y2="18" /><line
                                x1="6"
                                y1="6"
                                x2="18"
                                y2="18"
                              /></svg
                            >{/if}
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
        {/if}
      </tbody>
    </table>
  </div>

  {#if processedRows.length === 0}
    <div class="empty-overlay">
      {rows.length === 0 ? 'No rows to display.' : 'No rows match the current filters.'}
    </div>
  {/if}

  <!-- Context menu -->
  <CtxMenuContainer
    x={contextMenu?.x ?? 0}
    y={contextMenu?.y ?? 0}
    open={contextMenu !== null}
    onclose={dismissContextMenu}
    minWidth={220}
    zIndex={400}
  >
    {#if contextMenu !== null}
      {#if contextMenu.isNewRow}
        {#if contextMenu.colName !== null}
          {#if editable && !readOnly && !contextMenuSnapshotIsMultiCell}
            <CtxItem onclick={() => openNewRowInlineEditFromContextMenu()}>Edit</CtxItem>
            <CtxItem onclick={() => openNewRowModalFromContextMenu()}>Edit in modal</CtxItem>
          {/if}
          {#if !contextMenuSnapshotIsMultiCell}
            <CtxItem onclick={() => openNewRowViewModalFromContextMenu()}>View in modal</CtxItem>
          {/if}
          <CtxSep />
          {#if editable && !readOnly}
            <CtxItem onclick={() => setSelectionNull()}>Set to NULL</CtxItem>
            {#if !contextMenuSnapshotIsMultiCell && contextMenuColIsDatetime}
              <CtxItem onclick={setNowFromContextMenu}>Set to NOW</CtxItem>
            {/if}
            <CtxSep />
          {/if}
          {#if contextMenuSnapshotHasFocus}
            <CtxItem
              onclick={() => {
                copySelection();
                dismissContextMenu();
              }}
            >
              {contextMenuSnapshotIsMultiCell ? 'Copy selection' : 'Copy cell'}
            </CtxItem>
            {#if editable && !readOnly}
              <CtxItem
                onclick={() => {
                  cutSelection();
                  dismissContextMenu();
                }}
              >
                {contextMenuSnapshotIsMultiCell ? 'Cut selection' : 'Cut cell'}
              </CtxItem>
              <CtxItem
                disabled={!contextMenuClipboardHasContent}
                onclick={() => {
                  pasteFromClipboard();
                  dismissContextMenu();
                }}
              >
                Paste
              </CtxItem>
            {/if}
          {/if}
          <CtxSep />
          <CtxItem onclick={() => copyAsJson()}>Copy cell as JSON</CtxItem>
          <CtxItem onclick={() => copyAsSql()}>Copy cell as SQL</CtxItem>
          <CtxItem onclick={() => copyAsCsv()}>Copy cell as CSV</CtxItem>
          <CtxSep />
          <CtxItem onclick={() => copyColumnNames()}>
            {contextMenuSnapshotIsMultiCol ? 'Copy column names' : 'Copy column name'}
          </CtxItem>
          {#if onConnectColumn && !contextMenuSnapshotIsMultiCell}
            <CtxSep />
            <CtxItem
              onclick={() => {
                onConnectColumn!(contextMenu!.colName!);
                dismissContextMenu();
              }}
            >
              Connect column…
            </CtxItem>
          {/if}
          <CtxSep />
        {/if}
        <CtxItem onclick={() => copyRowTabSeparated(getContextRows()[0]?.row ?? [])}>
          Copy row (tab-separated)
        </CtxItem>
        <CtxItem onclick={() => copyAsJson()}>Copy row as JSON</CtxItem>
        <CtxItem onclick={() => copyAsCsv()}>Copy row as CSV</CtxItem>
        {#if editable && !readOnly}
          <CtxSep />
          <CtxItem onclick={() => cloneRow()}>Clone row</CtxItem>
        {/if}
        <CtxSep />
        <CtxItem danger onclick={() => deleteNewRow(contextMenu!.rowKey)}>Discard new row</CtxItem>
      {:else if contextMenuSnapshotIsRowSelection || contextMenu.colName === null}
        {#if selectedRowKeys.size > 1}
          <CtxItem onclick={() => copySelectedRowsTabSeparated()}>
            Copy {selectedRowKeys.size} rows (tab-separated)
          </CtxItem>
        {:else}
          <CtxItem onclick={() => copyRowTabSeparated(contextMenu!.row)}>
            Copy row (tab-separated)
          </CtxItem>
        {/if}
        <CtxItem onclick={() => copyAsJson()}>
          {selectedRowKeys.size > 1
            ? `Copy ${selectedRowKeys.size} rows as JSON`
            : 'Copy row as JSON'}
        </CtxItem>
        <CtxItem onclick={() => copyAsCsv()}>
          {selectedRowKeys.size > 1
            ? `Copy ${selectedRowKeys.size} rows as CSV`
            : 'Copy row as CSV'}
        </CtxItem>
        {#if editable && !readOnly}
          <CtxSep />
          {#if selectedRowKeys.size <= 1}
            <CtxItem onclick={() => cloneRow()}>Clone row</CtxItem>
            <CtxSep />
          {/if}
          <CtxItem danger onclick={() => deleteRow()}>
            {selectedRowKeys.size > 1
              ? [...selectedRowKeys].every((k) => pendingDeletedRows.has(k))
                ? `Undelete ${selectedRowKeys.size} rows`
                : `Delete ${selectedRowKeys.size} rows`
              : pendingDeletedRows.has(contextMenu!.rowKey)
                ? 'Undelete row'
                : 'Delete row'}
          </CtxItem>
        {/if}
      {:else}
        {#if contextMenu.colName && editable && !readOnly && !contextMenuSnapshotIsMultiCell}
          <CtxItem onclick={() => openInlineEditFromContextMenu()}>Edit</CtxItem>
          <CtxItem onclick={() => openModalFromContextMenu()}>Edit in modal</CtxItem>
        {/if}
        {#if contextMenu.colName && !contextMenuSnapshotIsMultiCell}
          <CtxItem onclick={() => openViewModalFromContextMenu()}>View in modal</CtxItem>
        {/if}
        <CtxSep />
        {#if editable && !readOnly}
          <CtxItem onclick={() => setSelectionNull()}>Set to NULL</CtxItem>
        {/if}
        {#if contextMenu.colName && editable && !readOnly && !contextMenuSnapshotIsMultiCell && contextMenuColIsDatetime}
          <CtxItem onclick={setNowFromContextMenu}>Set to NOW</CtxItem>
        {/if}
        {#if editable && !readOnly}
          <CtxSep />
        {/if}
        {#if contextMenu.colName && hasPendingChange(contextMenu.rowKey, contextMenu.colName)}
          <CtxItem danger onclick={() => discardCellEdit()}>Discard edit</CtxItem>
          <CtxSep />
        {/if}
        {#if contextMenuSnapshotHasFocus}
          <CtxItem
            onclick={() => {
              copySelection();
              dismissContextMenu();
            }}
          >
            {contextMenuSnapshotIsMultiCell ? 'Copy selection' : 'Copy cell'}
          </CtxItem>
          {#if editable && !readOnly}
            <CtxItem
              onclick={() => {
                cutSelection();
                dismissContextMenu();
              }}
            >
              {contextMenuSnapshotIsMultiCell ? 'Cut selection' : 'Cut cell'}
            </CtxItem>
            <CtxItem
              disabled={!contextMenuClipboardHasContent}
              onclick={() => {
                pasteFromClipboard();
                dismissContextMenu();
              }}
            >
              Paste
            </CtxItem>
          {/if}
        {/if}
        <CtxSep />
        <CtxItem onclick={() => copyAsJson()}>Copy cell as JSON</CtxItem>
        <CtxItem onclick={() => copyAsSql()}>Copy cell as SQL</CtxItem>
        <CtxItem onclick={() => copyAsCsv()}>Copy cell as CSV</CtxItem>
        <CtxSep />
        <CtxItem onclick={() => copyColumnNames()}>
          {contextMenuSnapshotIsMultiCol ? 'Copy column names' : 'Copy column name'}
        </CtxItem>
        {#if contextMenu.colName && onConnectColumn && !contextMenuSnapshotIsMultiCell}
          <CtxSep />
          <CtxItem
            onclick={() => {
              onConnectColumn!(contextMenu!.colName!);
              dismissContextMenu();
            }}
          >
            Connect column…
          </CtxItem>
        {/if}
        <CtxSep />
        {#if selectedRowKeys.size > 1}
          <CtxItem onclick={() => copySelectedRowsTabSeparated()}>
            Copy {selectedRowKeys.size} selected rows (tab-separated)
          </CtxItem>
        {:else}
          <CtxItem onclick={() => copyRowTabSeparated(contextMenu!.row)}>
            Copy row (tab-separated)
          </CtxItem>
        {/if}
        {#if editable && !readOnly}
          <CtxSep />
          <CtxItem onclick={() => cloneRow()}>Clone row</CtxItem>
          <CtxSep />
          <CtxItem danger onclick={() => deleteRow()}>
            {pendingDeletedRows.has(contextMenu!.rowKey) ? 'Undelete row' : 'Delete row'}
          </CtxItem>
        {/if}
      {/if}
    {/if}
  </CtxMenuContainer>

  <!-- Header context menu -->
  <CtxMenuContainer
    x={headerContextMenu?.x ?? 0}
    y={headerContextMenu?.y ?? 0}
    open={headerContextMenu !== null}
    onclose={() => (headerContextMenu = null)}
    minWidth={160}
    zIndex={400}
  >
    <CtxItem onclick={() => startHeaderRename(headerContextMenu!.colName)}>Rename column</CtxItem>
    {#if headerContextMenu !== null && columnRenames[headerContextMenu.colName] !== undefined && columnRenames[headerContextMenu.colName] !== headerContextMenu.colName}
      <CtxItem
        onclick={() => {
          onRenameColumn?.(headerContextMenu!.colName, headerContextMenu!.colName);
          headerContextMenu = null;
        }}>Reset name</CtxItem
      >
    {/if}
  </CtxMenuContainer>

  <!-- Inline cell editor overlay -->
  {#if editTarget !== null}
    <CellEditor
      bind:this={cellEditorInstance}
      value={editTarget.value}
      originalValue={editTarget.originalValue}
      dataType={editTarget.dataType}
      nullable={editTarget.nullable}
      initialViewportTop={editTarget.initialViewportTop}
      initialViewportLeft={editTarget.initialViewportLeft}
      width={editTarget.width}
      height={editTarget.height}
      scrollEl={tableScrollEl}
      panelEl={tableContainerEl}
      onConfirm={confirmEdit}
      onCancel={cancelEdit}
      onTab={handleTabFromEditor}
      onTabConfirm={handleTabConfirm}
      onOpenModal={openInlineAsModal}
      {connectionId}
      {database}
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
    nullable={modalTarget.nullable}
    onConfirm={confirmModalEdit}
    onCancel={cancelModalEdit}
    {connectionId}
    {database}
  />
{/if}

<!-- Modal cell viewer (portal, full-screen, readonly) -->
{#if viewModalTarget !== null}
  <CellViewModal
    value={viewModalTarget.value}
    colName={viewModalTarget.colName}
    dataType={viewModalTarget.dataType}
    onClose={() => {
      viewModalTarget = null;
    }}
  />
{/if}

<style>
  .data-table-wrapper {
    position: relative;
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    overflow: clip;
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
    table-layout: fixed;
    width: max-content;
    min-width: 100%;
  }

  /* ── Header ─────────────────────────────────────────────────────────────── */

  thead {
    position: sticky;
    top: 0;
    z-index: 2;
  }

  .header-row {
    height: 34px;
    background: var(--color-table-header-bg);
    background-attachment: fixed;
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
    -webkit-user-select: none;
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

  .header-name--renamed {
    font-style: italic;
  }

  .header-rename-input {
    width: calc(100% - 8px);
    margin: 0 4px;
    padding: 2px 4px;
    background: var(--color-bg-primary);
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    outline: none;
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

  .header-cell.drop-indicator {
    box-shadow: inset 2px 0 0 var(--color-accent);
  }

  .header-cell.drop-after-last {
    box-shadow: inset -2px 0 0 var(--color-accent);
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
    background: var(--color-table-row-selected);
  }

  .data-row.row-selected .rownum-cell {
    border-left: 2px solid var(--color-accent);
  }

  .data-row.row-selected:hover {
    background: var(--color-table-row-selected);
  }

  /* ── Filler column (absorbs extra width so rownum stays fixed) ──────────── */

  .filler-header-cell {
    width: 100%;
    border-bottom: 1px solid var(--color-border-strong);
    padding: 0;
  }

  .filler-cell {
    width: 100%;
    border-bottom: 1px solid var(--color-border);
    padding: 0;
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
    outline: none;
  }

  .rownum {
    font-size: 11px;
    font-family: var(--font-family-mono);
    color: var(--color-text-muted);
    -webkit-user-select: none;
    user-select: none;
  }

  .row-dirty-dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--color-accent);
  }

  .data-row.row-deleted {
    background: color-mix(in srgb, var(--color-danger) 6%, transparent);
  }

  .data-row.row-deleted .rownum-cell {
    border-left: 2px solid var(--color-danger);
  }

  .data-row.row-deleted:hover {
    background: color-mix(in srgb, var(--color-danger) 10%, transparent);
  }

  .data-row.row-deleted .data-cell {
    text-decoration: line-through;
    opacity: 0.5;
  }

  .row-deleted-indicator {
    font-size: 14px;
    font-weight: bold;
    color: var(--color-danger);
    -webkit-user-select: none;
    user-select: none;
    line-height: 1;
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
    min-width: 0;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .cell-multiline .cell-content {
    display: -webkit-box;
    -webkit-line-clamp: var(--cell-max-lines, 2);
    line-clamp: var(--cell-max-lines, 2);
    -webkit-box-orient: vertical;
    white-space: pre-wrap;
    text-overflow: clip;
  }

  .cell-multiline .data-cell {
    height: auto;
    min-height: 38px;
  }

  .cell-multiline .cell-inner {
    height: auto;
    align-items: flex-start;
    padding: 9px 0;
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
    -webkit-user-select: none;
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

  .empty-overlay {
    position: absolute;
    top: 35px;
    left: 0;
    right: 0;
    bottom: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    pointer-events: none;
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
    -webkit-user-select: none;
    user-select: none;
  }

  .cell-required-empty {
    background: color-mix(in srgb, var(--color-danger, #ef4444) 12%, transparent);
    box-shadow: inset 2px 0 0 var(--color-danger, #ef4444);
  }

  .data-cell.cell-fk:hover {
    cursor: pointer;
  }

  .data-cell.cell-fk:hover .cell-content {
    text-decoration: underline;
    text-underline-offset: 2px;
    text-decoration-style: dotted;
    text-decoration-color: var(--color-text-muted);
  }

  /* ── Search highlighting ────────────────────────────────────────────────── */

  .data-cell.cell-search-match {
    background: color-mix(in srgb, var(--color-accent) 8%, transparent);
  }

  :global(.search-highlight) {
    background: color-mix(in srgb, var(--color-accent) 40%, transparent);
    color: var(--color-text-primary);
    border-radius: 2px;
    padding: 0 1px;
  }

  /* ── Quick view ─────────────────────────────────────────────────────────── */

  .quick-view-row {
    background: transparent;
  }

  .quick-view-cell {
    padding: 0;
    border-bottom: 2px solid var(--color-accent);
    outline: none;
  }

  .quick-view-panel {
    padding: var(--spacing-2) var(--spacing-2) var(--spacing-3);
    background: color-mix(in srgb, var(--color-accent) 6%, var(--color-bg-primary));
    border-top: 1px solid color-mix(in srgb, var(--color-accent) 40%, transparent);
    position: sticky;
    left: 0;
    overflow: hidden;
  }

  .quick-view-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: var(--spacing-2);
    gap: var(--spacing-2);
  }

  .quick-view-title {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-secondary);
    display: flex;
    align-items: center;
    gap: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .quick-view-table-name {
    color: var(--color-accent);
    font-weight: var(--font-weight-semibold);
  }

  .quick-view-sep {
    color: var(--color-text-muted);
    margin: 0 2px;
  }

  .quick-view-filter {
    color: var(--color-text-secondary);
  }

  .quick-view-actions {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    flex-shrink: 0;
  }

  .quick-view-go,
  .quick-view-close {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    padding: 0;
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    font-size: 11px;
    color: var(--color-text-muted);
    cursor: pointer;
    line-height: 1;
    transition:
      background var(--transition-fast),
      color var(--transition-fast),
      border-color var(--transition-fast);
  }

  .quick-view-go:hover,
  .quick-view-close:hover {
    background: var(--color-bg-hover);
    border-color: var(--color-border);
    color: var(--color-text-primary);
  }

  .quick-view-empty {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
  }

  .quick-view-scroll {
    overflow-x: auto;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    scrollbar-color: var(--color-scrollbar-thumb) transparent;
    scrollbar-width: thin;
  }

  .quick-view-table {
    border-collapse: separate;
    border-spacing: 0;
    font-size: 12px;
    white-space: nowrap;
    width: 100%;
  }

  .quick-view-th {
    padding: 5px 12px;
    font-size: 11px;
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-secondary);
    background: var(--color-table-header-bg);
    background-attachment: fixed;
    border-bottom: 1px solid var(--color-border-strong);
    text-align: left;
    white-space: nowrap;
  }

  .quick-view-th-pk {
    color: var(--color-accent);
  }

  .quick-view-pk {
    display: inline;
    vertical-align: middle;
    margin-right: 3px;
    color: var(--color-accent);
    position: relative;
    top: -1px;
  }

  .quick-view-td {
    padding: 6px 12px;
    color: var(--color-text-primary);
    font-size: 12.5px;
    -webkit-user-select: text;
    user-select: text;
  }

  .quick-view-td-number {
    font-family: var(--font-family-mono);
    color: var(--color-editor-number);
  }
</style>
