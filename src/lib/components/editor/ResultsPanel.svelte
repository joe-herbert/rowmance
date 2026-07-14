<script lang="ts">
  import { tick, untrack } from 'svelte';
  import type { QueryResult } from '$lib/types';
  import type { ColumnMeta } from '$lib/types';
  import Loader from '$lib/components/ui/Loader.svelte';
  import DataTable from '$lib/components/table/DataTable.svelte';
  import ColumnPicker from '$lib/components/table/ColumnPicker.svelte';
  import {
    exportResultToFile,
    exportResultToClipboard,
    type ExportFormat,
  } from '$lib/tauri/export';
  import { save as saveDialog } from '@tauri-apps/plugin-dialog';
  import { errorMessage } from '$lib/utils/errors';
  import { useToast } from '$lib/stores/toast.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import type { RowChange, RowDelete } from '$lib/tauri/query';
  import { saveTableChanges } from '$lib/tauri/query';
  import { portal } from '$lib/actions/portal';
  import SqlHighlight from '$lib/components/ui/SqlHighlight.svelte';
  import AiModal from '$lib/components/ai/AiModal.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import { useCellSelection } from '$lib/stores/cellSelection.svelte';

  const toast = useToast();
  const connections = useConnections();
  const settingsStore = useSettings();
  const cellSelectionStore = useCellSelection();

  interface Props {
    results: QueryResult[];
    statements?: string[];
    connectionId?: string;
    database?: string;
    isRunning?: boolean;
    initialActiveTab?: number;
    onActiveTabChange?: (_tab: number) => void;
    variableValues?: Record<string, string | null>;
  }

  let {
    results,
    statements = [],
    connectionId,
    database,
    isRunning = false,
    initialActiveTab = 0,
    onActiveTabChange,
    variableValues = {},
  }: Props = $props();

  type CellValue = string | number | boolean | null;

  let activeTab = $state(untrack(() => initialActiveTab));

  $effect(() => {
    // Reset to first tab whenever results change (new execution).
    results;
    untrack(() => {
      activeTab = 0;
    });
  });

  $effect(() => {
    const tab = activeTab;
    untrack(() => onActiveTabChange?.(tab));
  });

  let result = $derived(results[activeTab] ?? null);

  let successCount = $derived(results.filter((r) => r.error === null).length);
  let errorCount = $derived(results.filter((r) => r.error !== null).length);

  let rowLabel = $derived(
    result
      ? `${result.rows.length.toLocaleString()} row${result.rows.length !== 1 ? 's' : ''}`
      : '',
  );

  function formatDuration(us: number): string {
    if (us >= 1_000_000) return `${(us / 1_000_000).toFixed(2)}s`;
    if (us >= 1_000) return `${(us / 1_000).toFixed(1)}ms`;
    return `${us}µs`;
  }
  let durationLabel = $derived(result ? formatDuration(result.durationUs) : '');

  let affectedLabel = $derived(
    result?.affectedRows != null ? `${result.affectedRows} affected` : '',
  );

  // Whether we have a meaningful data payload (successful query with columns).
  let hasData = $derived(result !== null && result.error === null && result.columns.length > 0);

  // Strip leading -- and /* */ comments so the tab shows the executable SQL,
  // not the comment text (which would collapse into the SQL visually at nowrap).
  function displaySql(sql: string): string {
    let s = sql;
    for (;;) {
      s = s.trimStart();
      if (s.startsWith('--')) {
        const nl = s.indexOf('\n');
        if (nl === -1) return '';
        s = s.slice(nl + 1);
      } else if (s.startsWith('/*')) {
        const end = s.indexOf('*/');
        if (end === -1) return '';
        s = s.slice(end + 2);
      } else {
        break;
      }
    }
    return s.trim();
  }

  // Detect @varnames being SET in the active statement (MySQL/MariaDB style).
  let setVariableNames = $derived.by((): string[] => {
    const stmt = statements[activeTab];
    if (!stmt) return [];
    const cleaned = stmt
      .replace(/--[^\n]*/g, '')
      .replace(/\/\*[\s\S]*?\*\//g, '')
      .trim();
    if (!/^\s*SET\s+@/i.test(cleaned)) return [];
    const re = /@([a-zA-Z_]\w*)\s*:?=/g;
    const vars: string[] = [];
    let m;
    while ((m = re.exec(cleaned)) !== null) {
      vars.push(`@${m[1]}`);
    }
    return vars;
  });

  // ── Editing state ─────────────────────────────────────────────────────────

  let pendingChanges = $state<Map<string, Map<string, CellValue>>>(new Map());
  let originalRows = $state<Map<string, CellValue[]>>(new Map());
  let pendingDeletedRows = $state<Map<string, CellValue[]>>(new Map());
  let addRowTrigger = $state(0);
  let tableKey = $state(0);
  let isSaving = $state(false);
  let saveError = $state<string | null>(null);

  // Column management (session-only, no persistence for query results)
  let hiddenColumns = $state<Set<string>>(new Set());
  let colWidths = $state<Record<string, number>>({});
  let columnOrder = $state<string[]>([]);
  let columnOrderOverride = $state<string[] | undefined>(undefined);

  // AI summarise
  let showAiSummarise = $state(false);

  // Search
  let showLocalSearch = $state(false);
  let localSearchTerm = $state('');
  let localSearchInputEl = $state<HTMLInputElement | null>(null);

  // Column picker
  let showColumnPicker = $state(false);
  let columnPickerAnchorEl = $state<HTMLButtonElement | null>(null);
  let pickerTop = $state(0);
  let pickerLeft = $state(0);

  // ── Derived editing values ────────────────────────────────────────────────

  let connectionReadOnly = $derived(
    connectionId ? (connections.getById(connectionId)?.readOnly ?? false) : true,
  );
  let canEdit = $derived(!!connectionId && !connectionReadOnly && hasData);

  // Extract table name from the active statement's SQL
  let detectedTable = $derived.by(() => {
    const stmt = statements[activeTab];
    if (!stmt) return null;
    const m = stmt.match(/\bFROM\s+(?:[`"']?\w+[`"']?\s*\.\s*)?[`"']?(\w+)[`"']?\b/i);
    return m ? m[1] : null;
  });

  // Also try to detect database from SQL (e.g., FROM mydb.users)
  let detectedDatabase = $derived.by(() => {
    const stmt = statements[activeTab];
    if (!stmt || !database) return database ?? null;
    const m = stmt.match(/\bFROM\s+[`"']?(\w+)[`"']?\s*\.\s*[`"']?\w+[`"']?\b/i);
    return m ? m[1] : (database ?? null);
  });

  let canSave = $derived(canEdit && detectedTable !== null && !!detectedDatabase);

  function handleCellSelect(colIndex: number, row: CellValue[]): void {
    if (!result || !connectionId || !detectedTable || !detectedDatabase) return;
    const col = result.columns[colIndex];
    if (!col) return;
    cellSelectionStore.set({
      connectionId,
      database: detectedDatabase,
      table: detectedTable,
      columnName: col.name,
      cellValue: row[colIndex],
      row,
      columns: result.columns,
    });
  }

  function handleRowSelect(row: CellValue[], cols: ColumnMeta[]): void {
    if (!connectionId || !detectedTable || !detectedDatabase) return;
    cellSelectionStore.set({
      connectionId,
      database: detectedDatabase,
      table: detectedTable,
      row,
      columns: cols,
    });
  }

  let pendingCount = $derived(
    [...pendingChanges.values()].reduce((sum, colMap) => sum + colMap.size, 0) +
      pendingDeletedRows.size,
  );

  // Client-side filtered rows for local search
  let displayRows = $derived.by(() => {
    if (!result) return [];
    const term = localSearchTerm.trim().toLowerCase();
    if (!term) return result.rows;
    return result.rows.filter((row) =>
      row.some((cell) => cell !== null && String(cell).toLowerCase().includes(term)),
    );
  });

  let currentColumns = $derived<ColumnMeta[]>(result?.columns ?? []);

  // ── Reset effects ─────────────────────────────────────────────────────────

  $effect(() => {
    results;
    untrack(() => {
      pendingChanges = new Map();
      originalRows = new Map();
      pendingDeletedRows = new Map();
      addRowTrigger = 0;
      saveError = null;
      hiddenColumns = new Set();
      colWidths = {};
      columnOrder = [];
      columnOrderOverride = undefined;
      showLocalSearch = false;
      localSearchTerm = '';
      tableKey++;
    });
  });

  $effect(() => {
    activeTab;
    untrack(() => {
      pendingChanges = new Map();
      originalRows = new Map();
      pendingDeletedRows = new Map();
      addRowTrigger = 0;
      saveError = null;
      tableKey++;
    });
  });

  // ── Editing callbacks ─────────────────────────────────────────────────────

  function handleChangePending(
    changes: Map<string, Map<string, CellValue>>,
    rows: Map<string, CellValue[]>,
  ): void {
    pendingChanges = new Map([...changes].map(([k, v]) => [k, new Map(v)]));
    originalRows = new Map([...rows].map(([k, v]) => [k, [...v]]));
  }

  function handleDeleteRowsPending(deletedRows: Map<string, CellValue[]>): void {
    pendingDeletedRows = new Map([...deletedRows].map(([k, v]) => [k, [...v]]));
  }

  function discardChanges(): void {
    pendingChanges = new Map();
    originalRows = new Map();
    pendingDeletedRows = new Map();
    addRowTrigger = 0;
    tableKey++;
  }

  async function saveChanges(): Promise<void> {
    if (!result || !detectedTable || !detectedDatabase || !connectionId) return;
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

      await saveTableChanges(
        connectionId,
        detectedDatabase,
        detectedTable,
        rowChanges,
        insertValues,
        deleteChanges,
      );
      pendingChanges = new Map();
      pendingDeletedRows = new Map();
      addRowTrigger = 0;
      tableKey++;
      toast.addToast('Changes saved', 'success', 2000);
    } catch (err) {
      saveError = errorMessage(err);
    } finally {
      isSaving = false;
    }
  }

  function toggleColumn(name: string): void {
    const next = new Set(hiddenColumns);
    if (next.has(name)) next.delete(name);
    else next.add(name);
    hiddenColumns = next;
  }

  function openColumnPicker(): void {
    if (columnPickerAnchorEl) {
      const rect = columnPickerAnchorEl.getBoundingClientRect();
      pickerTop = rect.bottom + 4;
      pickerLeft = rect.right - 320;
    }
    showColumnPicker = true;
  }

  function openLocalSearch(): void {
    showLocalSearch = true;
    tick().then(() => localSearchInputEl?.focus());
  }

  function closeLocalSearch(): void {
    showLocalSearch = false;
    localSearchTerm = '';
  }

  function handleLocalSearchKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape') closeLocalSearch();
  }

  // ── Export state ──────────────────────────────────────────────────────────

  let showExportMenu = $state(false);
  let exportTableName = $state('');
  let showTableNameInput = $state(false);
  let pendingExportFormat = $state<ExportFormat | null>(null);
  let pendingExportToFile = $state(false);
  let exportError = $state<string | null>(null);
  let exportBtnEl = $state<HTMLButtonElement | null>(null);
  let exportMenuEl = $state<HTMLDivElement | null>(null);
  let menuTop = $state(0);
  let menuLeft = $state(0);

  function positionMenu() {
    if (!exportBtnEl || !exportMenuEl) return;
    const rect = exportBtnEl.getBoundingClientRect();
    menuTop = rect.top - exportMenuEl.offsetHeight - 4;
    menuLeft = rect.right - exportMenuEl.offsetWidth;
  }

  $effect(() => {
    if (!showExportMenu) return;
    requestAnimationFrame(positionMenu);
    function onMousedown(e: MouseEvent) {
      const t = e.target as Node;
      if (!exportBtnEl?.contains(t) && !exportMenuEl?.contains(t)) showExportMenu = false;
    }
    document.addEventListener('mousedown', onMousedown, true);
    return () => {
      document.removeEventListener('mousedown', onMousedown, true);
    };
  });

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

  function getColumns(): string[] {
    return result?.columns.map((c) => c.name) ?? [];
  }

  function getRows(): unknown[][] {
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

  async function doExport(
    format: ExportFormat,
    toFile: boolean,
    tableName: string | undefined,
  ): Promise<void> {
    exportError = null;
    try {
      if (toFile) {
        const ext = FORMAT_EXTENSIONS[format];
        const filePath = await saveDialog({
          defaultPath: `export.${ext}`,
          filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
        });
        if (!filePath) return; // user cancelled
        await exportResultToFile(getRows(), getColumns(), format, filePath, tableName);
      } else {
        await exportResultToClipboard(getRows(), getColumns(), format, tableName);
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
</script>

<div class="results-panel">
  {#if isRunning}
    <div class="placeholder">
      <Loader />
    </div>
  {:else if results.length === 0}
    <div class="placeholder">
      <span class="placeholder-text">Run a query to see results</span>
    </div>
  {:else}
    {#if results.length > 1}
      <div class="tab-bar" role="tablist">
        <div class="tab-counts">
          {#if successCount > 0}
            <span class="tab-count-ok">{successCount} ok</span>
          {/if}
          {#if errorCount > 0}
            <span class="tab-count-err">{errorCount} err</span>
          {/if}
        </div>
        <div class="tab-list">
          {#each results as r, i}
            <button
              class="tab-btn"
              class:tab-btn--active={activeTab === i}
              class:tab-btn--ok={r.error === null}
              class:tab-btn--error={r.error !== null}
              role="tab"
              aria-selected={activeTab === i}
              onclick={() => {
                activeTab = i;
              }}
            >
              {i + 1}
            </button>
          {/each}
        </div>
        {#if statements[activeTab]}
          <div class="tab-sql-wrap" title={statements[activeTab]}>
            <SqlHighlight sql={displaySql(statements[activeTab])} class="tab-sql" />
          </div>
        {/if}
      </div>
    {/if}

    {#if result !== null && result.error !== null}
      <div class="error-box" role="alert">
        <div class="error-header">
          <span class="error-label">Error</span>
          <button
            class="error-copy"
            onclick={() =>
              navigator.clipboard
                .writeText(result!.error!)
                .then(() => toast.addToast('Copied', 'success', 1500))}
            aria-label="Copy error message"
            title="Copy error"
          >
            <svg
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
              ><rect x="9" y="9" width="13" height="13" rx="2" /><path
                d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"
              /></svg
            >
          </button>
        </div>
        <span class="error-message">{result.error}</span>
      </div>
      <div class="flex-fill"></div>
      <div class="status-bar">
        <span class="status-item">{durationLabel}</span>
        <div class="status-spacer"></div>
      </div>
    {:else if hasData}
      <div class="results-toolbar">
        {#if canEdit}
          <button
            class="toolbar-btn"
            onclick={() => {
              addRowTrigger++;
            }}
          >
            <svg
              width="10"
              height="10"
              viewBox="0 0 10 10"
              fill="none"
              stroke="currentColor"
              stroke-width="1.8"
              stroke-linecap="round"
              aria-hidden="true"
              ><line x1="5" y1="1" x2="5" y2="9" /><line x1="1" y1="5" x2="9" y2="5" /></svg
            >
            Add Row
          </button>
        {/if}
        <button bind:this={columnPickerAnchorEl} class="toolbar-btn" onclick={openColumnPicker}>
          Columns
        </button>
        <button class="toolbar-btn" onclick={openLocalSearch}> Search </button>
        {#if settingsStore.settings.aiProvider !== 'none' && settingsStore.settings.aiContextLevel === 'structure_and_data'}
          <button class="toolbar-btn" onclick={() => { showAiSummarise = true; }}>
            AI Summarise
          </button>
        {/if}
        <div class="toolbar-spacer"></div>
        {#if pendingCount > 0}
          <span class="pending-label">{pendingCount} pending</span>
          {#if canSave}
            <button class="toolbar-btn toolbar-btn--save" onclick={saveChanges} disabled={isSaving}>
              {isSaving ? 'Saving…' : 'Save'}
            </button>
          {/if}
          <button class="toolbar-btn toolbar-btn--discard" onclick={discardChanges}>
            Discard
          </button>
        {/if}
      </div>

      {#if showLocalSearch}
        <div class="local-search-bar">
          <input
            bind:this={localSearchInputEl}
            class="local-search-input"
            type="text"
            placeholder="Search results…"
            bind:value={localSearchTerm}
            onkeydown={handleLocalSearchKeydown}
          />
          <button class="local-search-close" onclick={closeLocalSearch}>✕</button>
        </div>
      {/if}

      <div class="table-wrapper">
        {#key tableKey}
          <DataTable
            columns={result.columns}
            rows={displayRows}
            editable={canEdit}
            {hiddenColumns}
            {addRowTrigger}
            onAddRow={() => {
              addRowTrigger++;
            }}
            onChangePending={handleChangePending}
            onDeleteRowsPending={handleDeleteRowsPending}
            initialColWidths={Object.keys(colWidths).length > 0 ? colWidths : undefined}
            initialColumnOrder={columnOrder.length > 0 ? columnOrder : undefined}
            {columnOrderOverride}
            onColWidthsChange={(widths) => {
              colWidths = widths;
            }}
            onColumnOrderChange={(order) => {
              columnOrder = order;
            }}
            {connectionId}
            database={detectedDatabase ?? undefined}
            onCellSelect={detectedTable ? handleCellSelect : undefined}
            onRowSelect={detectedTable ? handleRowSelect : undefined}
          />
        {/key}
      </div>

      {#if showColumnPicker && currentColumns.length > 0}
        <div use:portal class="picker-positioner" style="top: {pickerTop}px; left: {pickerLeft}px;">
          <ColumnPicker
            columns={currentColumns}
            {hiddenColumns}
            columnOrder={columnOrder.length > 0 ? columnOrder : currentColumns.map((c) => c.name)}
            onToggle={toggleColumn}
            onClose={() => {
              showColumnPicker = false;
            }}
            onReorder={(order) => {
              columnOrder = order;
              columnOrderOverride = [...order];
            }}
            onReset={() => {
              hiddenColumns = new Set();
              const dbOrder = currentColumns.map((c) => c.name);
              columnOrder = dbOrder;
              columnOrderOverride = [...dbOrder];
            }}
          />
        </div>
      {/if}

      {#if showTableNameInput}
        <div class="export-details-bar">
          <div class="table-name-input-row">
            <label class="table-name-label" for="rp-table-name">Table name:</label>
            <input
              id="rp-table-name"
              class="table-name-input"
              type="text"
              placeholder="table_name"
              bind:value={exportTableName}
              onkeydown={handleExportKeydown}
              aria-label="Table name for SQL INSERT export"
            />
            <button class="export-confirm-btn" onclick={confirmTableNameExport}>Export</button>
            <button class="export-cancel-btn" onclick={cancelTableNameExport}>Cancel</button>
          </div>
        </div>
      {/if}
      {#if exportError}
        <div class="export-details-bar">
          <div class="export-error-wrap">
            <span class="export-error">{exportError}</span>
            <button
              class="export-error-copy"
              onclick={() =>
                navigator.clipboard
                  .writeText(exportError!)
                  .then(() => toast.addToast('Copied', 'success', 1500))}
              aria-label="Copy error message"
              title="Copy"
            >
              <svg
                width="11"
                height="11"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
                ><rect x="9" y="9" width="13" height="13" rx="2" /><path
                  d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"
                /></svg
              >
            </button>
          </div>
        </div>
      {/if}
      {#if saveError}
        <div class="save-error-bar">
          <span class="save-error">{saveError}</span>
        </div>
      {/if}
      <div class="status-bar">
        <span class="status-item">{rowLabel}</span>
        {#if durationLabel}
          <span class="status-separator">·</span>
          <span class="status-item">{durationLabel}</span>
        {/if}
        {#if affectedLabel}
          <span class="status-separator">·</span>
          <span class="status-item">{affectedLabel}</span>
        {/if}
        <div class="status-spacer"></div>
        <div class="export-dropdown">
          <button
            bind:this={exportBtnEl}
            class="export-btn"
            onclick={() => {
              showExportMenu = !showExportMenu;
              exportError = null;
            }}
            aria-expanded={showExportMenu}
            aria-label="Export results"
          >
            Export ▾
          </button>

          {#if showExportMenu}
            <div
              bind:this={exportMenuEl}
              class="export-menu"
              role="menu"
              style="top:{menuTop}px;left:{menuLeft}px"
              use:portal
            >
              {#each EXPORT_FORMATS as fmt}
                <div class="export-menu-section">
                  <span class="export-format-label">{fmt.label}</span>
                  <button
                    class="export-menu-item"
                    role="menuitem"
                    onclick={() => startExport(fmt.format, false)}
                  >
                    To Clipboard
                  </button>
                  <button
                    class="export-menu-item"
                    role="menuitem"
                    onclick={() => startExport(fmt.format, true)}
                  >
                    To File
                  </button>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      </div>
    {:else if result !== null}
      <!-- Query ran successfully but returned no columns (e.g. DDL, SET, or empty result). -->
      <div class="empty-result">
        {#if setVariableNames.length > 0}
          <div class="set-variable-result">
            {#each setVariableNames as varName}
              <div class="set-variable-row">
                <code class="set-variable-name">{varName}</code>
                <span class="set-variable-eq">=</span>
                {#if varName in variableValues}
                  {#if variableValues[varName] === null}
                    <code class="set-variable-value set-variable-value--null">NULL</code>
                  {:else}
                    <code class="set-variable-value">{variableValues[varName]}</code>
                  {/if}
                {:else}
                  <span class="set-variable-value set-variable-value--unknown">set</span>
                {/if}
              </div>
            {/each}
          </div>
        {:else}
          <span class="empty-text">Query executed successfully</span>
        {/if}
      </div>
      <div class="status-bar">
        {#if durationLabel}
          <span class="status-item">{durationLabel}</span>
        {/if}
        {#if affectedLabel}
          <span class="status-separator">·</span>
          <span class="status-item">{affectedLabel}</span>
        {/if}
        <div class="status-spacer"></div>
      </div>
    {/if}
  {/if}
</div>

{#if showAiSummarise && result && statements[activeTab]}
  <AiModal
    mode="summarise"
    sql={statements[activeTab]}
    columns={result.columns.map((c) => c.name)}
    rows={result.rows}
    connectionId={connectionId ?? ''}
    database={database ?? ''}
    onclose={() => { showAiSummarise = false; }}
  />
{/if}

<style>
  .results-panel {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    overflow: hidden;
  }

  .placeholder {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .placeholder-text {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
  }

  /* ── Result tabs ────────────────────────────────────────────────────────── */

  .tab-bar {
    flex-shrink: 0;
    display: flex;
    align-items: stretch;
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    overflow: hidden;
  }

  .tab-counts {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: 0 var(--spacing-3);
    flex-shrink: 0;
    border-right: 1px solid var(--color-border);
  }

  .tab-count-ok {
    font-size: var(--font-size-xs);
    color: var(--color-success, #22c55e);
    font-variant-numeric: tabular-nums;
  }

  .tab-count-err {
    font-size: var(--font-size-xs);
    color: var(--color-danger);
    font-variant-numeric: tabular-nums;
  }

  .tab-list {
    display: flex;
    align-items: stretch;
    flex: 1 1 0;
    min-width: 0;
    overflow-x: auto;
    scrollbar-width: none;
  }

  .tab-list::-webkit-scrollbar {
    display: none;
  }

  .status-ok {
    font-size: var(--font-size-xs);
    color: var(--color-success, #22c55e);
    font-variant-numeric: tabular-nums;
  }

  .status-err {
    font-size: var(--font-size-xs);
    color: var(--color-danger);
    font-variant-numeric: tabular-nums;
  }

  .tab-sql-wrap {
    flex: 0 0 auto;
    max-width: 50%;
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: none;
    border-left: 1px solid var(--color-border);
    display: flex;
    align-items: center;
  }

  .tab-sql-wrap::-webkit-scrollbar {
    display: none;
  }

  .tab-sql-wrap :global(.tab-sql) {
    padding: 0 var(--spacing-3);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    white-space: nowrap;
  }

  .tab-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: 0 var(--spacing-3);
    height: 30px;
    border: none;
    border-right: 1px solid var(--color-border);
    background: transparent;
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
    cursor: pointer;
    white-space: nowrap;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .tab-btn:hover {
    filter: brightness(1.06);
    color: var(--color-text-primary);
  }

  .tab-btn--ok {
    background: color-mix(in srgb, var(--color-success, #22c55e) 10%, transparent);
  }

  .tab-btn--error {
    background: color-mix(in srgb, var(--color-danger) 12%, transparent);
  }

  .tab-btn--active {
    color: var(--color-text-primary);
    box-shadow: inset 0 -2px 0 var(--color-accent);
  }

  .tab-btn--active.tab-btn--ok {
    background: color-mix(in srgb, var(--color-success, #22c55e) 14%, var(--color-bg-primary));
  }

  .tab-btn--active.tab-btn--error {
    background: color-mix(in srgb, var(--color-danger) 16%, var(--color-bg-primary));
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
    transition:
      opacity var(--transition-fast),
      background var(--transition-fast);
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
    -webkit-user-select: text;
    user-select: text;
  }

  .table-wrapper {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .empty-result {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .empty-text {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
  }

  .set-variable-result {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
    align-items: center;
  }

  .set-variable-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    font-size: var(--font-size-sm);
  }

  .set-variable-name {
    font-family: var(--font-family-mono);
    color: var(--color-editor-keyword, var(--color-accent));
  }

  .set-variable-eq {
    color: var(--color-text-muted);
  }

  .set-variable-value {
    font-family: var(--font-family-mono);
    color: var(--color-editor-string, var(--color-text-primary));
    background: var(--color-bg-tertiary);
    padding: 1px var(--spacing-2);
    border-radius: var(--radius-sm);
    border: 1px solid var(--color-border);
  }

  .set-variable-value--null {
    color: var(--color-text-muted);
    font-style: italic;
  }

  .set-variable-value--unknown {
    font-family: var(--font-family-ui);
    color: var(--color-text-muted);
    background: transparent;
    border-color: transparent;
    font-size: var(--font-size-xs);
  }

  .status-bar {
    flex: 0 0 var(--statusbar-height);
    min-height: var(--statusbar-height);
    max-height: var(--statusbar-height);
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: 0 var(--spacing-3);
    background: var(--color-bg-secondary);
    border-top: 1px solid var(--color-border);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  .status-separator {
    color: var(--color-border-strong);
  }

  .status-item {
    font-variant-numeric: tabular-nums;
  }

  /* ── Results toolbar ─────────────────────────────────────────────────────── */

  .results-toolbar {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: 0 var(--spacing-2);
    height: 30px;
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
  }

  .toolbar-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: var(--spacing-1) var(--spacing-2);
    background: var(--color-bg-tertiary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
    cursor: pointer;
    white-space: nowrap;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .toolbar-btn:hover:not(:disabled) {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .toolbar-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .toolbar-btn--save {
    background: var(--color-accent-subtle);
    border-color: var(--color-accent);
    color: var(--color-accent);
  }

  .toolbar-btn--save:hover:not(:disabled) {
    background: var(--color-accent);
    color: var(--color-text-on-accent);
  }

  .toolbar-btn--discard {
    border-color: var(--color-danger);
    color: var(--color-danger);
  }

  .toolbar-btn--discard:hover:not(:disabled) {
    background: var(--color-danger-subtle);
  }

  .toolbar-spacer {
    flex: 1;
  }

  .pending-label {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-variant-numeric: tabular-nums;
  }

  /* ── Local search ────────────────────────────────────────────────────────── */

  .local-search-bar {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: var(--spacing-1) var(--spacing-2);
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
  }

  .local-search-input {
    flex: 1;
    padding: var(--spacing-1) var(--spacing-2);
    background: var(--color-bg-primary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    color: var(--color-text-primary);
    outline: none;
    transition: border-color var(--transition-fast);
  }

  .local-search-input:focus {
    border-color: var(--color-accent);
  }

  .local-search-close {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .local-search-close:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  /* ── Column picker positioner ────────────────────────────────────────────── */

  .picker-positioner {
    position: fixed;
    z-index: 300;
  }

  /* ── Save error ──────────────────────────────────────────────────────────── */

  .save-error-bar {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    padding: var(--spacing-1) var(--spacing-3);
    background: var(--color-danger-subtle);
    border-top: 1px solid var(--color-danger);
  }

  .save-error {
    font-size: var(--font-size-xs);
    color: var(--color-danger);
    font-family: var(--font-family-mono);
    white-space: pre-wrap;
    word-break: break-word;
  }

  /* ── Export ─────────────────────────────────────────────────────────────── */

  .export-details-bar {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-1) var(--spacing-3);
    background: var(--color-bg-secondary);
    border-top: 1px solid var(--color-border);
  }

  .flex-fill {
    flex: 1;
  }

  .status-spacer {
    flex: 1;
  }

  .export-dropdown {
    position: relative;
  }

  .export-btn {
    padding: var(--spacing-1) var(--spacing-2);
    background: var(--color-bg-tertiary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
    white-space: nowrap;
  }

  .export-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .export-menu {
    position: fixed;
    background: var(--color-bg-overlay);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-overlay);
    z-index: 300;
    min-width: 200px;
    padding: var(--spacing-1) 0;
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
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .export-menu-item:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .table-name-input-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
  }

  .table-name-label {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    white-space: nowrap;
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
    width: 160px;
    transition: border-color var(--transition-fast);
  }

  .table-name-input:focus {
    border-color: var(--color-accent);
  }

  .export-confirm-btn {
    padding: var(--spacing-1) var(--spacing-2);
    background: var(--color-accent-subtle);
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    color: var(--color-accent);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .export-confirm-btn:hover {
    background: var(--color-accent);
    color: var(--color-text-on-accent);
  }

  .export-cancel-btn {
    padding: var(--spacing-1) var(--spacing-2);
    background: transparent;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .export-cancel-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .export-error-wrap {
    display: flex;
    align-items: baseline;
    gap: var(--spacing-1);
    flex: 1;
    min-width: 0;
  }

  .export-error {
    font-size: var(--font-size-xs);
    color: var(--color-danger);
    font-family: var(--font-family-mono);
    white-space: pre-wrap;
    word-break: break-word;
    flex: 1;
  }

  .export-error-copy {
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
    transition:
      opacity var(--transition-fast),
      background var(--transition-fast);
  }

  .export-error-copy:hover {
    opacity: 1;
    background: color-mix(in srgb, var(--color-danger) 15%, transparent);
  }
</style>
