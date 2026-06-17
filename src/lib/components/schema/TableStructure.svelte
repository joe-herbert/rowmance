<!--
  TableStructure — panel showing columns, indexes, and foreign keys for a table.
-->
<script lang="ts">
  import * as schemaApi from '$lib/tauri/schema';
  import type { ColumnInfo, IndexInfo, ForeignKeyInfo } from '$lib/types';
  import { errorMessage } from '$lib/utils/errors';

  interface Props {
    connectionId: string;
    database: string;
    table: string;
  }

  const { connectionId, database, table }: Props = $props();

  let isLoading = $state(true);
  let loadError = $state<string | null>(null);
  let columns = $state<ColumnInfo[]>([]);
  let indexes = $state<IndexInfo[]>([]);
  let foreignKeys = $state<ForeignKeyInfo[]>([]);

  $effect(() => {
    isLoading = true;
    loadError = null;
    Promise.all([
      schemaApi.listColumns(connectionId, database, table),
      schemaApi.listIndexes(connectionId, database, table),
      schemaApi.listForeignKeys(connectionId, database, table),
    ]).then(([cols, idxs, fks]) => {
      columns = cols;
      indexes = idxs;
      foreignKeys = fks;
      isLoading = false;
    }).catch((err) => {
      loadError = errorMessage(err);
      isLoading = false;
    });
  });
</script>

