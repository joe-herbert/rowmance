<!--
  CsvImportModal — multi-step CSV import dialog.
  Step 1: file picker.
  Step 2: preview table with editable column names and type dropdowns.
-->
<script lang="ts">
  import { open as openFileDialog } from '@tauri-apps/plugin-dialog';
  import * as importApi from '$lib/tauri/import';
  import type { InferredColumn } from '$lib/tauri/import';

  interface Props {
    connectionId: string;
    onclose: () => void;
    onimported?: (rowCount: number) => void;
  }

  const { connectionId, onclose, onimported }: Props = $props();

  type Step = 'pick' | 'preview' | 'importing' | 'done';
  let step = $state<Step>('pick');
  let filePath = $state('');
  let preview = $state<importApi.CsvPreview | null>(null);
  let columns = $state<InferredColumn[]>([]);
  let tableName = $state('');
  let createTable = $state(true);
  let importedCount = $state(0);
  let error = $state<string | null>(null);
  let loading = $state(false);

  const DB_TYPES = ['TEXT', 'INTEGER', 'REAL', 'BLOB', 'BOOLEAN', 'DATE', 'TIMESTAMP', 'VARCHAR(255)'];

  async function pickFile() {
    const selected = await openFileDialog({
      multiple: false,
      filters: [{ name: 'CSV Files', extensions: ['csv'] }],
    });
    if (typeof selected === 'string' && selected) {
      filePath = selected;
      await loadPreview();
    }
  }

  async function loadPreview() {
    if (!filePath) return;
    loading = true;
    error = null;
    try {
      preview = await importApi.importCsvPreview(filePath);
      columns = preview.columns.map((c) => ({
        name: c.name,
        inferredType: c.inferredType,
      }));
      // Default table name from file name.
      const parts = filePath.replace(/\\/g, '/').split('/');
      const fname = parts[parts.length - 1] ?? '';
      tableName = fname.replace(/\.csv$/i, '').replace(/[^a-zA-Z0-9_]/g, '_');
      step = 'preview';
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      loading = false;
    }
  }

  function typeForInferred(t: InferredColumn['inferredType']): string {
    switch (t) {
      case 'integer': return 'INTEGER';
      case 'float': return 'REAL';
      case 'date': return 'DATE';
      default: return 'TEXT';
    }
  }

  async function runImport() {
    if (!filePath || !tableName.trim()) return;
    step = 'importing';
    error = null;

    try {
      const overrides: importApi.ColumnOverride[] = columns.map((c) => ({
        name: c.name,
        dbType: typeForInferred(c.inferredType),
      }));

      importedCount = await importApi.importCsvExecute(
        connectionId,
        filePath,
        tableName.trim(),
        createTable,
        overrides,
      );
      step = 'done';
      onimported?.(importedCount);
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
      step = 'preview';
    }
  }

  function handleBackdrop(e: MouseEvent) {
    if (e.target === e.currentTarget) onclose();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="backdrop" role="dialog" aria-modal="true" aria-label="Import CSV" onclick={handleBackdrop}>
  <div class="modal">
    <header class="modal-header">
      <h2 class="modal-title">Import CSV</h2>
      <button class="close-btn" onclick={onclose} aria-label="Close">✕</button>
    </header>

    <div class="modal-body">
      {#if step === 'pick'}
        <div class="pick-step">
          <p class="pick-hint">Select a CSV file to import.</p>
          <button class="btn btn--primary" onclick={pickFile} disabled={loading}>
            {loading ? 'Loading…' : 'Choose File'}
          </button>
          {#if error}
            <div class="error-msg">{error}</div>
          {/if}
        </div>

      {:else if step === 'preview' && preview}
        <div class="preview-step">
          <div class="file-row">
            <span class="file-path">{filePath}</span>
            <button class="btn btn--ghost btn--sm" onclick={pickFile}>Change</button>
          </div>

          <div class="table-settings">
            <div class="setting-row">
              <label class="label" for="import-table-name">Table Name</label>
              <input id="import-table-name" class="input" type="text" bind:value={tableName} />
            </div>
            <div class="setting-row setting-row--inline">
              <label class="label" for="import-create">Create new table</label>
              <input id="import-create" type="checkbox" bind:checked={createTable} />
            </div>
          </div>

          <h3 class="section-label">Column Mapping ({columns.length} columns)</h3>
          <div class="col-table-wrap">
            <table class="col-table">
              <thead>
                <tr>
                  <th>Column Name</th>
                  <th>Type</th>
                  <th>Sample</th>
                </tr>
              </thead>
              <tbody>
                {#each columns as col, i}
                  <tr>
                    <td>
                      <input
                        class="col-name-input"
                        type="text"
                        bind:value={col.name}
                        aria-label="Column {i + 1} name"
                      />
                    </td>
                    <td>
                      <select class="col-type-select" bind:value={col.inferredType} aria-label="Column {i + 1} type">
                        {#each DB_TYPES as t}
                          <option value={t.toLowerCase()}>{t}</option>
                        {/each}
                      </select>
                    </td>
                    <td class="sample-cell">
                      {preview.previewRows[0]?.[i] ?? ''}
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>

          <p class="preview-hint">Showing first {Math.min(preview.previewRows.length, 20)} rows</p>

          {#if error}
            <div class="error-msg">{error}</div>
          {/if}
        </div>

      {:else if step === 'importing'}
        <div class="importing-step">
          <div class="spinner" aria-label="Importing…"></div>
          <p>Importing rows…</p>
        </div>

      {:else if step === 'done'}
        <div class="done-step">
          <span class="success-icon" aria-hidden="true">✓</span>
          <p>Successfully imported <strong>{importedCount}</strong> rows into <code>{tableName}</code>.</p>
        </div>
      {/if}
    </div>

    <footer class="modal-footer">
      {#if step === 'preview'}
        <button class="btn btn--ghost" onclick={onclose}>Cancel</button>
        <button
          class="btn btn--primary"
          onclick={runImport}
          disabled={!tableName.trim()}
        >
          Import
        </button>
      {:else if step === 'done'}
        <button class="btn btn--primary" onclick={onclose}>Close</button>
      {:else if step !== 'importing'}
        <button class="btn btn--ghost" onclick={onclose}>Cancel</button>
      {/if}
    </footer>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 300;
  }

  .modal {
    background: var(--color-bg-overlay);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    width: 640px;
    max-width: calc(100vw - var(--spacing-8));
    max-height: calc(100vh - var(--spacing-8));
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .modal-header {
    display: flex;
    align-items: center;
    padding: var(--spacing-4) var(--spacing-5);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .modal-title {
    flex: 1;
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
  }

  .close-btn {
    color: var(--color-text-muted);
    font-size: var(--font-size-md);
    padding: var(--spacing-1);
    border-radius: var(--radius-sm);
    transition: color var(--transition-fast), background var(--transition-fast);
  }

  .close-btn:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .modal-body {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-5);
  }

  .modal-footer {
    padding: var(--spacing-4) var(--spacing-5);
    border-top: 1px solid var(--color-border);
    display: flex;
    justify-content: flex-end;
    gap: var(--spacing-2);
    flex-shrink: 0;
  }

  .pick-step {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--spacing-4);
    padding: var(--spacing-8) 0;
  }

  .pick-hint {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .preview-step {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-3);
  }

  .file-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-3);
    background: var(--color-bg-secondary);
    border-radius: var(--radius-md);
    border: 1px solid var(--color-border);
  }

  .file-path {
    flex: 1;
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .table-settings {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
  }

  .setting-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
  }

  .setting-row--inline {
    justify-content: space-between;
  }

  .label {
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-secondary);
    white-space: nowrap;
  }

  .input {
    flex: 1;
    height: 28px;
    padding: 0 var(--spacing-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-primary);
    color: var(--color-text-primary);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    outline: none;
  }

  .input:focus { border-color: var(--color-accent); }

  .section-label {
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .col-table-wrap {
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    overflow-y: auto;
    max-height: 280px;
  }

  .col-table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--font-size-sm);
  }

  .col-table th {
    text-align: left;
    padding: var(--spacing-2) var(--spacing-3);
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
    position: sticky;
    top: 0;
  }

  .col-table td {
    padding: var(--spacing-1) var(--spacing-2);
    border-bottom: 1px solid var(--color-border);
    vertical-align: middle;
  }

  .col-name-input {
    width: 100%;
    padding: 2px var(--spacing-2);
    background: var(--color-bg-primary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    color: var(--color-text-primary);
    outline: none;
    font-family: var(--font-family-mono);
  }

  .col-type-select {
    padding: 2px var(--spacing-2);
    background: var(--color-bg-primary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    color: var(--color-text-primary);
    outline: none;
  }

  .sample-cell {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    max-width: 120px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .preview-hint {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
  }

  .importing-step {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--spacing-4);
    padding: var(--spacing-8) 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .spinner {
    width: 32px;
    height: 32px;
    border: 3px solid var(--color-border);
    border-top-color: var(--color-accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  .done-step {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--spacing-4);
    padding: var(--spacing-8) 0;
    text-align: center;
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
  }

  .success-icon {
    font-size: 40px;
    color: var(--color-success);
  }

  .error-msg {
    padding: var(--spacing-2) var(--spacing-3);
    background: var(--color-danger-subtle);
    color: var(--color-danger);
    border-radius: var(--radius-md);
    font-size: var(--font-size-sm);
  }

  .btn {
    height: 32px;
    padding: 0 var(--spacing-3);
    border-radius: var(--radius-md);
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    font-family: var(--font-family-ui);
    cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
    white-space: nowrap;
  }

  .btn--sm {
    height: 26px;
    padding: 0 var(--spacing-2);
    font-size: var(--font-size-xs);
  }

  .btn:disabled { opacity: 0.5; cursor: not-allowed; }

  .btn--primary {
    background: var(--color-accent);
    color: var(--color-text-on-accent);
    border: 1px solid transparent;
  }

  .btn--primary:not(:disabled):hover { background: var(--color-accent-hover); }

  .btn--ghost {
    background: transparent;
    color: var(--color-text-secondary);
    border: 1px solid var(--color-border);
  }

  .btn--ghost:not(:disabled):hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }
</style>
