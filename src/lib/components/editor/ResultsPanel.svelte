<script lang="ts">
  import type { QueryResult } from '$lib/types';
  import DataTable from '$lib/components/table/DataTable.svelte';
  import { exportResultToFile, exportResultToClipboard, type ExportFormat } from '$lib/tauri/export';
  import { save as saveDialog } from '@tauri-apps/plugin-dialog';
  import { errorMessage } from '$lib/utils/errors';
  import { useToast } from '$lib/stores/toast.svelte';

  const toast = useToast();

  interface Props {
    result: QueryResult | null;
  }

  let { result }: Props = $props();

  let rowLabel = $derived(
    result
      ? `${result.rows.length.toLocaleString()} row${result.rows.length !== 1 ? 's' : ''}`
      : '',
  );

  let durationLabel = $derived(result ? `${result.durationMs}ms` : '');

  let affectedLabel = $derived(
    result?.affectedRows != null ? `${result.affectedRows} affected` : '',
  );

  // Whether we have a meaningful data payload (successful query with columns).
  let hasData = $derived(result !== null && result.error === null && result.columns.length > 0);

  // ── Export state ──────────────────────────────────────────────────────────

  let showExportMenu = $state(false);
  let exportTableName = $state('');
  let showTableNameInput = $state(false);
  let pendingExportFormat = $state<ExportFormat | null>(null);
  let pendingExportToFile = $state(false);
  let exportError = $state<string | null>(null);

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

  async function doExport(format: ExportFormat, toFile: boolean, tableName: string | undefined): Promise<void> {
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
  {#if result === null}
    <div class="placeholder">
      <span class="placeholder-text">Run a query to see results</span>
    </div>
  {:else if result.error !== null}
    <div class="error-box" role="alert">
      <div class="error-header">
        <span class="error-label">Error</span>
        <button class="error-copy" onclick={() => navigator.clipboard.writeText(result!.error!).then(() => toast.addToast('Copied', 'success', 1500))} aria-label="Copy error message" title="Copy error">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
        </button>
      </div>
      <span class="error-message">{result.error}</span>
    </div>
  {:else if hasData}
    <div class="export-toolbar">
      <div class="export-dropdown">
        <button
          class="export-btn"
          onclick={() => { showExportMenu = !showExportMenu; exportError = null; }}
          aria-expanded={showExportMenu}
          aria-label="Export results"
        >
          Export ▾
        </button>

        {#if showExportMenu}
          <div class="export-menu" role="menu">
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
          <div
            class="export-backdrop"
            role="presentation"
            onclick={() => (showExportMenu = false)}
            onkeydown={(e) => { if (e.key === 'Escape') showExportMenu = false; }}
          ></div>
        {/if}
      </div>

      {#if showTableNameInput}
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
      {/if}

      {#if exportError}
        <div class="export-error-wrap">
          <span class="export-error">{exportError}</span>
          <button class="export-error-copy" onclick={() => navigator.clipboard.writeText(exportError!).then(() => toast.addToast('Copied', 'success', 1500))} aria-label="Copy error message" title="Copy">
            <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
          </button>
        </div>
      {/if}
    </div>

    <div class="table-wrapper">
      <DataTable columns={result.columns} rows={result.rows} />
    </div>
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
    </div>
  {:else}
    <!-- Query ran successfully but returned no columns (e.g. DDL or empty result). -->
    <div class="empty-result">
      <span class="empty-text">Query executed successfully — no rows returned.</span>
    </div>
    <div class="status-bar">
      {#if durationLabel}
        <span class="status-item">{durationLabel}</span>
      {/if}
      {#if affectedLabel}
        <span class="status-separator">·</span>
        <span class="status-item">{affectedLabel}</span>
      {/if}
    </div>
  {/if}
</div>

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

  /* ── Export toolbar ─────────────────────────────────────────────────────── */

  .export-toolbar {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-1) var(--spacing-3);
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
    min-height: var(--toolbar-height);
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
    transition: background var(--transition-fast), color var(--transition-fast);
    white-space: nowrap;
  }

  .export-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .export-menu {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    background: var(--color-bg-overlay);
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
    transition: background var(--transition-fast), color var(--transition-fast);
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
    transition: background var(--transition-fast), color var(--transition-fast);
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
    transition: opacity var(--transition-fast), background var(--transition-fast);
  }

  .export-error-copy:hover {
    opacity: 1;
    background: color-mix(in srgb, var(--color-danger) 15%, transparent);
  }
</style>