<div class="structure-viewer">
  <div class="toolbar">
    <span class="object-label">
      <span class="object-type">table</span>
      <span class="object-type-sep">/</span>
      <span class="object-path">{database}.{table}</span>
    </span>
    {#if !isLoading && !loadError}
      <span class="col-count">{columns.length} columns</span>
    {/if}
  </div>

  <div class="content">
    {#if isLoading}
      <div class="state-overlay">
        <span class="loading-text">Loading…</span>
      </div>
    {:else if loadError}
      <div class="state-overlay state-overlay--error">
        <span class="error-text">{loadError}</span>
      </div>
    {:else}
      <div class="sections">
        <!-- Columns -->
        <section class="section">
          <table class="col-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Type</th>
                <th class="th-narrow">Key</th>
                <th class="th-narrow" title="Nullable">Null</th>
                <th>Default</th>
              </tr>
            </thead>
            <tbody>
              {#each columns as col (col.name)}
                <tr class:pk-row={col.isPrimaryKey}>
                  <td class="col-name mono">{col.name}</td>
                  <td class="col-type mono">{col.dataType}</td>
                  <td class="col-keys">
                    {#if col.isPrimaryKey}<span class="badge badge--pk">PK</span>{/if}
                    {#if col.isAutoIncrement}<span class="badge badge--ai">AI</span>{/if}
                    {#if col.isForeignKey}<span class="badge badge--fk">FK</span>{/if}
                  </td>
                  <td class="col-null center-cell">{col.nullable ? '✓' : ''}</td>
                  <td class="col-default mono">{col.defaultValue ?? ''}</td>
                </tr>
                {#if col.comment}
                  <tr class="comment-row">
                    <td colspan="5" class="col-comment">{col.comment}</td>
                  </tr>
                {/if}
              {/each}
            </tbody>
          </table>
        </section>

        <!-- Indexes -->
        {#if indexes.length > 0}
          <section class="section">
            <div class="section-header">Indexes ({indexes.length})</div>
            <div class="index-list">
              {#each indexes as idx (idx.name)}
                <div class="index-row">
                  <div class="index-main">
                    <span class="index-name mono">{idx.name}</span>
                    <span class="index-cols mono">{idx.columns.join(', ')}</span>
                  </div>
                  <div class="index-meta">
                    {#if idx.unique}<span class="badge badge--unique">UNIQUE</span>{/if}
                    <span class="index-type">{idx.indexType}</span>
                  </div>
                </div>
              {/each}
            </div>
          </section>
        {/if}

        <!-- Foreign Keys -->
        {#if foreignKeys.length > 0}
          <section class="section">
            <div class="section-header">Foreign Keys ({foreignKeys.length})</div>
            <div class="fk-list">
              {#each foreignKeys as fk (fk.constraintName)}
                <div class="fk-card">
                  <div class="fk-name mono">{fk.constraintName}</div>
                  <div class="fk-relation">
                    <span class="mono fk-cols">{fk.columns.join(', ')}</span>
                    <span class="fk-arrow">→</span>
                    <span class="mono fk-ref">{fk.referencedTable}.{fk.referencedColumns.join(', ')}</span>
                  </div>
                  <div class="fk-actions">
                    <span class="fk-action-label">ON DELETE</span> {fk.onDelete}
                    <span class="fk-sep">·</span>
                    <span class="fk-action-label">ON UPDATE</span> {fk.onUpdate}
                  </div>
                </div>
              {/each}
            </div>
          </section>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .structure-viewer {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: var(--color-bg-primary);
  }

  /* ── Toolbar ───────────────────────────────────────────────────────────── */

  .toolbar {
    flex-shrink: 0;
    height: var(--toolbar-height);
    min-height: var(--toolbar-height);
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: 0 var(--spacing-3);
    background: var(--color-bg-secondary);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border-bottom: 1px solid var(--color-border);
  }

  .object-label {
    display: flex;
    align-items: center;
    gap: 6px;
    overflow: hidden;
    min-width: 0;
  }

  .object-type {
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-disabled);
    text-transform: uppercase;
    letter-spacing: 0.07em;
    flex-shrink: 0;
  }

  .object-type-sep {
    color: var(--color-border-strong);
    flex-shrink: 0;
    font-size: var(--font-size-xs);
  }

  .object-path {
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    font-family: var(--font-family-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .col-count {
    margin-left: auto;
    font-size: 10px;
    color: var(--color-text-muted);
    flex-shrink: 0;
    background: var(--color-bg-tertiary, var(--color-bg-hover));
    padding: 2px 7px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--color-border);
    letter-spacing: 0.02em;
  }

  /* ── Scroll content ────────────────────────────────────────────────────── */

  .content {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
  }

  .sections {
    display: flex;
    flex-direction: column;
    padding-bottom: var(--spacing-4);
  }

  .section {
    border-bottom: 1px solid var(--color-border);
  }

  .section:last-child {
    border-bottom: none;
  }

  .section-header {
    padding: 6px var(--spacing-3);
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    background: var(--color-bg-secondary);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border-bottom: 1px solid var(--color-border);
    position: sticky;
    top: 0;
    z-index: 1;
  }

  /* ── Columns table ─────────────────────────────────────────────────────── */

  .col-table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--font-size-xs);
  }

  .col-table thead {
    position: sticky;
    top: 0;
    z-index: 2;
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
  }

  .col-table thead tr {
    background: var(--color-table-header-bg);
  }

  .col-table th {
    padding: 5px var(--spacing-3);
    text-align: left;
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    border-bottom: 1px solid var(--color-border-strong);
    white-space: nowrap;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  .th-narrow {
    width: 1px;
    text-align: center;
    white-space: nowrap;
  }

  .col-table td {
    padding: 7px var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
    vertical-align: middle;
    transition: background var(--transition-fast);
  }

  .col-table tbody tr:last-child td {
    border-bottom: none;
  }

  .col-table tbody tr:hover td {
    background: var(--color-table-row-hover);
  }

  .pk-row .col-name {
    color: var(--color-accent);
  }

  .col-name {
    color: var(--color-text-primary);
    font-weight: var(--font-weight-medium);
    white-space: nowrap;
  }

  .col-type {
    color: var(--color-text-secondary);
    white-space: nowrap;
  }

  .col-keys {
    white-space: nowrap;
    text-align: center;
  }

  .col-null {
    text-align: center;
    color: var(--color-success);
  }

  .col-default {
    color: var(--color-text-muted);
    max-width: 120px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .comment-row td {
    padding: 0 var(--spacing-3) 6px;
    font-size: 11px;
    color: var(--color-text-muted);
    font-style: italic;
    border-bottom: 1px solid var(--color-border);
  }

  .mono {
    font-family: var(--font-family-mono);
  }

  /* ── Badges ────────────────────────────────────────────────────────────── */

  .badge {
    display: inline-block;
    font-size: 9px;
    font-weight: var(--font-weight-semibold);
    padding: 1px 5px;
    border-radius: var(--radius-sm);
    letter-spacing: 0.04em;
    margin-right: 2px;
    border: 1px solid transparent;
  }

  .badge--pk {
    background: var(--color-accent-subtle);
    color: var(--color-accent);
    border-color: rgba(124, 92, 255, 0.22);
  }

  .badge--ai {
    background: var(--color-bg-tertiary, var(--color-bg-hover));
    color: var(--color-text-muted);
    border-color: var(--color-border);
  }

  .badge--fk {
    background: var(--color-success-subtle);
    color: var(--color-success);
    border-color: rgba(22, 163, 74, 0.22);
  }

  .badge--unique {
    background: var(--color-success-subtle);
    color: var(--color-success);
    border-color: rgba(22, 163, 74, 0.22);
    font-size: 9px;
  }

  /* ── Indexes ───────────────────────────────────────────────────────────── */

  .index-list {
    padding: 0;
  }

  .index-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: 7px var(--spacing-3);
    font-size: var(--font-size-xs);
    border-bottom: 1px solid var(--color-border);
    transition: background var(--transition-fast);
  }

  .index-row:last-child {
    border-bottom: none;
  }

  .index-row:hover {
    background: var(--color-table-row-hover);
  }

  .index-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .index-name {
    color: var(--color-text-primary);
    font-weight: var(--font-weight-medium);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .index-cols {
    color: var(--color-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 10px;
  }

  .index-meta {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    flex-shrink: 0;
  }

  .index-type {
    font-size: 10px;
    color: var(--color-text-muted);
    background: var(--color-bg-tertiary, var(--color-bg-hover));
    padding: 1px 5px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--color-border);
    letter-spacing: 0.03em;
    text-transform: uppercase;
  }

  /* ── Foreign Keys ──────────────────────────────────────────────────────── */

  .fk-list {
    padding: var(--spacing-2) var(--spacing-3);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
  }

  .fk-card {
    padding: var(--spacing-2) var(--spacing-3);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg-secondary);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    box-shadow: var(--shadow-sm);
    font-size: var(--font-size-xs);
    transition: box-shadow var(--transition-fast), border-color var(--transition-fast);
  }

  .fk-card:hover {
    border-color: var(--color-border-strong);
    box-shadow: var(--shadow-md);
  }

  .fk-name {
    color: var(--color-text-muted);
    font-weight: var(--font-weight-medium);
    margin-bottom: 5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 10px;
    letter-spacing: 0.02em;
    font-family: var(--font-family-mono);
  }

  .fk-relation {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    margin-bottom: 6px;
    flex-wrap: wrap;
    font-family: var(--font-family-mono);
  }

  .fk-cols {
    color: var(--color-text-primary);
    font-weight: var(--font-weight-medium);
  }

  .fk-arrow {
    color: var(--color-accent);
    flex-shrink: 0;
    opacity: 0.6;
  }

  .fk-ref {
    color: var(--color-accent);
  }

  .fk-actions {
    font-size: 10px;
    color: var(--color-text-muted);
    padding-top: 5px;
    border-top: 1px solid var(--color-border);
    display: flex;
    align-items: center;
    gap: 3px;
    flex-wrap: wrap;
  }

  .fk-action-label {
    font-weight: var(--font-weight-semibold);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    font-size: 9px;
    color: var(--color-text-muted);
  }

  .fk-sep {
    color: var(--color-border-strong);
    margin: 0 5px;
  }

  /* ── State overlays ────────────────────────────────────────────────────── */

  .state-overlay {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: var(--spacing-8) var(--spacing-4);
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    height: 200px;
  }

  .state-overlay--error {
    color: var(--color-danger);
  }

  .loading-text {
    font-style: italic;
    animation: pulse 1.4s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.35; }
  }
</style>
