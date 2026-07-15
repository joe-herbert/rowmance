<!--
  RelationsPanel — shows foreign-key relations for the currently focused table cell.
  Forward: FKs FROM the current table that include the selected column (→ referenced table).
  Reverse: FKs FROM other tables that point TO the current table's selected column (← referencing tables).
  Also shows user-defined virtual relations that span databases/connections.
-->
<script lang="ts">
  import { useCellSelection, type CellSelection } from '$lib/stores/cellSelection.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { useVirtualRelations } from '$lib/stores/virtualRelations.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { getErdGraph } from '$lib/tauri/erd';
  import { executeQuery } from '$lib/tauri/query';
  import type { ErdRelation, DbType } from '$lib/types';
  import { errorMessage } from '$lib/utils/errors';
  import { useToast } from '$lib/stores/toast.svelte';
  import Loader from '$lib/components/ui/Loader.svelte';
  import { listen } from '@tauri-apps/api/event';
  import { portal } from '$lib/actions/portal';
  import LinkIcon from '$lib/components/icons/LinkIcon.svelte';
  import CloseCircleIcon from '$lib/components/icons/CloseCircleIcon.svelte';
  import ChevronIcon from '$lib/components/icons/ChevronIcon.svelte';
  import TableSmIcon from '$lib/components/icons/TableSmIcon.svelte';
  import OpenInPanelIcon from '$lib/components/icons/OpenInPanelIcon.svelte';
  import RelArrowIcon from '$lib/components/icons/RelArrowIcon.svelte';

  type CellValue = string | number | boolean | null;

  interface RelationEntry {
    edge: ErdRelation | null;
    direction: 'forward' | 'reverse';
    targetTable: string;
    filterColumn: string;
    filterValue: CellValue;
    rows: CellValue[][];
    columnNames: string[];
    totalRows: number | null;
    queryId: string | null;
    loading: boolean;
    error: string | null;
    expanded: boolean;
    virtual?: boolean;
    vrId?: string;
    targetConnectionId?: string;
    targetDatabase?: string;
  }

  const cellSelectionStore = useCellSelection();
  const connectionStore = useConnections();
  const vrStore = useVirtualRelations();
  const panelStore = usePanels();
  const toast = useToast();

  let relations = $state<RelationEntry[]>([]);
  let globalLoading = $state(false);
  let forwardExpanded = $state(true);
  let reverseExpanded = $state(true);

  let panelEl = $state<HTMLElement | null>(null);
  let fwdSpacerEl = $state<HTMLElement | null>(null);
  let revSpacerEl = $state<HTMLElement | null>(null);
  let headerPositions = $state({ panelLeft: 0, panelWidth: 0, fwdTop: 0, revTop: 0 });

  function updateHeaderPositions() {
    if (!panelEl) return;
    const panelRect = panelEl.getBoundingClientRect();
    const fwdRect = fwdSpacerEl?.getBoundingClientRect();
    const revRect = revSpacerEl?.getBoundingClientRect();
    headerPositions = {
      panelLeft: panelRect.left,
      panelWidth: panelRect.width,
      fwdTop: fwdRect ? Math.max(panelRect.top, fwdRect.top) : panelRect.top,
      revTop: revRect ? Math.max(panelRect.top, revRect.top) : panelRect.top,
    };
  }

  $effect(() => {
    if (!panelEl) return;
    panelEl.addEventListener('scroll', updateHeaderPositions, { passive: true });
    const ro = new ResizeObserver(updateHeaderPositions);
    ro.observe(panelEl);
    updateHeaderPositions();
    return () => {
      panelEl.removeEventListener('scroll', updateHeaderPositions);
      ro.disconnect();
    };
  });

  $effect(() => {
    let unlisten: (() => void) | null = null;
    listen<{ queryId: string; totalRows: number }>('query-count-updated', (event) => {
      const { queryId, totalRows } = event.payload;
      const idx = relations.findIndex((r) => r.queryId === queryId);
      if (idx !== -1) relations[idx].totalRows = totalRows;
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  });

  function quoteId(name: string, dbType: DbType): string {
    if (dbType === 'postgres') return `"${name.replace(/"/g, '""')}"`;
    if (dbType === 'sqlserver') return `[${name.replace(/\]/g, ']]')}]`;
    return `\`${name.replace(/`/g, '``')}\``;
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
    relations = [];

    try {
      const graph = await getErdGraph(sel.connectionId, sel.database);
      const entries: RelationEntry[] = [];

      // Forward: edges where this table is the source
      for (const edge of graph.edges) {
        if (edge.fromTable === sel.table && edge.fromColumns.includes(sel.columnName)) {
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
            totalRows: null,
            queryId: null,
            loading: true,
            error: null,
            expanded: true,
          });
        }
      }

      // Reverse: edges where this table is the target
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
            totalRows: null,
            queryId: null,
            loading: true,
            error: null,
            expanded: true,
          });
        }
      }

      // Virtual relations — forward (from.column matches selected column)
      const vrForward = vrStore.forwardFrom({
        connectionId: sel.connectionId,
        database: sel.database,
        table: sel.table,
        column: sel.columnName,
      });
      for (const vr of vrForward) {
        entries.push({
          edge: null,
          direction: 'forward',
          targetTable: vr.to.table,
          filterColumn: vr.to.column,
          filterValue: sel.cellValue,
          rows: [],
          columnNames: [],
          totalRows: null,
          loading: true,
          error: null,
          expanded: true,
          virtual: true,
          vrId: vr.id,
          targetConnectionId: vr.to.connectionId,
          targetDatabase: vr.to.database,
        });
      }

      // Virtual relations — reverse (to.column matches selected column)
      const vrReverse = vrStore.reverseFrom({
        connectionId: sel.connectionId,
        database: sel.database,
        table: sel.table,
        column: sel.columnName,
      });
      for (const vr of vrReverse) {
        entries.push({
          edge: null,
          direction: 'reverse',
          targetTable: vr.from.table,
          filterColumn: vr.from.column,
          filterValue: sel.cellValue,
          rows: [],
          columnNames: [],
          totalRows: null,
          loading: true,
          error: null,
          expanded: true,
          virtual: true,
          vrId: vr.id,
          targetConnectionId: vr.from.connectionId,
          targetDatabase: vr.from.database,
        });
      }

      relations = entries;
      globalLoading = false;

      // Fetch rows for each relation concurrently
      await Promise.all(
        entries.map(async (_, i) => {
          const rel = relations[i];
          const connId = rel.targetConnectionId ?? sel.connectionId;
          const db = rel.targetDatabase ?? sel.database;
          const connDbType = connectionStore.getById(connId)?.dbType ?? dbType;
          try {
            const sql = `SELECT * FROM ${quoteId(db, connDbType)}.${quoteId(rel.targetTable, connDbType)} WHERE ${quoteId(rel.filterColumn, connDbType)} = ${valueLiteral(rel.filterValue)}`;
            const result = await executeQuery(
              connId,
              sql,
              1,
              rel.direction === 'forward' ? 20 : 10,
            );
            if (result.error) {
              relations[i].loading = false;
              toast.addToast(result.error, 'error', 0);
            } else {
              relations[i].loading = false;
              relations[i].rows = result.rows;
              relations[i].columnNames = result.columns.map((c) => c.name);
              relations[i].totalRows = result.totalRows;
              relations[i].queryId = result.queryId;
              if (result.rows.length === 0) relations[i].expanded = false;
            }
          } catch (err) {
            relations[i].loading = false;
            toast.addToast(errorMessage(err), 'error', 0);
          }
        }),
      );
    } catch (err) {
      globalLoading = false;
      toast.addToast(errorMessage(err), 'error', 0);
    }
  }

  async function openRelation(sel: CellSelection, rel: RelationEntry) {
    const connId = rel.targetConnectionId ?? sel.connectionId;
    const db = rel.targetDatabase ?? sel.database;
    const connDbType = connectionStore.getById(connId)?.dbType ?? 'mysql';
    if (rel.virtual && rel.targetConnectionId && !connectionStore.isActive(connId)) {
      await connectionStore.connect(connId);
    }
    panelStore.openInFocused({
      kind: 'table_browser',
      connectionId: connId,
      database: db,
      table: rel.targetTable,
      initialFilter: `${quoteId(rel.filterColumn, connDbType)} = ${valueLiteral(rel.filterValue)}`,
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
    const _vr = vrStore.relations; // track virtual relation changes

    const timer = setTimeout(() => {
      if (sel && sel.columnName && sel.cellValue !== null && sel.cellValue !== undefined) {
        loadRelations(sel);
      } else {
        relations = [];
        globalLoading = false;
      }
    }, 200);

    return () => clearTimeout(timer);
  });

  const sel = $derived(cellSelectionStore.current);
  const forwardRelations = $derived(relations.filter((r) => r.direction === 'forward'));
  const reverseRelations = $derived(relations.filter((r) => r.direction === 'reverse'));

  function getRelKey(rel: RelationEntry, i: number): string {
    if (rel.virtual && rel.vrId) return `vr-${rel.vrId}`;
    return rel.edge ? rel.edge.constraintName : `idx-${i}`;
  }

  function connName(connectionId: string): string {
    return connectionStore.profiles.find((p) => p.id === connectionId)?.name ?? connectionId;
  }
</script>

<div class="relations-panel" bind:this={panelEl}>
  {#if !sel || !sel.columnName}
    <div class="empty-state">
      <LinkIcon class="empty-icon" />
      <p>Select a table cell to explore its relations</p>
    </div>
  {:else if sel.cellValue === null || sel.cellValue === undefined}
    <div class="empty-state">
      <CloseCircleIcon class="empty-icon" />
      <p>Cell is NULL — no relations to show</p>
    </div>
  {:else}
    <div class="context-bar">
      <div class="context-cell">
        <span class="context-table">{sel.table}</span><span class="context-dot">.</span><span
          class="context-col">{sel.columnName}</span
        >
      </div>
      <div class="context-value-row">
        <span class="context-eq-label">value</span>
        <span class="context-val">{formatValue(sel.cellValue)}</span>
      </div>
    </div>

    {#if globalLoading}
      <div class="global-state"><Loader /><span>Loading relations…</span></div>
    {:else if relations.length === 0}
      <div class="empty-state">
        <LinkIcon class="empty-icon" />
        <p>No relations found for <span class="inline-col">{sel.columnName}</span></p>
      </div>
    {:else}
      <div class="relations-list">
        {#if forwardRelations.length > 0}
          <div class="section">
            <div class="section-header section-header-spacer" aria-hidden="true" bind:this={fwdSpacerEl}>
              <div class="section-icon forward-icon"></div>
              <div class="section-info">
                <span class="section-label">References</span>
                <span class="section-desc">&nbsp;</span>
              </div>
            </div>
            {#if panelEl}
              <button
                class="section-header"
                use:portal
                style:position="fixed"
                style:top="{headerPositions.fwdTop}px"
                style:left="{headerPositions.panelLeft}px"
                style:width="{headerPositions.panelWidth}px"
                style:z-index="50"
                onclick={() => (forwardExpanded = !forwardExpanded)}
              >
                <div class="section-icon forward-icon">
                  <RelArrowIcon direction="forward" />
                </div>
                <div class="section-info">
                  <span class="section-label">References</span>
                  <span class="section-desc">{sel.table} → foreign key targets</span>
                </div>
                <span class="section-count">{forwardRelations.length}</span>
                <ChevronIcon class="section-chevron {!forwardExpanded ? 'collapsed' : ''}" width={16} height={16} strokeWidth={1.5} />
              </button>
            {/if}

            <div class="section-body" class:collapsed={!forwardExpanded}>
              <div class="section-body-inner">
                {#each forwardRelations as rel, i (getRelKey(rel, i))}
                  <div class="relation-card">
                    <div
                      class="card-header"
                      role="button"
                      tabindex="0"
                      title="{rel.targetTable}/{rel.filterColumn}"
                      onclick={() => (rel.expanded = !rel.expanded)}
                      onkeydown={(e) => e.key === 'Enter' && (rel.expanded = !rel.expanded)}
                    >
                      <TableSmIcon class="table-icon" />
                      <span class="card-header-text">
                        <span class="card-table">{rel.targetTable}</span>
                        <span class="card-filter">{rel.filterColumn}</span>
                        {#if rel.virtual}
                          <span class="virtual-badge" title="Virtual relation"
                            >{#if rel.targetConnectionId}<span class="card-via"
                                >{connName(rel.targetConnectionId)}・{rel.targetDatabase}</span
                              >{:else}Virtual{/if}</span
                          >
                        {/if}
                      </span>
                      {#if !rel.loading}
                        <span class="card-row-count">{(rel.totalRows ?? rel.rows.length).toLocaleString()}</span>
                      {/if}
                      <button
                        class="card-open-btn"
                        title="Open in panel"
                        onclick={(e) => {
                          e.stopPropagation();
                          openRelation(sel, rel);
                        }}
                      >
                        <OpenInPanelIcon />
                      </button>
                      <ChevronIcon class="card-chevron {!rel.expanded ? 'collapsed' : ''}" width={16} height={16} strokeWidth={1.5} />
                    </div>

                    <div class="card-collapse" class:collapsed={!rel.expanded}>
                      <div class="card-body">
                        {#if rel.loading}
                          <div class="card-state"><Loader /></div>
                        {:else if rel.rows.length > 0}
                          <div class="table-scroll">
                            <table class="data-table">
                              <thead>
                                <tr>
                                  {#each rel.columnNames as col}
                                    <th
                                      class="data-th"
                                      class:filter-col={col === rel.filterColumn}
                                      title={col}>{col}</th
                                    >
                                  {/each}
                                </tr>
                              </thead>
                              <tbody>
                                {#each rel.rows as row, ri (ri)}
                                  <tr class="data-row">
                                    {#each rel.columnNames as _col, ci (ci)}
                                      <td
                                        class="data-td"
                                        class:filter-col={ci ===
                                          rel.columnNames.indexOf(rel.filterColumn)}
                                        title={formatValue(row[ci])}
                                      >
                                        {#if row[ci] === null}<span class="null-val">NULL</span
                                          >{:else}{formatValue(row[ci])}{/if}
                                      </td>
                                    {/each}
                                  </tr>
                                {/each}
                              </tbody>
                            </table>
                          </div>
                        {:else}
                          <div class="card-empty">No rows found</div>
                        {/if}
                      </div>
                    </div>
                  </div>
                {/each}
              </div>
            </div>
          </div>
        {/if}

        {#if reverseRelations.length > 0}
          <div class="section">
            <div class="section-header section-header-spacer" aria-hidden="true" bind:this={revSpacerEl}>
              <div class="section-icon reverse-icon"></div>
              <div class="section-info">
                <span class="section-label">Referenced by</span>
                <span class="section-desc">&nbsp;</span>
              </div>
            </div>
            {#if panelEl}
              <button
                class="section-header"
                use:portal
                style:position="fixed"
                style:top="{headerPositions.revTop}px"
                style:left="{headerPositions.panelLeft}px"
                style:width="{headerPositions.panelWidth}px"
                style:z-index="51"
                onclick={() => (reverseExpanded = !reverseExpanded)}
              >
                <div class="section-icon reverse-icon">
                  <RelArrowIcon direction="back" />
                </div>
                <div class="section-info">
                  <span class="section-label">Referenced by</span>
                  <span class="section-desc">tables pointing to {sel.table}</span>
                </div>
                <span class="section-count">{reverseRelations.length}</span>
                <ChevronIcon class="section-chevron {!reverseExpanded ? 'collapsed' : ''}" width={16} height={16} strokeWidth={1.5} />
              </button>
            {/if}

            <div class="section-body" class:collapsed={!reverseExpanded}>
              <div class="section-body-inner">
                {#each reverseRelations as rel, i (getRelKey(rel, i + 1000))}
                  <div class="relation-card">
                    <div
                      class="card-header"
                      role="button"
                      tabindex="0"
                      title="{rel.targetTable}/{rel.filterColumn}"
                      onclick={() => (rel.expanded = !rel.expanded)}
                      onkeydown={(e) => e.key === 'Enter' && (rel.expanded = !rel.expanded)}
                    >
                      <TableSmIcon class="table-icon" />
                      <span class="card-header-text">
                        <span class="card-table">{rel.targetTable}</span>
                        <span class="card-filter">{rel.filterColumn}</span>
                        {#if rel.virtual}
                          <span class="virtual-badge" title="Virtual relation"
                            >{#if rel.targetConnectionId}<span class="card-via"
                                >{connName(rel.targetConnectionId)}・{rel.targetDatabase}</span
                              >{:else}Virtual{/if}</span
                          >
                        {/if}
                      </span>
                      {#if !rel.loading}
                        <span class="card-row-count">{(rel.totalRows ?? rel.rows.length).toLocaleString()}</span>
                      {/if}
                      <button
                        class="card-open-btn"
                        title="Open in panel"
                        onclick={(e) => {
                          e.stopPropagation();
                          openRelation(sel, rel);
                        }}
                      >
                        <OpenInPanelIcon />
                      </button>
                      <ChevronIcon class="card-chevron {!rel.expanded ? 'collapsed' : ''}" width={16} height={16} strokeWidth={1.5} />
                    </div>

                    <div class="card-collapse" class:collapsed={!rel.expanded}>
                      <div class="card-body">
                        {#if rel.loading}
                          <div class="card-state"><Loader /></div>
                        {:else if rel.rows.length > 0}
                          <div class="table-scroll">
                            <table class="data-table">
                              <thead>
                                <tr>
                                  {#each rel.columnNames as col}
                                    <th
                                      class="data-th"
                                      class:filter-col={col === rel.filterColumn}
                                      title={col}>{col}</th
                                    >
                                  {/each}
                                </tr>
                              </thead>
                              <tbody>
                                {#each rel.rows as row, ri (ri)}
                                  <tr class="data-row">
                                    {#each rel.columnNames as _col, ci (ci)}
                                      <td
                                        class="data-td"
                                        class:filter-col={ci ===
                                          rel.columnNames.indexOf(rel.filterColumn)}
                                        title={formatValue(row[ci])}
                                      >
                                        {#if row[ci] === null}<span class="null-val">NULL</span
                                          >{:else}{formatValue(row[ci])}{/if}
                                      </td>
                                    {/each}
                                  </tr>
                                {/each}
                              </tbody>
                            </table>
                          </div>
                        {:else}
                          <div class="card-empty">No rows found</div>
                        {/if}
                      </div>
                    </div>
                  </div>
                {/each}
              </div>
            </div>
          </div>
        {/if}
      </div>
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

  /* ── Empty / null states ── */

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--spacing-2);
    padding: var(--spacing-6) var(--spacing-4);
    color: var(--color-text-muted);
    text-align: center;
    flex: 1;
  }

  .empty-icon {
    width: 28px;
    height: 28px;
    opacity: 0.4;
  }

  .empty-state p {
    font-size: var(--font-size-xs);
    line-height: var(--line-height-normal);
    margin: 0;
  }

  .inline-col {
    font-family: var(--font-family-mono);
    color: var(--color-accent);
  }

  /* ── Context bar ── */

  .context-bar {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: var(--spacing-2) var(--spacing-3);
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .context-cell {
    display: flex;
    align-items: baseline;
    gap: 0;
    overflow: hidden;
  }

  .context-table {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-secondary);
    font-weight: var(--font-weight-medium);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .context-dot {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .context-col {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-accent);
    font-weight: var(--font-weight-semibold);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .context-value-row {
    display: flex;
    align-items: baseline;
    gap: var(--spacing-1);
  }

  .context-eq-label {
    font-size: 10px;
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    flex-shrink: 0;
  }

  .context-val {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* ── Global loading / error ── */

  .global-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--spacing-2);
    padding: var(--spacing-3) var(--spacing-3);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    flex: 1;
  }

  /* ── Sections ── */

  .relations-list {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
    padding-top: var(--spacing-2);
    flex-shrink: 0;
  }

  .section {
    display: flex;
    flex-direction: column;
  }

  .section-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
    border-top: 1px solid var(--color-border);
    background: color-mix(in srgb, var(--color-bg-secondary) 80%, transparent);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    width: 100%;
    text-align: left;
    cursor: pointer;
    transition: background var(--transition-fast);
    box-sizing: border-box;
  }

  .section-header-spacer {
    visibility: hidden;
    pointer-events: none;
    position: relative;
  }

  .section-header:not(.section-header-spacer):hover {
    background: color-mix(in srgb, var(--color-bg-tertiary) 80%, transparent);
  }

  .section-chevron {
    width: 13px;
    height: 13px;
    color: var(--color-text-muted);
    flex-shrink: 0;
    transition: transform 180ms ease;
  }

  .section-chevron.collapsed {
    transform: rotate(-90deg);
  }

  .section-body {
    display: grid;
    grid-template-rows: 1fr;
    transition: grid-template-rows 180ms ease;
  }

  .section-body.collapsed {
    grid-template-rows: 0fr;
  }

  .section-body-inner {
    overflow: hidden;
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
    padding: 0 var(--spacing-2);
  }

  .section-body-inner .relation-card:first-child {
    margin-top: var(--spacing-2);
  }

  .section-body-inner .relation-card:last-child {
    margin-bottom: var(--spacing-2);
  }

  .section-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: var(--radius-sm);
    flex-shrink: 0;
  }

  .section-icon :global(svg) {
    width: 12px;
    height: 12px;
  }

  .forward-icon {
    background: color-mix(in srgb, var(--color-accent) 15%, transparent);
    color: var(--color-accent);
  }

  .reverse-icon {
    background: color-mix(in srgb, var(--color-text-muted) 15%, transparent);
    color: var(--color-text-secondary);
  }

  .section-info {
    display: flex;
    flex-direction: column;
    gap: 1px;
    flex: 1;
    min-width: 0;
  }

  .section-label {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    line-height: 1.2;
  }

  .section-desc {
    font-size: 10px;
    color: var(--color-text-muted);
    font-family: var(--font-family-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .section-count {
    font-size: 11px;
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    background: var(--color-bg-tertiary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    padding: 0 6px;
    line-height: 18px;
    flex-shrink: 0;
  }

  /* ── Relation cards ── */

  .relation-card {
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    overflow: hidden;
  }

  .card-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: 5px 5px 5px var(--spacing-2);
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .card-header:hover {
    background: var(--color-bg-tertiary);
  }

  .card-chevron {
    width: 12px;
    height: 12px;
    color: var(--color-text-muted);
    flex-shrink: 0;
    transition: transform 180ms ease;
  }

  .card-chevron.collapsed {
    transform: rotate(-90deg);
  }

  .table-icon {
    width: 13px;
    height: 13px;
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .card-header-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-wrap: nowrap;
    align-items: baseline;
    gap: 3px;
    overflow: hidden;
  }

  .card-table {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex-shrink: 1;
    min-width: 0;
  }

  .card-filter {
    font-size: 10px;
    color: var(--color-text-muted);
    font-family: var(--font-family-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex-shrink: 1;
    min-width: 0;
  }

  .card-open-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: var(--radius-sm);
    color: var(--color-text-muted);
    flex-shrink: 0;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .card-open-btn:hover {
    background: var(--color-bg-primary);
    color: var(--color-text-primary);
  }

  .card-open-btn :global(svg) {
    width: 11px;
    height: 11px;
  }

  .virtual-badge {
    font-size: 9px;
    padding: 1px 5px;
    background: var(--color-accent-subtle);
    color: var(--color-accent);
    border-radius: var(--radius-lg);
    font-weight: var(--font-weight-semibold);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .card-via {
    font-size: 10px;
    letter-spacing: initial;
    text-transform: initial;
  }

  /* ── Card collapse / body ── */

  .card-collapse {
    display: grid;
    grid-template-rows: 1fr;
    transition: grid-template-rows 180ms ease;
  }

  .card-collapse.collapsed {
    grid-template-rows: 0fr;
  }

  .card-body {
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .card-state {
    display: flex;
    align-items: center;
    padding: var(--spacing-2) var(--spacing-3);
  }

  .card-empty {
    padding: var(--spacing-2) var(--spacing-3);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
  }

  .card-row-count {
    font-size: 10px;
    font-family: var(--font-family-mono);
    color: var(--color-text-muted);
    background: var(--color-bg-tertiary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    padding: 0 5px;
    line-height: 16px;
    align-self: center;
    margin-left: auto;
    flex-shrink: 0;
  }

  /* ── Data table ── */

  .table-scroll {
    overflow-x: auto;
  }

  .table-scroll::-webkit-scrollbar {
    height: 4px;
  }

  .table-scroll::-webkit-scrollbar-thumb {
    background: var(--color-border);
    border-radius: var(--radius-xs);
  }

  .data-table {
    width: max-content;
    border-collapse: collapse;
    font-size: var(--font-size-xs);
  }

  .data-th {
    padding: var(--spacing-1) var(--spacing-2);
    background: var(--color-bg-secondary);
    color: var(--color-text-muted);
    font-weight: var(--font-weight-semibold);
    font-size: 10px;
    text-align: left;
    white-space: nowrap;
    border-bottom: 1px solid var(--color-border);
    letter-spacing: 0.02em;
  }

  .data-th.filter-col {
    color: var(--color-accent);
  }

  .data-row:not(:last-child) {
    border-bottom: 1px solid color-mix(in srgb, var(--color-border) 50%, transparent);
  }

  .data-row:hover .data-td {
    background: var(--color-bg-secondary);
  }

  .data-td {
    padding: var(--spacing-1) var(--spacing-2);
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
    white-space: nowrap;
    vertical-align: middle;
    -webkit-user-select: text;
    user-select: text;
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--color-text-primary);
  }

  .data-td.filter-col {
    color: var(--color-accent);
    font-weight: var(--font-weight-medium);
  }

  .null-val {
    color: var(--color-null);
    font-style: italic;
    font-size: 10px;
  }
</style>
