<!--
  RelationsPanel — shows foreign-key relations for the currently focused table cell.
  Forward: FKs FROM the current table that include the selected column (→ referenced table).
  Reverse: FKs FROM other tables that point TO the current table's selected column (← referencing tables).
  Uses erd_get_graph to load all FK edges for the database, then filters client-side.
-->
<script lang="ts">
  import { useCellSelection, type CellSelection } from '$lib/stores/cellSelection.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { getErdGraph } from '$lib/tauri/erd';
  import { executeQuery } from '$lib/tauri/query';
  import type { ErdRelation, DbType } from '$lib/types';
  import { errorMessage } from '$lib/utils/errors';

  type CellValue = string | number | boolean | null;

  interface RelationEntry {
    edge: ErdRelation;
    direction: 'forward' | 'reverse';
    targetTable: string;
    filterColumn: string;
    filterValue: CellValue;
    rows: CellValue[][];
    columnNames: string[];
    loading: boolean;
    error: string | null;
  }

  const cellSelectionStore = useCellSelection();
  const connectionStore = useConnections();
  const panelStore = usePanels();

  let relations = $state<RelationEntry[]>([]);
  let globalLoading = $state(false);
  let globalError = $state<string | null>(null);

  function quoteId(name: string, dbType: DbType): string {
    return dbType === 'postgres' ? `"${name.replace(/"/g, '""')}"` : `\`${name.replace(/`/g, '``')}\``;
  }

  function escapeStr(val: string): string {
    return val.replace(/'/g, "''");
  }

  function valueLiteral(val: CellValue): string {
    if (val === null) return 'NULL';
    if (typeof val === 'boolean') return val ? '1' : '0';
    if (typeof val === 'number') return String(val);
    return `'${escapeStr(String(val))}'`;
  }

  async function loadRelations(sel: CellSelection) {
    const dbType = connectionStore.getById(sel.connectionId)?.dbType ?? 'mysql';
    globalLoading = true;
    globalError = null;
    relations = [];

    try {
      const graph = await getErdGraph(sel.connectionId, sel.database);

      const entries: RelationEntry[] = [];

      // Forward: edges where this table is the source and selected column is a FK column
      for (const edge of graph.edges) {
        if (edge.fromTable === sel.table && edge.fromColumns.includes(sel.columnName)) {
          // Resolve the value for each FK column (may be composite)
          const colIdx = edge.fromColumns.indexOf(sel.columnName);
          const refCol = edge.toColumns[colIdx] ?? edge.toColumns[0];
          entries.push({
            edge,
            direction: 'forward',
            targetTable: edge.toTable,
            filterColumn: refCol,
            filterValue: sel.cellValue,
            rows: [],
            columnNames: [],
            loading: true,
            error: null,
          });
        }
      }

      // Reverse: edges where this table is the target and selected column is a referenced column
      for (const edge of graph.edges) {
        if (edge.toTable === sel.table && edge.toColumns.includes(sel.columnName)) {
          const colIdx = edge.toColumns.indexOf(sel.columnName);
          const fkCol = edge.fromColumns[colIdx] ?? edge.fromColumns[0];
          entries.push({
            edge,
            direction: 'reverse',
            targetTable: edge.fromTable,
            filterColumn: fkCol,
            filterValue: sel.cellValue,
            rows: [],
            columnNames: [],
            loading: true,
            error: null,
          });
        }
      }

      relations = entries;
      globalLoading = false;

      // Fetch rows for each relation concurrently
      await Promise.all(
        entries.map(async (_, i) => {
          const rel = relations[i];
          try {
            const sql = `SELECT * FROM ${quoteId(sel.database, dbType)}.${quoteId(rel.targetTable, dbType)} WHERE ${quoteId(rel.filterColumn, dbType)} = ${valueLiteral(rel.filterValue)}`;
            const result = await executeQuery(sel.connectionId, sql, 1, rel.direction === 'forward' ? 20 : 10);
            if (result.error) {
              relations[i].loading = false;
              relations[i].error = result.error;
            } else {
              relations[i].loading = false;
              relations[i].rows = result.rows;
              relations[i].columnNames = result.columns.map((c) => c.name);
            }
          } catch (err) {
            relations[i].loading = false;
            relations[i].error = errorMessage(err);
          }
        }),
      );
    } catch (err) {
      globalLoading = false;
      globalError = errorMessage(err);
    }
  }

  function openRelation(sel: CellSelection, rel: RelationEntry) {
    const dbType = connectionStore.getById(sel.connectionId)?.dbType ?? 'mysql';
    panelStore.openInFocused({
      kind: 'table_browser',
      connectionId: sel.connectionId,
      database: sel.database,
      table: rel.targetTable,
      initialFilter: `${quoteId(rel.filterColumn, dbType)} = ${valueLiteral(rel.filterValue)}`,
    });
  }

  function formatValue(val: CellValue): string {
    if (val === null) return 'NULL';
    if (val === '') return 'EMPTY';
    return String(val);
  }

  // Debounced reload when the selected cell changes
  $effect(() => {
    const sel = cellSelectionStore.current;

    const timer = setTimeout(() => {
      if (sel && sel.cellValue !== null) {
        loadRelations(sel);
      } else {
        relations = [];
        globalLoading = false;
        globalError = null;
      }
    }, 200);

    return () => clearTimeout(timer);
  });

  const sel = $derived(cellSelectionStore.current);
  const forwardRelations = $derived(relations.filter((r) => r.direction === 'forward'));
  const reverseRelations = $derived(relations.filter((r) => r.direction === 'reverse'));
  const MAX_COLS = 3;
</script>

<div class="relations-panel">
  {#if !sel}
    <div class="empty-state">
      <p>Select a cell in a table to see its relations.</p>
    </div>
  {:else if sel.cellValue === null}
    <div class="empty-state">
      <p>Cell is NULL — no relations to show.</p>
    </div>
  {:else}
    <div class="context-bar">
      <span class="context-table">{sel.table}</span><span class="context-dot">.</span><span
        class="context-col">{sel.columnName}</span
      ><span class="context-eq">&nbsp;=&nbsp;</span><span class="context-val"
        >{formatValue(sel.cellValue)}</span
      >
    </div>

    {#if globalLoading}
      <div class="state-row">Loading…</div>
    {:else if globalError}
      <div class="error-row">{globalError}</div>
    {:else if relations.length === 0}
      <div class="empty-state">
        <p>No foreign key relations found for this column.</p>
      </div>
    {:else}
      {#if forwardRelations.length > 0}
        <div class="section-header">
          <span class="section-arrow">↗</span>
          <span class="section-label">REFERENCES</span>
        </div>
        {#each forwardRelations as rel (rel.edge.constraintName)}
          {@const previewCols = rel.columnNames.slice(0, MAX_COLS)}
          <div class="relation-card">
            <div class="relation-title">
              <span class="rel-table">{rel.targetTable}</span>
              <span class="rel-via">via {rel.edge.fromColumns.join(', ')} → {rel.edge.toColumns.join(', ')}</span>
            </div>
            {#if rel.loading}
              <div class="rel-state">Loading…</div>
            {:else if rel.error}
              <div class="rel-error">{rel.error}</div>
            {:else if rel.rows.length === 0}
              <div class="rel-state">No matching row found.</div>
            {:else}
              <div class="mini-table-wrap">
                <table class="mini-table">
                  <thead>
                    <tr>
                      {#each previewCols as col}<th class="mini-th" title={col}>{col}</th>{/each}
                      {#if rel.columnNames.length > MAX_COLS}<th class="mini-th extra">+{rel.columnNames.length - MAX_COLS}</th>{/if}
                    </tr>
                  </thead>
                  <tbody>
                    {#each rel.rows as row, ri (ri)}
                      <tr class="mini-row">
                        {#each previewCols as _col, ci (ci)}
                          <td class="mini-td" title={formatValue(row[ci])}>
                            {#if row[ci] === null}<span class="null-val">NULL</span>{:else}{formatValue(row[ci])}{/if}
                          </td>
                        {/each}
                        {#if rel.columnNames.length > MAX_COLS}<td class="mini-td extra"></td>{/if}
                      </tr>
                    {/each}
                  </tbody>
                </table>
              </div>
            {/if}
            <button class="open-btn" onclick={() => openRelation(sel, rel)}>
              Open {rel.targetTable} ›
            </button>
          </div>
        {/each}
      {/if}

      {#if reverseRelations.length > 0}
        <div class="section-header">
          <span class="section-arrow">↙</span>
          <span class="section-label">REFERENCED BY</span>
        </div>
        {#each reverseRelations as rel (rel.edge.constraintName + rel.targetTable)}
          {@const previewCols = rel.columnNames.slice(0, MAX_COLS)}
          <div class="relation-card">
            <div class="relation-title">
              <span class="rel-table">{rel.targetTable}</span>
              <span class="rel-via">via {rel.filterColumn}</span>
            </div>
            {#if rel.loading}
              <div class="rel-state">Loading…</div>
            {:else if rel.error}
              <div class="rel-error">{rel.error}</div>
            {:else if rel.rows.length === 0}
              <div class="rel-state">No referencing rows found.</div>
            {:else}
              <div class="row-count-hint">{rel.rows.length} row{rel.rows.length !== 1 ? 's' : ''} shown</div>
              <div class="mini-table-wrap">
                <table class="mini-table">
                  <thead>
                    <tr>
                      {#each previewCols as col}<th class="mini-th" title={col}>{col}</th>{/each}
                      {#if rel.columnNames.length > MAX_COLS}<th class="mini-th extra">+{rel.columnNames.length - MAX_COLS}</th>{/if}
                    </tr>
                  </thead>
                  <tbody>
                    {#each rel.rows as row, ri (ri)}
                      <tr class="mini-row">
                        {#each previewCols as _col, ci (ci)}
                          <td class="mini-td" title={formatValue(row[ci])}>
                            {#if row[ci] === null}<span class="null-val">NULL</span>{:else}{formatValue(row[ci])}{/if}
                          </td>
                        {/each}
                        {#if rel.columnNames.length > MAX_COLS}<td class="mini-td extra"></td>{/if}
                      </tr>
                    {/each}
                  </tbody>
                </table>
              </div>
            {/if}
            <button class="open-btn" onclick={() => openRelation(sel, rel)}>
              Open {rel.targetTable} ›
            </button>
          </div>
        {/each}
      {/if}
    {/if}
  {/if}
</div>

<style>
  .relations-panel {
    display: flex;
    flex-direction: column;
    flex: 1;
    overflow-y: auto;
    min-height: 0;
  }

  .empty-state {
    padding: var(--spacing-3);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
    line-height: var(--line-height-normal);
  }

  .context-bar {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    padding: var(--spacing-2);
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .context-table {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-secondary);
    font-weight: var(--font-weight-medium);
  }

  .context-dot {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  .context-col {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-accent);
    font-weight: var(--font-weight-medium);
  }

  .context-eq {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  .context-val {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-primary);
    max-width: 120px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .state-row {
    padding: var(--spacing-3) var(--spacing-2);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
  }

  .error-row {
    margin: var(--spacing-2);
    padding: var(--spacing-2);
    font-size: var(--font-size-xs);
    color: var(--color-danger);
    background: var(--color-danger-subtle);
    border-radius: var(--radius-sm);
  }

  .section-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: var(--spacing-2) var(--spacing-2) var(--spacing-1);
  }

  .section-arrow {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
  }

  .section-label {
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.07em;
  }

  .relation-card {
    margin: 0 var(--spacing-2) var(--spacing-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    overflow: hidden;
    background: var(--color-bg-primary);
  }

  .relation-title {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: var(--spacing-1) var(--spacing-2);
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
  }

  .rel-table {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-primary);
  }

  .rel-via {
    font-size: 10px;
    color: var(--color-text-muted);
    font-family: var(--font-family-mono);
  }

  .rel-state {
    padding: var(--spacing-2);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
  }

  .rel-error {
    padding: var(--spacing-2);
    font-size: var(--font-size-xs);
    color: var(--color-danger);
  }

  .row-count-hint {
    padding: 2px var(--spacing-2);
    font-size: 10px;
    color: var(--color-text-muted);
    font-style: italic;
  }

  .mini-table-wrap {
    overflow-x: auto;
    border-bottom: 1px solid var(--color-border);
  }

  .mini-table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--font-size-xs);
  }

  .mini-th {
    padding: 3px var(--spacing-1);
    background: var(--color-table-header-bg);
    color: var(--color-text-muted);
    font-weight: var(--font-weight-medium);
    font-size: 10px;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 80px;
    border-right: 1px solid var(--color-border);
  }

  .mini-th:last-child {
    border-right: none;
  }

  .mini-th.extra {
    color: var(--color-text-muted);
    font-style: italic;
    font-weight: normal;
  }

  .mini-row {
    border-bottom: 1px solid var(--color-border);
  }

  .mini-row:last-child {
    border-bottom: none;
  }

  .mini-td {
    padding: 3px var(--spacing-1);
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 80px;
    border-right: 1px solid var(--color-border);
    vertical-align: middle;
  }

  .mini-td:last-child {
    border-right: none;
  }

  .mini-td.extra {
    color: var(--color-text-muted);
  }

  .null-val {
    color: var(--color-null);
    font-style: italic;
    font-size: 10px;
  }

  .open-btn {
    display: block;
    width: 100%;
    padding: var(--spacing-1) var(--spacing-2);
    font-size: var(--font-size-xs);
    color: var(--color-accent);
    text-align: right;
    cursor: pointer;
    background: transparent;
    border: none;
    transition: background var(--transition-fast);
  }

  .open-btn:hover {
    background: var(--color-accent-subtle);
  }
</style>
