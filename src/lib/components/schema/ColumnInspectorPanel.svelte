<script lang="ts">
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useCellSelection } from '$lib/stores/cellSelection.svelte';
  import * as schemaApi from '$lib/tauri/schema';
  import { getErdGraph } from '$lib/tauri/erd';
  import type { ColumnInfo, IndexInfo, ForeignKeyInfo, ErdRelation } from '$lib/types';
  import { errorMessage } from '$lib/utils/errors';
  import { useToast } from '$lib/stores/toast.svelte';

  const panelStore = usePanels();
  const cellSelectionStore = useCellSelection();
  const toast = useToast();

  let columnInfoLoading = $state(false);
  let columnInfoData = $state<ColumnInfo | null>(null);
  let columnIndexes = $state<IndexInfo[]>([]);
  let columnForeignKeys = $state<ForeignKeyInfo[]>([]);
  let columnReferencedBy = $state<ErdRelation[]>([]);
  let columnInspectorKey = $state<string | null>(null);

  $effect(() => {
    const sel = cellSelectionStore.current;
    const key =
      sel && sel.columnName
        ? `${sel.connectionId}:${sel.database}:${sel.table}:${sel.columnName}`
        : null;
    if (key === columnInspectorKey) return;
    columnInspectorKey = key;

    if (!sel || !sel.columnName) {
      columnInfoData = null;
      columnIndexes = [];
      columnForeignKeys = [];
      columnReferencedBy = [];
      return;
    }

    const columnName = sel.columnName;
    columnInfoLoading = true;
    Promise.all([
      schemaApi.listColumns(sel.connectionId, sel.database, sel.table),
      schemaApi.listIndexes(sel.connectionId, sel.database, sel.table),
      schemaApi.listForeignKeys(sel.connectionId, sel.database, sel.table),
      getErdGraph(sel.connectionId, sel.database),
    ])
      .then(([cols, idxs, fks, graph]) => {
        columnInfoData = cols.find((c) => c.name === columnName) ?? null;
        columnIndexes = idxs.filter((idx) => idx.columns.includes(columnName));
        columnForeignKeys = fks.filter((fk) => fk.columns.includes(columnName));
        columnReferencedBy = graph.edges.filter(
          (edge) => edge.toTable === sel.table && edge.toColumns.includes(columnName),
        );
        columnInfoLoading = false;
      })
      .catch((err) => {
        toast.addToast(errorMessage(err), 'error', 0);
        columnInfoLoading = false;
      });
  });

  function openReferencingTable(edge: ErdRelation) {
    const sel = cellSelectionStore.current;
    if (!sel) return;
    panelStore.openInFocused({
      kind: 'table_browser',
      connectionId: sel.connectionId,
      database: sel.database,
      table: edge.fromTable,
    });
  }
</script>

