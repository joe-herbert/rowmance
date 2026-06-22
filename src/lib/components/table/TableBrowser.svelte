<script module>
  // Shared across all TableBrowser instances — persists across remounts.
  type CellValue = string | number | boolean | null;
  const tableScrollPositions = new Map<string, number>();
  const tablePendingState = new Map<string, {
    changes: Map<string, Map<string, CellValue>>;
    originalRows: Map<string, CellValue[]>;
    deletedRows: Map<string, CellValue[]>;
  }>();

  export function clearTablePendingState(key: string): void {
    tablePendingState.delete(key);
    tableScrollPositions.delete(key);
  }
</script>

<script lang="ts">
  import { untrack, tick } from 'svelte';
  import { executeQuery, updateRows, insertRow, deleteRows } from '$lib/tauri/query';
  import type { RowChange, RowDelete } from '$lib/tauri/query';
  import { listColumns, listIndexes, listForeignKeys } from '$lib/tauri/schema';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { useCellSelection } from '$lib/stores/cellSelection.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useVirtualRelations } from '$lib/stores/virtualRelations.svelte';
  import VirtualRelationModal from '$lib/components/relations/VirtualRelationModal.svelte';
  import type { QueryResult, ColumnMeta, ForeignKeyInfo } from '$lib/types';
  import { errorMessage } from '$lib/utils/errors';
  import DataTable, { type PageInfo, type QuickViewData } from '$lib/components/table/DataTable.svelte';
  import ColumnPicker from '$lib/components/table/ColumnPicker.svelte';
  import FilterEditor, {
    type FilterEditorState,
    type FilterRule,
    emptyFilterState,
    filterStateIsActive,
    activeRuleCount,
    buildWhereClause,
    parseSqlConditions,
  } from '$lib/components/table/FilterEditor.svelte';
  import CsvImportModal from '$lib/components/table/CsvImportModal.svelte';
  import SqlImportModal from '$lib/components/table/SqlImportModal.svelte';
  import SqlPreviewModal from '$lib/components/table/SqlPreviewModal.svelte';
  import type { DbType } from '$lib/types';
  import { exportResultToFile, exportResultToClipboard, type ExportFormat } from '$lib/tauri/export';
  import { save as saveDialog } from '@tauri-apps/plugin-dialog';
  import { useStatusBar } from '$lib/stores/statusBar.svelte';
  import RefreshIcon from '$lib/components/icons/RefreshIcon.svelte';
  import { useToast } from '$lib/stores/toast.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import { loadColPrefs, saveColPrefs } from '$lib/utils/table-prefs';

  interface Props {
    connectionId: string;
    database: string;
    table: string;
    initialFilter?: string;
    isFocused?: boolean;
  }

  let { connectionId, database, table, initialFilter, isFocused = false }: Props = $props();

  const connections = useConnections();
  const cellSelectionStore = useCellSelection();
  const panelStore = usePanels();
  const vrStore = useVirtualRelations();
  let connectColumnName = $state<string | null>(null);
  const statusBar = useStatusBar();
  const toast = useToast();
  const settings = useSettings();

  const PAGE_SIZE = $derived(settings.settings.pageSize);

  let page = $state(1);
  let filterEditorState = $state<FilterEditorState>(
    untrack(() =>
      initialFilter?.trim()
        ? { mode: 'sql', groupJunction: 'AND', groups: [], sql: initialFilter }
        : emptyFilterState(),
    ),
  );
  let showFilterEditor = $state(false);
  let filterButtonEl = $state<HTMLButtonElement | null>(null);
  let filterEditorTop = $state(0);
  let filterEditorLeft = $state(0);
  let result = $state<QueryResult | null>(null);
  let foreignKeys = $state<ForeignKeyInfo[]>([]);
  let isLoading = $state(false);
  let error = $state<string | null>(null);

  // ── Pending changes ───────────────────────────────────────────────────────

  const _pendingKey = untrack(() => `${connectionId}:${database}:${table}`);
  const _restoredPending = untrack(() => tablePendingState.get(_pendingKey) ?? null);

  let pendingChanges = $state<Map<string, Map<string, CellValue>>>(
    _restoredPending ? new Map(_restoredPending.changes) : new Map(),
  );
  let originalRows = $state<Map<string, CellValue[]>>(
    _restoredPending ? new Map(_restoredPending.originalRows) : new Map(),
  );
  // rowKey → original row snapshot, used to build DELETE WHERE clauses on save
  let pendingDeletedRows = $state<Map<string, CellValue[]>>(
    _restoredPending ? new Map(_restoredPending.deletedRows) : new Map(),
  );
  let isSaving = $state(false);
  let saveError = $state<string | null>(null);
  let showSqlPreview = $state(false);

  function quoteIdent(name: string, dbType: DbType): string {
    if (dbType === 'postgres') return `"${name.replace(/"/g, '""')}"`;
    if (dbType === 'sqlite') return `"${name.replace(/"/g, '""')}"`;
    return `\`${name.replace(/`/g, '``')}\``;
  }

  function formatSqlValue(val: unknown, dbType: DbType): string {
    if (val === null || val === undefined) return 'NULL';
    if (typeof val === 'boolean') {
      if (dbType === 'postgres') return val ? 'TRUE' : 'FALSE';
      return val ? '1' : '0';
    }
    if (typeof val === 'number') return String(val);
    return `'${String(val).replace(/'/g, "''")}'`;
  }

  function buildPreviewStatements(): string[] {
    if (!result) return [];
    const connection = connections.getById(connectionId);
    const dbType: DbType = connection?.dbType ?? 'mysql';
    const q = (name: string) => quoteIdent(name, dbType);
    const v = (val: unknown) => formatSqlValue(val, dbType);

    const useSchema = dbType !== 'sqlite';
    const tableRef = useSchema ? `${q(database)}.${q(table)}` : q(table);
    const pkColumns = result.columns.filter((c) => c.isPrimaryKey).map((c) => c.name);
    const hasPk = pkColumns.length > 0;

    const stmts: string[] = [];

    for (const [rowKey, colMap] of pendingChanges) {
      if (rowKey.startsWith('__new__')) {
        const vals: Record<string, unknown> = Object.fromEntries(colMap);
        for (const col of result.columns) {
          if (col.name in vals) continue;
          if (col.isAutoIncrement) continue;
          if (col.nullable && col.defaultValue == null) vals[col.name] = null;
        }
        if (Object.keys(vals).length === 0) continue;
        const cols = Object.keys(vals).map(q).join(', ');
        const values = Object.values(vals).map(v).join(', ');
        stmts.push(`INSERT INTO ${tableRef} (${cols}) VALUES (${values});`);
        continue;
      }

      if (pendingDeletedRows.has(rowKey)) continue;

      const origRow = originalRows.get(rowKey);
      if (!origRow) continue;

      const primaryKeys: Record<string, unknown> = {};
      if (hasPk) {
        pkColumns.forEach((pkCol) => {
          const idx = result!.columns.findIndex((c) => c.name === pkCol);
          primaryKeys[pkCol] = idx >= 0 ? (origRow[idx] ?? null) : null;
        });
      } else {
        result.columns.forEach((col, i) => { primaryKeys[col.name] = origRow[i] ?? null; });
      }

      const setClauses = [...colMap.entries()].map(([col, val]) => `${q(col)} = ${v(val)}`).join(', ');
      const whereClauses = Object.entries(primaryKeys)
        .map(([col, val]) => val === null ? `${q(col)} IS NULL` : `${q(col)} = ${v(val)}`)
        .join(' AND ');
      const limit = (dbType === 'mysql' || dbType === 'mariadb') ? ' LIMIT 1' : '';
      stmts.push(`UPDATE ${tableRef} SET ${setClauses} WHERE ${whereClauses}${limit};`);
    }

    for (const [rowKey, origRow] of pendingDeletedRows) {
      if (rowKey.startsWith('__new__')) continue;
      const primaryKeys: Record<string, unknown> = {};
      if (hasPk) {
        pkColumns.forEach((pkCol) => {
          const idx = result!.columns.findIndex((c) => c.name === pkCol);
          primaryKeys[pkCol] = idx >= 0 ? (origRow[idx] ?? null) : null;
        });
      } else {
        result.columns.forEach((col, i) => { primaryKeys[col.name] = origRow[i] ?? null; });
      }
      const whereClauses = Object.entries(primaryKeys)
        .map(([col, val]) => val === null ? `${q(col)} IS NULL` : `${q(col)} = ${v(val)}`)
        .join(' AND ');
      const limit = (dbType === 'mysql' || dbType === 'mariadb') ? ' LIMIT 1' : '';
      stmts.push(`DELETE FROM ${tableRef} WHERE ${whereClauses}${limit};`);
    }

    return stmts;
  }

  // ── No-PK warning banner ──────────────────────────────────────────────────

  const NO_PK_WARN_KEY = 'rowmance_no_pk_warn_dismissed';
  let noPkWarnDismissed = $state(localStorage.getItem(NO_PK_WARN_KEY) === 'true');
  let connectionReadOnly = $derived(connections.getById(connectionId)?.readOnly ?? false);

  const tableHasNoPk = $derived(
    result !== null &&
    result.columns.length > 0 &&
    result.columns.every((c) => !c.isPrimaryKey),
  );

  const showNoPkWarning = $derived(tableHasNoPk && !connectionReadOnly && !noPkWarnDismissed);

  function dismissNoPkWarning(): void {
    noPkWarnDismissed = true;
  }

  function dismissNoPkWarningForever(): void {
    localStorage.setItem(NO_PK_WARN_KEY, 'true');
    noPkWarnDismissed = true;
  }

  // Key to force DataTable to fully reset (clears pending changes displayed)
  let tableKey = $state(0);

  const pendingCount = $derived(pendingChanges.size + pendingDeletedRows.size);

  function handleChangePending(changes: Map<string, Map<string, CellValue>>, rows: Map<string, CellValue[]>): void {
    // Deep-copy to plain Maps so TableBrowser owns its data independently of DataTable's reactive proxies.
    pendingChanges = new Map([...changes].map(([k, v]) => [k, new Map(v)]));
    originalRows = new Map([...rows].map(([k, v]) => [k, [...v]]));
  }

  function handleDeleteRowsPending(deletedRows: Map<string, CellValue[]>): void {
    pendingDeletedRows = new Map([...deletedRows].map(([k, v]) => [k, [...v]]));
  }

  function discardChanges(): void {
    tablePendingState.delete(_pendingKey);
    pendingChanges = new Map();
    originalRows = new Map();
    pendingDeletedRows = new Map();
    addRowTrigger = 0;
    tableKey++;
  }

  async function saveChanges(): Promise<void> {
    if (!result) return;
    isSaving = true;
    saveError = null;

    try {
      const pkColumns = result.columns.filter((c) => c.isPrimaryKey).map((c) => c.name);
      const hasPk = pkColumns.length > 0;

      const rowChanges: RowChange[] = [];
      const insertValues: Record<string, unknown>[] = [];

      for (const [rowKey, colMap] of pendingChanges) {
        if (rowKey.startsWith('__new__')) {
          const vals: Record<string, unknown> = Object.fromEntries(colMap);
          for (const col of result.columns) {
            if (col.name in vals) continue;
            if (col.isAutoIncrement) continue;
            if (col.nullable && col.defaultValue == null) vals[col.name] = null;
          }
          if (Object.keys(vals).length > 0) insertValues.push(vals);
          continue;
        }

        // Skip updates for rows that are also pending deletion (delete wins)
        if (pendingDeletedRows.has(rowKey)) continue;

        const primaryKeys: Record<string, unknown> = {};

        if (hasPk) {
          const origRow = originalRows.get(rowKey);
          if (!origRow) continue;
          pkColumns.forEach((pkCol) => {
            const idx = result!.columns.findIndex((c) => c.name === pkCol);
            primaryKeys[pkCol] = idx >= 0 ? (origRow[idx] ?? null) : null;
          });
        } else {
          // No PK: identify the row by all its original column values.
          // The backend will use IS NULL for null values and = ? for non-null,
          // with LIMIT 1 (MySQL) as a safety guard against duplicate rows.
          const origRow = originalRows.get(rowKey);
          if (!origRow) continue;
          result.columns.forEach((col, i) => {
            primaryKeys[col.name] = origRow[i] ?? null;
          });
        }

        const changes: Record<string, unknown> = {};
        for (const [col, val] of colMap) {
          changes[col] = val;
        }

        rowChanges.push({ primaryKeys, changes });
      }

      const deleteChanges: RowDelete[] = [];
      for (const [rowKey, origRow] of pendingDeletedRows) {
        if (rowKey.startsWith('__new__')) continue;
        const primaryKeys: Record<string, unknown> = {};
        if (hasPk) {
          pkColumns.forEach((pkCol) => {
            const idx = result!.columns.findIndex((c) => c.name === pkCol);
            primaryKeys[pkCol] = idx >= 0 ? (origRow[idx] ?? null) : null;
          });
        } else {
          result.columns.forEach((col, i) => {
            primaryKeys[col.name] = origRow[i] ?? null;
          });
        }
        deleteChanges.push({ primaryKeys });
      }

      await updateRows(connectionId, database, table, rowChanges);
      for (const values of insertValues) {
        await insertRow(connectionId, database, table, values);
      }
      if (deleteChanges.length > 0) {
        await deleteRows(connectionId, database, table, deleteChanges);
      }
      tablePendingState.delete(_pendingKey);
      pendingChanges = new Map();
      pendingDeletedRows = new Map();
      addRowTrigger = 0;
      tableKey++;
      await load();
      toast.addToast('Changes saved', 'success', 2000);
    } catch (err) {
      saveError = errorMessage(err);
    } finally {
      isSaving = false;
    }
  }

  // ── Column visibility ─────────────────────────────────────────────────────

  let hiddenColumns = $state<Set<string>>(new Set());
  let initialColWidths = $state<Record<string, number> | undefined>(undefined);
  let initialColumnOrder = $state<string[] | undefined>(undefined);

  // Load column prefs from DB whenever connection/db/table change.
  $effect(() => {
    const conn = connectionId, db = database, tbl = table;
    hiddenColumns = new Set();
    initialColWidths = undefined;
    initialColumnOrder = undefined;
    loadColPrefs(conn, db, tbl).then((prefs) => {
      hiddenColumns = new Set(prefs?.hiddenColumns ?? []);
      initialColWidths = prefs?.colWidths;
      initialColumnOrder = prefs?.columnOrder?.length ? prefs.columnOrder : undefined;
    });
  });

  let showColumnPicker = $state(false);
  let columnPickerAnchorEl = $state<HTMLButtonElement | null>(null);

  function toggleColumn(name: string): void {
    const next = new Set(hiddenColumns);
    if (next.has(name)) next.delete(name);
    else next.add(name);
    hiddenColumns = next;
    saveColPrefs(connectionId, database, table, { hiddenColumns: [...next] });
  }

  // ── DB type + SQL helpers ─────────────────────────────────────────────────

  let dbType = $derived(connections.getById(connectionId)?.dbType ?? 'mysql');
  let connectionColor = $derived(connections.getById(connectionId)?.color ?? null);

  function quoteIdentifier(name: string): string {
    return dbType === 'postgres' ? `"${name}"` : `\`${name}\``;
  }

  function buildSql(): string {
    const quotedDb = quoteIdentifier(database);
    const quotedTable = quoteIdentifier(table);
    let base = `SELECT * FROM ${quotedDb}.${quotedTable}`;
    const where = buildWhereClause(filterEditorState, quoteIdentifier);
    if (where) {
      base += ` WHERE ${where}`;
    }
    return base;
  }

  function isRuleActive(rule: FilterRule): boolean {
    if (rule.rawSql !== undefined) return rule.rawSql.trim() !== '';
    return rule.column !== '' && (rule.operator === 'IS NULL' || rule.operator === 'IS NOT NULL' || rule.value.trim() !== '');
  }

  function formatRuleText(rule: FilterRule): string {
    if (rule.rawSql !== undefined) return rule.rawSql;
    if (rule.operator === 'IS NULL' || rule.operator === 'IS NOT NULL') return `${rule.column} ${rule.operator}`;
    if (rule.operator === 'IN') return `${rule.column} IN (${rule.value})`;
    return `${rule.column} ${rule.operator} ${rule.value}`;
  }

  function tryStripOuterParens(s: string): string | null {
    const t = s.trim();
    if (!t.startsWith('(') || !t.endsWith(')')) return null;
    let depth = 0;
    for (let i = 0; i < t.length - 1; i++) {
      if (t[i] === '(') depth++;
      else if (t[i] === ')') depth--;
      if (depth === 0) return null;
    }
    return t.slice(1, -1).trim();
  }

  type SummaryGroup = { keyword: string; bordered: boolean; rules: { conjunction: string; text: string; first: boolean }[] };

  const filterSummaryBlocks = $derived.by((): SummaryGroup[] => {
    if (filterEditorState.mode === 'sql') {
      return parseSqlConditions(filterEditorState.sql).map((part) => {
        const inner = tryStripOuterParens(part.condition);
        if (inner) {
          const innerParts = parseSqlConditions(inner);
          if (innerParts.length > 1) {
            return {
              keyword: part.keyword,
              bordered: true,
              rules: innerParts.map((ip, ri) => ({ conjunction: ip.keyword, text: ip.condition, first: ri === 0 })),
            };
          }
        }
        return { keyword: part.keyword, bordered: false, rules: [{ conjunction: '', text: part.condition, first: true }] };
      });
    }
    const activeGroups = filterEditorState.groups
      .map((g) => ({ ...g, active: g.rules.filter(isRuleActive) }))
      .filter((g) => g.active.length > 0);
    const multi = activeGroups.length > 1;
    return activeGroups.map((g, gi) => ({
      keyword: gi === 0 ? 'WHERE' : filterEditorState.groupJunction,
      bordered: multi && g.active.length > 1,
      rules: g.active.map((r, ri) => ({ conjunction: g.conjunction, text: formatRuleText(r), first: ri === 0 })),
    }));
  });

  function openFilterEditor(): void {
    if (filterButtonEl) {
      const rect = filterButtonEl.getBoundingClientRect();
      filterEditorTop = rect.bottom + 4;
      filterEditorLeft = Math.max(4, rect.right - 360);
    }
    showFilterEditor = true;
  }

  function portal(node: HTMLElement): { destroy(): void } {
    document.body.appendChild(node);
    return { destroy() { node.remove(); } };
  }

  let lastQueryMs = $state<number | null>(null);

  // ── Dirty state tracking ──────────────────────────────────────────────────

  const _dirtyKey = _pendingKey;

  $effect(() => {
    panelStore.setItemDirty(_dirtyKey, pendingCount > 0);
    return () => {
      // Only clear dirty if there are no saved pending changes (e.g. after save/discard).
      // When switching tabs, tablePendingState still holds the data so the indicator should stay.
      if (!tablePendingState.has(_pendingKey)) {
        panelStore.setItemDirty(_dirtyKey, false);
      }
    };
  });

  // Eagerly mirror pending state into the module-level map so it's available
  // immediately when a new instance mounts (cleanup timing is not reliable).
  $effect(() => {
    if (pendingChanges.size > 0 || pendingDeletedRows.size > 0) {
      tablePendingState.set(_pendingKey, {
        changes: new Map([...pendingChanges].map(([k, v]) => [k, new Map(v)])),
        originalRows: new Map([...originalRows].map(([k, v]) => [k, [...v]])),
        deletedRows: new Map([...pendingDeletedRows].map(([k, v]) => [k, [...v]])),
      });
    } else {
      tablePendingState.delete(_pendingKey);
    }
  });

  // ── Scroll position persistence ───────────────────────────────────────────

  // Restore scroll position after first data load
  let _scrollRestored = false;
  $effect(() => {
    if (result !== null && !_scrollRestored) {
      _scrollRestored = true;
      const saved = tableScrollPositions.get(_dirtyKey);
      if (saved) {
        tick().then(() => {
          const scrollEl = tableBrowserEl?.querySelector('.table-scroll') as HTMLElement | null;
          if (scrollEl) scrollEl.scrollTop = saved;
        });
      }
    }
  });

  // Save scroll position on unmount
  $effect(() => {
    return () => {
      const scrollEl = tableBrowserEl?.querySelector('.table-scroll') as HTMLElement | null;
      if (scrollEl && scrollEl.scrollTop > 0) {
        tableScrollPositions.set(_dirtyKey, scrollEl.scrollTop);
      } else if (scrollEl && scrollEl.scrollTop === 0) {
        tableScrollPositions.delete(_dirtyKey);
      }
    };
  });

  async function load(): Promise<void> {
    isLoading = true;
    error = null;
    const t0 = performance.now();
    try {
      const [queryResult, schemaColumns, indexes, fks] = await Promise.all([
        executeQuery(connectionId, buildSql(), untrack(() => page), PAGE_SIZE),
        listColumns(connectionId, database, table).catch(() => []),
        listIndexes(connectionId, database, table).catch(() => []),
        listForeignKeys(connectionId, database, table).catch(() => []),
      ]);
      foreignKeys = fks;

      if (queryResult.error) {
        error = queryResult.error;
        result = null;
      } else {
        if (schemaColumns.length > 0) {
          const schemaMap = new Map(schemaColumns.map((c) => [c.name, c]));
          const uniqueColNames = new Set(
            indexes.filter((idx) => idx.unique).flatMap((idx) => idx.columns),
          );
          queryResult.columns = queryResult.columns.map((col) => {
            const s = schemaMap.get(col.name);
            return {
              ...col,
              dataType: s?.dataType ?? col.dataType,
              nullable: s ? s.nullable : col.nullable,
              isPrimaryKey: s?.isPrimaryKey ?? false,
              isForeignKey: (s?.isForeignKey ?? false) || vrStore.hasAny(connectionId, database, table, col.name),
              defaultValue: s?.defaultValue ?? null,
              isAutoIncrement: s?.isAutoIncrement ?? false,
              isUnique: uniqueColNames.has(col.name),
            };
          });
        }
        result = queryResult;
        lastQueryMs = Math.round(performance.now() - t0);
      }
    } catch (err) {
      error = errorMessage(err);
      result = null;
    } finally {
      isLoading = false;
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
    const _filter = initialFilter;

    page = 1;
    filterEditorState = initialFilter?.trim()
      ? { mode: 'sql', groupJunction: 'AND', groups: [], sql: initialFilter }
      : emptyFilterState();
    showFilterEditor = false;
    showColumnPicker = false;
    noPkWarnDismissed = localStorage.getItem(NO_PK_WARN_KEY) === 'true';
    untrack(() => { load(); });
  });

  // Column picker position
  let pickerTop = $state(0);
  let pickerLeft = $state(0);

  function openColumnPicker(): void {
    if (columnPickerAnchorEl) {
      const rect = columnPickerAnchorEl.getBoundingClientRect();
      pickerTop = rect.bottom + 4;
      pickerLeft = rect.right - 220;
    }
    showColumnPicker = true;
  }

  const currentColumns = $derived<ColumnMeta[]>(result?.columns ?? []);

  // ── Export state ───────────────────────────────────────────────────────────

  let showExportMenu = $state(false);
  let exportMenuTop = $state(0);
  let exportMenuLeft = $state(0);
  let exportTableName = $state('');
  let showTableNameInput = $state(false);
  let pendingExportFormat = $state<ExportFormat | null>(null);
  let pendingExportToFile = $state(false);
  let exportError = $state<string | null>(null);

  // ── Import modal state ─────────────────────────────────────────────────────

  let showImportMenu = $state(false);
  let importMenuTop = $state(0);
  let importMenuLeft = $state(0);
  let showCsvImport = $state(false);
  let showSqlImport = $state(false);
  let importSource = $state<'file' | 'clipboard'>('file');
  let addRowTrigger = $state(0);

  const EXPORT_FORMATS: { label: string; format: ExportFormat; needsTableName: boolean }[] = [
    { label: 'CSV', format: 'csv', needsTableName: false },
    { label: 'JSON', format: 'json', needsTableName: false },
    { label: 'Tab-separated', format: 'tab_separated', needsTableName: false },
    { label: 'SQL INSERT', format: 'sql_insert', needsTableName: true },
    { label: 'SQL IN Clause', format: 'sql_in_clause', needsTableName: false },
  ];

  const FORMAT_EXTENSIONS: Record<ExportFormat, string> = {
    csv: 'csv',
    json: 'json',
    tab_separated: 'tsv',
    sql_insert: 'sql',
    sql_in_clause: 'sql',
  };

  function getExportColumns(): string[] {
    return result?.columns.map((c) => c.name) ?? [];
  }

  function getExportRows(): unknown[][] {
    return result?.rows ?? [];
  }

  function startExport(format: ExportFormat, toFile: boolean): void {
    showExportMenu = false;
    const def = EXPORT_FORMATS.find((f) => f.format === format);
    if (def?.needsTableName) {
      pendingExportFormat = format;
      pendingExportToFile = toFile;
      showTableNameInput = true;
      return;
    }
    doExport(format, toFile, undefined);
  }

  async function doExport(format: ExportFormat, toFile: boolean, tblName: string | undefined): Promise<void> {
    exportError = null;
    try {
      if (toFile) {
        const ext = FORMAT_EXTENSIONS[format];
        const filePath = await saveDialog({
          defaultPath: `${table}.${ext}`,
          filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
        });
        if (!filePath) return;
        await exportResultToFile(getExportRows(), getExportColumns(), format, filePath, tblName);
      } else {
        await exportResultToClipboard(getExportRows(), getExportColumns(), format, tblName);
      }
    } catch (err) {
      exportError = errorMessage(err);
    }
  }

  async function confirmTableNameExport(): Promise<void> {
    if (!pendingExportFormat) return;
    const format = pendingExportFormat;
    const toFile = pendingExportToFile;
    showTableNameInput = false;
    pendingExportFormat = null;
    await doExport(format, toFile, exportTableName || undefined);
    exportTableName = '';
  }

  function cancelTableNameExport(): void {
    showTableNameInput = false;
    pendingExportFormat = null;
    exportTableName = '';
  }

  function handleExportKeydown(e: KeyboardEvent): void {
    if (e.key === 'Enter') confirmTableNameExport();
    else if (e.key === 'Escape') cancelTableNameExport();
  }

  // ── Cell selection (feeds the Relations panel) ────────────────────────────

  function handleCellSelect(colIndex: number, row: CellValue[]): void {
    if (!result) return;
    const col = result.columns[colIndex];
    if (!col) return;
    cellSelectionStore.set({
      connectionId,
      database,
      table,
      columnName: col.name,
      cellValue: row[colIndex],
      row,
      columns: result.columns,
    });
  }

  function handleForeignKeyClick(colName: string, value: CellValue): void {
    const fk = foreignKeys.find((f) => f.columns.includes(colName));
    if (fk) {
      const colIdx = fk.columns.indexOf(colName);
      const refCol = fk.referencedColumns[colIdx];
      if (!refCol) return;
      const quotedCol = quoteIdentifier(refCol);
      let filter: string;
      if (typeof value === 'number' || typeof value === 'boolean') {
        filter = `${quotedCol} = ${value}`;
      } else {
        const escaped = String(value).replace(/'/g, "''");
        filter = `${quotedCol} = '${escaped}'`;
      }
      panelStore.openInFocused({
        kind: 'table_browser',
        connectionId,
        database,
        table: fk.referencedTable,
        initialFilter: filter,
      });
      return;
    }

    // Check virtual relations
    const vr = vrStore.forwardFrom({ connectionId, database, table, column: colName })[0];
    if (!vr) return;
    const targetConnId = vr.to.connectionId;
    const targetDb = vr.to.database;
    const targetDbType = connections.getById(targetConnId)?.dbType ?? 'mysql';
    const quotedCol = targetDbType === 'postgres' ? `"${vr.to.column}"` : `\`${vr.to.column}\``;
    let filter: string;
    if (typeof value === 'number' || typeof value === 'boolean') {
      filter = `${quotedCol} = ${value}`;
    } else {
      const escaped = String(value).replace(/'/g, "''");
      filter = `${quotedCol} = '${escaped}'`;
    }
    panelStore.openInFocused({
      kind: 'table_browser',
      connectionId: targetConnId,
      database: targetDb,
      table: vr.to.table,
      initialFilter: filter,
    });
  }

  async function handleForeignKeyQuickView(colName: string, value: CellValue): Promise<QuickViewData | null> {
    const fk = foreignKeys.find((f) => f.columns.includes(colName));
    if (!fk) return null;
    const colIdx = fk.columns.indexOf(colName);
    const refCol = fk.referencedColumns[colIdx];
    if (!refCol) return null;
    const quotedDb = quoteIdentifier(database);
    const quotedTable = quoteIdentifier(fk.referencedTable);
    const quotedCol = quoteIdentifier(refCol);
    let whereVal: string;
    if (typeof value === 'number' || typeof value === 'boolean') {
      whereVal = String(value);
    } else {
      const escaped = String(value).replace(/'/g, "''");
      whereVal = `'${escaped}'`;
    }
    const sql = `SELECT * FROM ${quotedDb}.${quotedTable} WHERE ${quotedCol} = ${whereVal}`;
    try {
      const queryResult = await executeQuery(connectionId, sql, 1, 1);
      if (queryResult.error) return null;
      return {
        tableName: fk.referencedTable,
        refColumn: refCol,
        refValue: value,
        columns: queryResult.columns,
        row: queryResult.rows[0] ?? null,
      };
    } catch {
      return null;
    }
  }

  // ── DataTable pagination state ─────────────────────────────────────────────

  let dtPageIndex = $state(0);
  let dtPageInfo = $state<PageInfo | null>(null);

  function handleDtPageInfo(info: PageInfo): void {
    dtPageInfo = info;
    dtPageIndex = info.pageIndex;
  }

  // ── Table browser ref (for focus detection) ────────────────────────────────

  let tableBrowserEl = $state<HTMLElement | null>(null);

  // ── Shortcut event handling ────────────────────────────────────────────────

  function handleShortcutAction(e: Event): void {
    const customEvent = e as CustomEvent<{ action: string }>;
    const action = customEvent.detail?.action;

    // Only handle when this panel is focused/visible
    if (!tableBrowserEl || !document.contains(tableBrowserEl)) return;
    // Check if any part of this component is active (contains focus)
    const hasFocus = tableBrowserEl.contains(document.activeElement) ||
      document.activeElement === document.body;
    if (!hasFocus) return;

    switch (action) {
      case 'TABLE_SAVE_CHANGES':
        if (pendingCount > 0) saveChanges();
        break;
      case 'TABLE_DISCARD_CHANGES':
        if (pendingCount > 0) discardChanges();
        break;
      case 'PAGE_NEXT':
        nextTablePage();
        break;
      case 'PAGE_PREV':
        prevTablePage();
        break;
    }
  }

  function nextTablePage(): void {
    if (dtPageInfo && page < dtPageInfo.pageCount) {
      page++;
      load();
    }
  }

  function prevTablePage(): void {
    if (page > 1) {
      page--;
      load();
    }
  }

  $effect(() => {
    window.addEventListener('shortcut-action', handleShortcutAction);
    return () => window.removeEventListener('shortcut-action', handleShortcutAction);
  });

  // Register this panel's state with the status bar when focused.
  $effect(() => {
    if (!isFocused) return;
    const rowCount = dtPageInfo ? dtPageInfo.processedRowsLength : null;
    statusBar.update({
      pendingCount,
      rowCount,
      lastQueryMs,
      isSaving,
      onSave: pendingCount > 0 ? saveChanges : null,
      onDiscard: pendingCount > 0 ? discardChanges : null,
    });
  });

  $effect(() => {
    return () => {
      if (isFocused) statusBar.clear();
    };
  });
</script>

<div class="table-browser" bind:this={tableBrowserEl}>
  <div class="toolbar" style={connectionColor ? `border-bottom: 2px solid ${connectionColor};` : undefined}>
    <span
      class="table-name"
      title={`Click to copy ${database}.${table}`}
      role="button"
      tabindex="0"
      onclick={() => navigator.clipboard.writeText(`${database}.${table}`).then(() => toast.addToast(`Copied ${database}.${table} to clipboard`, 'success'))}
      onkeydown={(e) => e.key === 'Enter' && navigator.clipboard.writeText(`${database}.${table}`).then(() => toast.addToast(`Copied ${database}.${table} to clipboard`, 'success'))}
    >
      <span class="db-name">{database}</span>
      <span class="separator">.</span>
      <span class="tbl-name">{table}</span>
    </span>

    <button
      bind:this={filterButtonEl}
      class="refresh-button"
      class:refresh-button--labeled={filterStateIsActive(filterEditorState)}
      class:filter-active={filterStateIsActive(filterEditorState)}
      onclick={openFilterEditor}
      title="Filters"
      aria-label="Filters"
      aria-expanded={showFilterEditor}
    >
      <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3"/></svg>
      {#if filterStateIsActive(filterEditorState)}
        <span class="badge">
          {activeRuleCount(filterEditorState)}
        </span>
      {/if}
    </button>

    <div class="toolbar-right">

    {#if dtPageInfo !== null}
      <span class="row-range">
        {#if dtPageInfo.pageRowsLength === 0}
          0
        {:else}
          {@const pageOffset = (page - 1) * PAGE_SIZE}
          {(pageOffset + 1).toLocaleString()}–{Math.min(pageOffset + dtPageInfo.pageRowsLength, dtPageInfo.processedRowsLength).toLocaleString()}
        {/if}
        of {dtPageInfo.processedRowsLength.toLocaleString()}
      </span>

      <div class="page-nav-group">
        <button
          class="page-nav-btn"
          onclick={prevTablePage}
          disabled={page === 1}
          aria-label="Previous page"
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="15 18 9 12 15 6"></polyline>
          </svg>
        </button>
        <button
          class="page-nav-btn page-nav-btn--next"
          onclick={nextTablePage}
          disabled={page >= dtPageInfo.pageCount}
          aria-label="Next page"
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="9 18 15 12 9 6"></polyline>
          </svg>
        </button>
      </div>
    {/if}

    {#if currentColumns.length > 0}
      <button
        bind:this={columnPickerAnchorEl}
        class="refresh-button"
        class:refresh-button--labeled={hiddenColumns.size > 0}
        onclick={openColumnPicker}
        title="Show/hide columns"
        aria-label="Column visibility"
        aria-expanded={showColumnPicker}
      >
        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><line x1="9" y1="3" x2="9" y2="21"/><line x1="15" y1="3" x2="15" y2="21"/></svg>
        {#if hiddenColumns.size > 0}
          <span class="badge">{hiddenColumns.size}</span>
        {/if}
      </button>
    {/if}

    {#if pendingCount > 0}
      <div class="save-split-btn">
        <button
          class="toolbar-btn save-btn save-split-main"
          onclick={saveChanges}
          disabled={isSaving}
          title="Save pending changes to the database"
          aria-label="Save {pendingCount} pending changes"
        >
          {isSaving ? 'Saving…' : `Save ${pendingCount} change${pendingCount !== 1 ? 's' : ''}`}
        </button>
        <button
          class="toolbar-btn save-btn save-split-arrow"
          onclick={() => { showSqlPreview = true; }}
          disabled={isSaving}
          title="Preview SQL"
          aria-label="Preview SQL"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>
        </button>
      </div>

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

    <!-- Insert row button -->
    {#if result !== null && !connectionReadOnly}
      <button
        class="refresh-button"
        onclick={() => addRowTrigger++}
        title="Insert new row"
        aria-label="Insert new row"
      >
        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
      </button>
    {/if}

    <!-- Export dropdown -->
    {#if result !== null}
      <div class="export-dropdown">
        <button
          class="refresh-button"
          onclick={(e) => { if (!showExportMenu) { const r = (e.currentTarget as HTMLElement).getBoundingClientRect(); exportMenuTop = r.bottom + 4; exportMenuLeft = r.right - 210; } showExportMenu = !showExportMenu; exportError = null; }}
          aria-expanded={showExportMenu}
          aria-label="Export table data"
          title="Export"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>        </button>

        {#if showExportMenu}
          <div use:portal class="export-positioner" style="top: {exportMenuTop}px; left: {exportMenuLeft}px;">
            <div class="export-menu" role="menu">
              <div class="export-menu-title">Export</div>
              {#each EXPORT_FORMATS as fmt}
                <div class="export-menu-section">
                  <span class="export-format-label">{fmt.label}</span>
                  <button
                    class="export-menu-item"
                    role="menuitem"
                    onclick={() => startExport(fmt.format, false)}
                  >
                    Clipboard
                  </button>
                  <button
                    class="export-menu-item"
                    role="menuitem"
                    onclick={() => startExport(fmt.format, true)}
                  >
                    File
                  </button>
                </div>
              {/each}
            </div>
            <div
              class="export-backdrop"
              role="presentation"
              onclick={() => (showExportMenu = false)}
              onkeydown={(e) => { if (e.key === 'Escape') showExportMenu = false; }}
            ></div>
          </div>
        {/if}
      </div>
    {/if}

    <!-- Import dropdown -->
    <div class="export-dropdown">
      <button
        class="refresh-button"
        onclick={(e) => { if (!showImportMenu) { const r = (e.currentTarget as HTMLElement).getBoundingClientRect(); importMenuTop = r.bottom + 4; importMenuLeft = r.right - 210; } showImportMenu = !showImportMenu; }}
        aria-expanded={showImportMenu}
        aria-label="Import data"
        title="Import"
      >
        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
      </button>

      {#if showImportMenu}
        <div use:portal class="export-positioner" style="top: {importMenuTop}px; left: {importMenuLeft}px;">
          <div class="export-menu" role="menu">
            <div class="export-menu-title">Import</div>
            <div class="export-menu-section">
              <span class="export-format-label">CSV</span>
              <button
                class="export-menu-item"
                role="menuitem"
                onclick={() => { showImportMenu = false; importSource = 'clipboard'; showCsvImport = true; }}
              >
                Clipboard
              </button>
              <button
                class="export-menu-item"
                role="menuitem"
                onclick={() => { showImportMenu = false; importSource = 'file'; showCsvImport = true; }}
              >
                File
              </button>
            </div>
            <div class="export-menu-section">
              <span class="export-format-label">SQL</span>
              <button
                class="export-menu-item"
                role="menuitem"
                onclick={() => { showImportMenu = false; importSource = 'clipboard'; showSqlImport = true; }}
              >
                Clipboard
              </button>
              <button
                class="export-menu-item"
                role="menuitem"
                onclick={() => { showImportMenu = false; importSource = 'file'; showSqlImport = true; }}
              >
                File
              </button>
            </div>
          </div>
          <div
            class="export-backdrop"
            role="presentation"
            onclick={() => (showImportMenu = false)}
            onkeydown={(e) => { if (e.key === 'Escape') showImportMenu = false; }}
          ></div>
        </div>
      {/if}
    </div>

    <button
      class="refresh-button"
      onclick={handleRefresh}
      disabled={isLoading}
      title="Refresh"
      aria-label="Refresh table data"
    >
      <RefreshIcon />
    </button>
    </div>
  </div>

  {#if filterStateIsActive(filterEditorState)}
    <button
      class="filter-summary-bar"
      onclick={openFilterEditor}
      title="Click to edit filters"
      aria-label="Edit active filters"
    >
      {#each filterSummaryBlocks as grp}
        <span class="fsb-group" class:fsb-group--bordered={grp.bordered}>
          {#each grp.rules as rule}
            <span class="fsb-line">
              <span class="fsb-kw">{rule.first ? grp.keyword : rule.conjunction}</span>
              <span class="fsb-text">{rule.text}</span>
            </span>
          {/each}
        </span>
      {/each}
    </button>
  {/if}

  {#if showTableNameInput}
    <div class="table-name-export-bar">
      <label class="table-name-label" for="tb-export-table-name">Table name for SQL INSERT:</label>
      <input
        id="tb-export-table-name"
        class="table-name-input"
        type="text"
        placeholder="table_name"
        bind:value={exportTableName}
        onkeydown={handleExportKeydown}
        aria-label="Table name for SQL INSERT export"
      />
      <button class="toolbar-btn save-btn" onclick={confirmTableNameExport}>Export</button>
      <button class="toolbar-btn" onclick={cancelTableNameExport}>Cancel</button>
    </div>
  {/if}

  {#if exportError !== null}
    <div class="save-error-bar" role="alert">
      <span class="save-error-label">Export failed:</span>
      <span class="save-error-message">{exportError}</span>
      <button class="save-error-copy" onclick={() => navigator.clipboard.writeText(exportError!).then(() => toast.addToast('Copied', 'success', 1500))} aria-label="Copy error message" title="Copy">
        <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
      </button>
      <button class="save-error-close" onclick={() => (exportError = null)} aria-label="Dismiss">✕</button>
    </div>
  {/if}

  {#if saveError !== null}
    <div class="save-error-bar" role="alert">
      <span class="save-error-label">Save failed:</span>
      <span class="save-error-message">{saveError}</span>
      <button class="save-error-copy" onclick={() => navigator.clipboard.writeText(saveError!).then(() => toast.addToast('Copied', 'success', 1500))} aria-label="Copy error message" title="Copy">
        <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
      </button>
      <button class="save-error-close" onclick={() => (saveError = null)} aria-label="Dismiss">✕</button>
    </div>
  {/if}

  {#if showNoPkWarning}
    <div class="no-pk-warning-bar" role="alert">
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
        <line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/>
      </svg>
      <span class="no-pk-warning-text">
        This table has no primary key. Edits are matched by all column values — avoid editing tables with duplicate rows.
      </span>
      <button class="no-pk-warning-btn" onclick={dismissNoPkWarning}>OK</button>
      <button class="no-pk-warning-btn" onclick={dismissNoPkWarningForever}>Don't show again</button>
    </div>
  {/if}

  <div class="content">
    {#if isLoading}
      <div class="loading">
        <span class="loading-text">Loading…</span>
      </div>
    {:else if error !== null}
      <div class="error-box" role="alert">
        <div class="error-header">
          <span class="error-label">Error</span>
          <button class="error-copy" onclick={() => navigator.clipboard.writeText(error!).then(() => toast.addToast('Copied', 'success', 1500))} aria-label="Copy error message" title="Copy error">
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
          </button>
        </div>
        <span class="error-message">{error}</span>
      </div>
    {:else if result !== null}
      {#key tableKey}
        <DataTable
          columns={result.columns}
          rows={result.rows}
          totalRows={result.totalRows}
          rowOffset={(page - 1) * PAGE_SIZE}
          pageSize={PAGE_SIZE}
          bind:pageIndex={dtPageIndex}
          editable={!connectionReadOnly}
          {hiddenColumns}
          {addRowTrigger}
          onAddRow={() => addRowTrigger++}
          onChangePending={handleChangePending}
          onDeleteRowsPending={handleDeleteRowsPending}
          onCellSelect={handleCellSelect}
          onDeselect={() => cellSelectionStore.set(null)}
          onPageInfo={handleDtPageInfo}
          onForeignKeyClick={(foreignKeys.length > 0 || vrStore.hasAnyForTable(connectionId, database, table)) ? handleForeignKeyClick : undefined}
          onForeignKeyQuickView={foreignKeys.length > 0 ? handleForeignKeyQuickView : undefined}
          onConnectColumn={(colName) => { connectColumnName = colName; }}
          initialColWidths={initialColWidths}
          initialColumnOrder={initialColumnOrder}
          onColWidthsChange={(widths) => saveColPrefs(connectionId, database, table, { colWidths: widths })}
          onColumnOrderChange={(order) => saveColPrefs(connectionId, database, table, { columnOrder: order })}
          initialPendingChanges={tableKey === 0 ? _restoredPending?.changes : undefined}
          initialOriginalRows={tableKey === 0 ? _restoredPending?.originalRows : undefined}
          initialDeletedRows={tableKey === 0 ? _restoredPending?.deletedRows : undefined}
        />
      {/key}
    {:else}
      <div class="loading">
        <span class="loading-text">No data.</span>
      </div>
    {/if}
  </div>

  {#if showColumnPicker && currentColumns.length > 0}
    <div use:portal class="picker-positioner" style="top: {pickerTop}px; left: {pickerLeft}px;">
      <ColumnPicker
        columns={currentColumns}
        {hiddenColumns}
        onToggle={toggleColumn}
        onClose={() => (showColumnPicker = false)}
      />
    </div>
  {/if}

  {#if showFilterEditor}
    <div use:portal class="filter-positioner" style="top: {filterEditorTop}px; left: {filterEditorLeft}px;">
      <FilterEditor
        columns={currentColumns}
        value={filterEditorState}
        {dbType}
        onApply={(newState) => {
          filterEditorState = newState;
          page = 1;
          load();
        }}
        onClose={() => (showFilterEditor = false)}
      />
    </div>
  {/if}
</div>

{#if showCsvImport}
  <CsvImportModal
    {connectionId}
    source={importSource}
    onclose={() => (showCsvImport = false)}
    onimported={() => { showCsvImport = false; load(); }}
  />
{/if}

{#if showSqlImport}
  <SqlImportModal
    {connectionId}
    source={importSource}
    onclose={() => (showSqlImport = false)}
  />
{/if}

{#if connectColumnName !== null}
  <VirtualRelationModal
    from={{ connectionId, database, table, column: connectColumnName }}
    onClose={() => (connectColumnName = null)}
  />
{/if}

{#if showSqlPreview}
  <SqlPreviewModal
    statements={buildPreviewStatements()}
    onrun={() => { showSqlPreview = false; saveChanges(); }}
    oncancel={() => { showSqlPreview = false; }}
    ondiscard={() => { showSqlPreview = false; discardChanges(); }}
  />
{/if}

<style>
  .table-browser {
    position: relative;
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    overflow: hidden;
  }

  .toolbar {
    flex-shrink: 0;
    min-height: var(--toolbar-height);
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-1) var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
    overflow: visible;
    position: relative;
    z-index: 10;
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
    cursor: pointer;
    user-select: none;
  }

  .db-name {
    color: var(--color-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-family-mono);
  }

  .separator {
    color: var(--color-text-muted);
    margin: 0 1px;
    flex-shrink: 0;
    font-family: var(--font-family-mono);
  }

  .tbl-name {
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-family-mono);
  }

  .toolbar-right {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--spacing-2);
    margin-left: auto;
  }

  .row-range {
    font-size: 11.5px;
    color: var(--color-text-secondary);
    font-family: var(--font-family-mono);
    flex-shrink: 0;
    white-space: nowrap;
  }

  .page-nav-group {
    display: flex;
    border: 1px solid var(--color-border);
    border-radius: 8px;
    overflow: hidden;
    flex-shrink: 0;
  }

  .page-nav-btn {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: none;
    background: var(--color-bg-tertiary);
    color: var(--color-text-muted);
    cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast);
  }

  .page-nav-btn--next {
    border-left: 1px solid var(--color-border);
  }

  .page-nav-btn:hover:not(:disabled) {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .page-nav-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .toolbar-btn {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    height: calc(var(--toolbar-height) - var(--spacing-2) * 2);
    padding: 0 var(--spacing-2);
    background: var(--color-bg-tertiary);
    border: 1px solid var(--color-border);
    border-radius: 8px;
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

  .save-split-btn {
    display: flex;
    align-items: stretch;
    flex-shrink: 0;
  }

  .save-split-main {
    border-top-right-radius: 0;
    border-bottom-right-radius: 0;
    border-right: none;
  }

  .save-split-arrow {
    border-top-left-radius: 0;
    border-bottom-left-radius: 0;
    padding: 0 6px;
    border-left-color: var(--color-accent);
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
    gap: var(--spacing-1);
    min-width: 28px;
    width: 28px;
    height: 28px;
    padding: 0;
    background: var(--color-bg-tertiary);
    border: 1px solid var(--color-border);
    border-radius: 8px;
    color: var(--color-text-secondary);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
    flex-shrink: 0;
  }

  .refresh-button--labeled {
    width: auto;
    padding: 0 var(--spacing-2);
  }

  .refresh-button:hover:not(:disabled) {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .refresh-button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .refresh-button.filter-active {
    border-color: var(--color-accent);
    color: var(--color-accent);
    background: var(--color-accent-subtle);
  }

  /* ── Filter summary bar ─────────────────────────────────────────────────── */

  .filter-summary-bar {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    padding: var(--spacing-1) var(--spacing-3);
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
    cursor: pointer;
    text-align: left;
    width: 100%;
    border-left: none;
    border-right: none;
    border-top: none;
    font-family: inherit;
    transition: background var(--transition-fast);
  }

  .filter-summary-bar:hover {
    background: var(--color-bg-hover);
  }

  .fsb-group {
    display: contents;
  }

  .fsb-group--bordered {
    display: flex;
    flex-direction: column;
    gap: 1px;
    border-left: 2px solid var(--color-accent);
    padding-left: var(--spacing-2);
  }

  .fsb-group--bordered .fsb-line {
    display: grid;
    grid-template-columns: 3em 1fr;
    align-items: baseline;
    gap: var(--spacing-1);
  }

  .fsb-group--bordered .fsb-line:first-child .fsb-kw {
    text-align: left;
  }

  .fsb-group--bordered .fsb-line:not(:first-child) .fsb-kw {
    text-align: right;
  }

  .fsb-line {
    display: flex;
    align-items: baseline;
    gap: var(--spacing-2);
    line-height: 1.5;
  }

  .fsb-kw {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    font-family: var(--font-family-mono);
    color: var(--color-accent);
    min-width: 3.5em;
    flex-shrink: 0;
    text-align: right;
  }

  .fsb-text {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-primary);
    white-space: pre-wrap;
    word-break: break-all;
  }

  /* ── Save error bar ──────────────────────────────────────────────────────── */

  .save-error-bar {
    flex-shrink: 0;
    display: flex;
    align-items: baseline;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-3);
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
    flex: 1;
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
  }

  .save-error-copy {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    padding: 2px var(--spacing-1);
    background: transparent;
    border: none;
    color: var(--color-danger);
    cursor: pointer;
    border-radius: var(--radius-sm);
    opacity: 0.7;
    transition: opacity var(--transition-fast), background var(--transition-fast);
  }

  .save-error-copy:hover {
    opacity: 1;
    background: color-mix(in srgb, var(--color-danger) 15%, transparent);
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
    background: color-mix(in srgb, var(--color-danger) 15%, transparent);
  }

  /* ── No-PK warning bar ──────────────────────────────────────────────────── */

  .no-pk-warning-bar {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-1) var(--spacing-3);
    background: color-mix(in srgb, var(--color-warning, #f59e0b) 12%, transparent);
    border-bottom: 1px solid color-mix(in srgb, var(--color-warning, #f59e0b) 40%, transparent);
    font-size: var(--font-size-xs);
    color: color-mix(in srgb, var(--color-warning, #f59e0b) 80%, var(--color-text-primary));
  }

  .no-pk-warning-bar svg {
    flex-shrink: 0;
    color: var(--color-warning, #f59e0b);
  }

  .no-pk-warning-text {
    flex: 1;
    font-family: var(--font-family-ui);
  }

  .no-pk-warning-btn {
    flex-shrink: 0;
    padding: 2px var(--spacing-2);
    background: transparent;
    border: 1px solid color-mix(in srgb, var(--color-warning, #f59e0b) 50%, transparent);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    color: inherit;
    cursor: pointer;
    white-space: nowrap;
    transition: background var(--transition-fast);
  }

  .no-pk-warning-btn:hover {
    background: color-mix(in srgb, var(--color-warning, #f59e0b) 20%, transparent);
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

  .error-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .error-label {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-danger);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .error-copy {
    display: flex;
    align-items: center;
    padding: 2px var(--spacing-1);
    background: transparent;
    border: none;
    color: var(--color-danger);
    cursor: pointer;
    border-radius: var(--radius-sm);
    opacity: 0.6;
    transition: opacity var(--transition-fast), background var(--transition-fast);
  }

  .error-copy:hover {
    opacity: 1;
    background: color-mix(in srgb, var(--color-danger) 15%, transparent);
  }

  .error-message {
    font-size: var(--font-size-sm);
    color: var(--color-danger);
    font-family: var(--font-family-mono);
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
  }

  /* ── Column picker positioner ────────────────────────────────────────────── */

  :global(.picker-positioner),
  :global(.filter-positioner),
  :global(.export-positioner) {
    position: fixed;
    z-index: 1000;
  }

  /* ── Export ─────────────────────────────────────────────────────────────── */

  .export-dropdown {
    position: relative;
    flex-shrink: 0;
  }

  .export-menu {
    background: var(--color-bg-overlay);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-overlay);
    min-width: 210px;
    padding: var(--spacing-1) 0;
  }

  .export-menu-title {
    padding: var(--spacing-1) var(--spacing-2);
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--color-text-muted);
    border-bottom: 1px solid var(--color-border);
  }

  .export-menu-section {
    display: grid;
    grid-template-columns: 1fr auto auto;
    align-items: center;
    gap: 0;
    padding: var(--spacing-1) var(--spacing-2);
    border-bottom: 1px solid var(--color-border);
  }

  .export-menu-section:last-child {
    border-bottom: none;
  }

  .export-format-label {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-primary);
    padding-right: var(--spacing-2);
  }

  .export-menu-item {
    padding: var(--spacing-1) var(--spacing-2);
    background: transparent;
    border: none;
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
    cursor: pointer;
    border-radius: var(--radius-sm);
    white-space: nowrap;
    transition: background var(--transition-fast), color var(--transition-fast);
    font-family: var(--font-family-ui);
  }

  .export-menu-item:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .export-backdrop {
    position: fixed;
    inset: 0;
    z-index: 299;
  }

  .table-name-export-bar {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-1) var(--spacing-3);
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
    font-size: var(--font-size-xs);
  }

  .table-name-label {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    white-space: nowrap;
    flex-shrink: 0;
  }

  .table-name-input {
    padding: var(--spacing-1) var(--spacing-2);
    background: var(--color-bg-primary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-primary);
    outline: none;
    width: 180px;
    transition: border-color var(--transition-fast);
  }

  .table-name-input:focus {
    border-color: var(--color-accent);
  }
</style>