<div id="panel-column" role="tabpanel" aria-label="Column Inspector">
  <div class="panel-toolbar">
    <span class="panel-title">Column Inspector</span>
  </div>
  {#if !cellSelectionStore.current || !cellSelectionStore.current.columnName}
    <div class="placeholder-panel">
      <p>Select a cell in a table to inspect its column.</p>
    </div>
  {:else if columnInfoLoading}
    <div class="loading-row">Loading…</div>
  {:else if !columnInfoData}
    <div class="placeholder-panel"><p>Column not found.</p></div>
  {:else}
    {@const sel = cellSelectionStore.current}
    <div class="context-bar">
      <span class="ctx-table">{sel!.table}</span><span class="ctx-dot">.</span><span class="ctx-col"
        >{columnInfoData.name}</span
      >
    </div>
    <div class="info-section">
      <div class="info-section-title">Properties</div>
      <dl class="info-dl">
        <div class="info-row">
          <dt>Type</dt>
          <dd class="mono">{columnInfoData.dataType}</dd>
        </div>
        <div class="info-row">
          <dt>Nullable</dt>
          <dd>{columnInfoData.nullable ? 'Yes' : 'No'}</dd>
        </div>
        {#if columnInfoData.defaultValue !== null}
          <div class="info-row">
            <dt>Default</dt>
            <dd class="mono">{columnInfoData.defaultValue}</dd>
          </div>
        {/if}
        {#if columnInfoData.isPrimaryKey}
          <div class="info-row">
            <dt>Primary Key</dt>
            <dd class="badge-pk">PK</dd>
          </div>
        {/if}
        {#if columnInfoData.isAutoIncrement}
          <div class="info-row">
            <dt>Auto Increment</dt>
            <dd>Yes</dd>
          </div>
        {/if}
        {#if columnInfoData.isForeignKey}
          <div class="info-row">
            <dt>Foreign Key</dt>
            <dd>Yes</dd>
          </div>
        {/if}
        {#if columnInfoData.comment}
          <div class="info-row">
            <dt>Comment</dt>
            <dd>{columnInfoData.comment}</dd>
          </div>
        {/if}
      </dl>
    </div>
    {#if columnIndexes.length > 0}
      <div class="info-section">
        <div class="info-section-title">Indexes</div>
        {#each columnIndexes as idx (idx.name)}
          <div class="tag-row">
            <span class="tag-name mono">{idx.name}</span>
            {#if idx.unique}<span class="tag-badge">UNIQUE</span>{/if}
            <span class="tag-type">{idx.indexType}</span>
          </div>
        {/each}
      </div>
    {/if}
    {#if columnForeignKeys.length > 0}
      <div class="info-section">
        <div class="info-section-title">Foreign Keys</div>
        {#each columnForeignKeys as fk (fk.constraintName)}
          <div class="fk-card">
            <div class="fk-ref-main">
              <span class="ctx-table">{fk.referencedTable}</span><span class="ctx-dot">.</span><span
                class="ctx-col">{fk.referencedColumns.join(', ')}</span
              >
            </div>
            <div class="fk-name">{fk.constraintName}</div>
            <div class="fk-actions">ON DELETE {fk.onDelete} · ON UPDATE {fk.onUpdate}</div>
          </div>
        {/each}
      </div>
    {/if}
    {#if columnReferencedBy.length > 0}
      <div class="info-section">
        <div class="info-section-title">Referenced By</div>
        {#each columnReferencedBy as edge (edge.constraintName)}
          <button type="button" class="fk-card fk-card-clickable" onclick={() => openReferencingTable(edge)}>
            <div class="fk-ref-main">
              <span class="ctx-table">{edge.fromTable}</span><span class="ctx-dot">.</span><span
                class="ctx-col">{edge.fromColumns.join(', ')}</span
              >
            </div>
            <div class="fk-name">{edge.constraintName}</div>
          </button>
        {/each}
      </div>
    {/if}
  {/if}
</div>

<style>
  .panel-toolbar {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: 6px var(--spacing-2);
    min-height: 34px;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .panel-title {
    font-size: 10.5px;
    font-weight: 700;
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .loading-row {
    padding: var(--spacing-3) var(--spacing-2);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
  }

  .placeholder-panel {
    padding: var(--spacing-3) var(--spacing-3);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
    line-height: var(--line-height-normal);
  }

  .context-bar {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 1px;
    padding: var(--spacing-2);
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .ctx-table {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-secondary);
    font-weight: var(--font-weight-medium);
  }

  .ctx-dot {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  .ctx-col {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-accent);
    font-weight: var(--font-weight-medium);
  }

  .info-section {
    border-bottom: 1px solid var(--color-border);
    padding: var(--spacing-2) 0;
  }

  .info-section-title {
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.07em;
    padding: 0 var(--spacing-2) var(--spacing-1);
  }

  .info-dl {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 0 var(--spacing-2);
  }

  .info-row {
    display: flex;
    align-items: baseline;
    gap: var(--spacing-2);
  }

  .info-row dt {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    min-width: 90px;
    flex-shrink: 0;
  }

  .info-row dd {
    font-size: var(--font-size-xs);
    color: var(--color-text-primary);
  }

  .mono {
    font-family: var(--font-family-mono);
  }

  .badge-pk {
    font-size: 10px;
    background: var(--color-accent-subtle);
    color: var(--color-accent);
    border-radius: var(--radius-sm);
    padding: 1px 5px;
    font-weight: var(--font-weight-semibold);
  }

  .tag-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: 2px var(--spacing-2);
    font-size: var(--font-size-xs);
  }

  .tag-name {
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 120px;
  }

  .tag-badge {
    font-size: 10px;
    background: var(--color-success-subtle);
    color: var(--color-success);
    border-radius: var(--radius-sm);
    padding: 1px 4px;
    font-weight: var(--font-weight-semibold);
    flex-shrink: 0;
  }

  .tag-type {
    font-size: 10px;
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .fk-card {
    margin: 0 var(--spacing-2) var(--spacing-1);
    padding: var(--spacing-1) var(--spacing-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-secondary);
  }

  .fk-name {
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
    font-weight: var(--font-weight-medium);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-style: italic;
  }

  .fk-actions {
    font-size: 10px;
    color: var(--color-text-muted);
    margin-top: 1px;
  }

  .fk-ref-main {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--font-size-sm);
  }

  .fk-card-clickable {
    display: block;
    width: calc(100% - var(--spacing-2) * 2);
    text-align: left;
    cursor: pointer;
    font-family: inherit;
    color: inherit;
  }

  .fk-card-clickable:hover {
    background: var(--color-bg-hover);
  }

  .fk-card-clickable .fk-name {
    margin-top: 1px;
  }
</style>
