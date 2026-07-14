<!--
  JsonViewerPanel — shows the currently selected row as an interactive JSON tree.
  FK and virtual-relation columns can be expanded to show the referenced row inline.
  Supports unlimited nesting depth via a recursive snippet and self-referential Expansion type.
-->
<script lang="ts">
  import { useCellSelection } from '$lib/stores/cellSelection.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { useVirtualRelations } from '$lib/stores/virtualRelations.svelte';
  import * as schemaApi from '$lib/tauri/schema';
  import { executeSelection } from '$lib/tauri/query';
  import type { ForeignKeyInfo, DbType } from '$lib/types';
  import { errorMessage } from '$lib/utils/errors';
  import { useToast } from '$lib/stores/toast.svelte';
  import JsonIcon from '$lib/components/icons/JsonIcon.svelte';
  import ExpandIcon from '$lib/components/icons/ExpandIcon.svelte';
  import CollapseIcon from '$lib/components/icons/CollapseIcon.svelte';
  import CheckIcon from '$lib/components/icons/CheckIcon.svelte';
  import CopyIcon from '$lib/components/icons/CopyIcon.svelte';
  import PointerIcon from '$lib/components/icons/PointerIcon.svelte';
  import Spinner from '$lib/components/ui/Spinner.svelte';

  type CellValue = string | number | boolean | null;

  interface FkTarget {
    targetTable: string;
    targetColumn: string;
    targetConnectionId?: string;
    targetDatabase?: string;
    virtual?: boolean;
  }

  // Self-referential — each expansion can expand its own FK children.
  interface Expansion {
    target: FkTarget;
    connId: string;
    db: string;
    expanded: boolean;
    loading: boolean;
    row: CellValue[] | null;
    columnNames: string[];
    fkTargets: Record<string, FkTarget>;
    children: Record<string, Expansion>;
    error: string | null;
  }

  const cellSelectionStore = useCellSelection();
  const connectionStore = useConnections();
  const vrStore = useVirtualRelations();
  const toast = useToast();

  let fkTargets = $state<Record<string, FkTarget>>({});
  let expansions = $state<Record<string, Expansion>>({});
  let metaKey = $state<string | null>(null);
  let copied = $state(false);

  function quoteId(name: string, dbType: DbType): string {
    return dbType === 'postgres'
      ? `"${name.replace(/"/g, '""')}"`
      : `\`${name.replace(/`/g, '``')}\``;
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

  async function loadFkTargets(connId: string, db: string, table: string): Promise<Record<string, FkTarget>> {
    const result: Record<string, FkTarget> = {};
    try {
      const fks = await schemaApi.listForeignKeys(connId, db, table);
      for (const fk of fks) {
        for (let i = 0; i < fk.columns.length; i++) {
          result[fk.columns[i]] = {
            targetTable: fk.referencedTable,
            targetColumn: fk.referencedColumns[i] ?? fk.referencedColumns[0],
          };
        }
      }
    } catch (_) {
      // Non-critical
    }
    return result;
  }

  // Reload top-level FK metadata when the source table changes.
  $effect(() => {
    const sel = cellSelectionStore.current;
    const key = sel ? `${sel.connectionId}:${sel.database}:${sel.table}` : null;
    if (key === metaKey) return;
    metaKey = key;
    fkTargets = {};
    expansions = {};
    if (!sel) return;

    schemaApi
      .listForeignKeys(sel.connectionId, sel.database, sel.table)
      .then((fks: ForeignKeyInfo[]) => {
        const next: Record<string, FkTarget> = {};
        for (const fk of fks) {
          for (let i = 0; i < fk.columns.length; i++) {
            next[fk.columns[i]] = {
              targetTable: fk.referencedTable,
              targetColumn: fk.referencedColumns[i] ?? fk.referencedColumns[0],
            };
          }
        }
        for (const col of sel.columns) {
          for (const vr of vrStore.forwardFrom({
            connectionId: sel.connectionId,
            database: sel.database,
            table: sel.table,
            column: col.name,
          })) {
            next[col.name] = {
              targetTable: vr.to.table,
              targetColumn: vr.to.column,
              targetConnectionId: vr.to.connectionId,
              targetDatabase: vr.to.database,
              virtual: true,
            };
          }
        }
        fkTargets = next;
      })
      .catch((err: unknown) => toast.addToast(errorMessage(err), 'error', 0));
  });

  // Single expand function — works on any level's children record.
  async function expand(
    children: Record<string, Expansion>,
    colName: string,
    target: FkTarget,
    cellValue: CellValue,
    parentConnId: string,
    parentDb: string,
  ) {
    if (cellValue === null) return;

    if (children[colName]) {
      children[colName].expanded = !children[colName].expanded;
      return;
    }

    const connId = target.targetConnectionId ?? parentConnId;
    const db = target.targetDatabase ?? parentDb;
    const dbType = connectionStore.getById(connId)?.dbType ?? 'mysql';

    children[colName] = {
      target,
      connId,
      db,
      expanded: false,
      loading: true,
      row: null,
      columnNames: [],
      fkTargets: {},
      children: {},
      error: null,
    };

    try {
      const sql = `SELECT * FROM ${quoteId(db, dbType)}.${quoteId(target.targetTable, dbType)} WHERE ${quoteId(target.targetColumn, dbType)} = ${valueLiteral(cellValue)} LIMIT 1`;
      const result = await executeSelection(connId, sql);
      if (result.error) {
        children[colName].error = result.error;
      } else if (result.rows.length > 0) {
        children[colName].row = result.rows[0];
        children[colName].columnNames = result.columns.map((c) => c.name);
        children[colName].fkTargets = await loadFkTargets(connId, db, target.targetTable);
      } else {
        children[colName].row = [];
      }
    } catch (err) {
      children[colName].error = errorMessage(err);
    } finally {
      children[colName].loading = false;
      children[colName].expanded = true;
    }
  }

  function collapseAllIn(children: Record<string, Expansion>) {
    for (const key of Object.keys(children)) {
      children[key].expanded = false;
      collapseAllIn(children[key].children);
    }
  }

  function collapseAll() {
    collapseAllIn(expansions);
  }

  async function expandRecursively(children: Record<string, Expansion>, visited: Set<string>) {
    await Promise.all(
      Object.values(children).map(async (child) => {
        if (!child.row || child.row.length === 0 || child.loading) return;
        child.expanded = true;
        await Promise.all(
          child.columnNames.map(async (colName, colIdx) => {
            if (!(colName in child.fkTargets)) return;
            const value = (child.row as CellValue[])[colIdx] ?? null;
            if (value === null) return;
            const target = child.fkTargets[colName];
            const connId = target.targetConnectionId ?? child.connId;
            const db = target.targetDatabase ?? child.db;
            const visitKey = `${connId}:${db}:${target.targetTable}:${String(value)}`;
            if (visited.has(visitKey)) return;
            visited.add(visitKey);
            if (child.children[colName]) {
              child.children[colName].expanded = true;
            } else {
              await expand(child.children, colName, target, value, child.connId, child.db);
            }
          }),
        );
        await expandRecursively(child.children, visited);
      }),
    );
  }

  async function expandAll() {
    const sel = cellSelectionStore.current;
    if (!sel) return;
    const visited = new Set<string>();
    await Promise.all(
      sel.columns.map(async (col, i) => {
        const value = sel.row[i] ?? null;
        if (!(col.name in fkTargets) || value === null) return;
        const target = fkTargets[col.name];
        const connId = target.targetConnectionId ?? sel.connectionId;
        const db = target.targetDatabase ?? sel.database;
        const visitKey = `${connId}:${db}:${target.targetTable}:${String(value)}`;
        if (visited.has(visitKey)) return;
        visited.add(visitKey);
        if (expansions[col.name]) {
          expansions[col.name].expanded = true;
        } else {
          await expand(expansions, col.name, target, value, sel.connectionId, sel.database);
        }
      }),
    );
    await expandRecursively(expansions, visited);
  }

  function buildCopyObject(columnNames: string[], row: CellValue[], children: Record<string, Expansion>): Record<string, unknown> {
    const obj: Record<string, unknown> = {};
    for (let i = 0; i < columnNames.length; i++) {
      const col = columnNames[i];
      const val = row[i] ?? null;
      const child = children[col];
      if (child?.expanded && child.row && child.row.length > 0) {
        obj[col] = buildCopyObject(child.columnNames, child.row, child.children);
      } else {
        obj[col] = val;
      }
    }
    return obj;
  }

  function copyAsJson() {
    const sel = cellSelectionStore.current;
    if (!sel) return;
    const obj = buildCopyObject(sel.columns.map((c) => c.name), sel.row, expansions);
    navigator.clipboard.writeText(JSON.stringify(obj, null, 2)).then(() => {
      copied = true;
      setTimeout(() => (copied = false), 1500);
    });
  }

  const sel = $derived(cellSelectionStore.current);
  const hasFkColumns = $derived(
    !!sel && sel.columns.some((c, i) => c.name in fkTargets && (sel.row[i] ?? null) !== null),
  );
</script>

<div class="json-panel">
  {#if !sel}
    <div class="empty-state">
      <JsonIcon class="empty-icon" />
      <p>Select a table row to view it as JSON</p>
    </div>
  {:else}
    <div class="toolbar">
      <span class="ctx-label"><span class="ctx-table">{sel.table}</span></span>
      <div class="toolbar-gap"></div>
      {#if hasFkColumns}
        <button class="icon-btn" onclick={expandAll} title="Expand all">
          <ExpandIcon width={13} height={13} strokeWidth={2} />
        </button>
        <button class="icon-btn" onclick={collapseAll} title="Collapse all">
          <CollapseIcon width={13} height={13} strokeWidth={2} />
        </button>
      {/if}
      <button class="copy-btn" onclick={copyAsJson} title="Copy row as JSON">
        {#if copied}
          <CheckIcon width={13} height={13} strokeWidth={2} />
        {:else}
          <CopyIcon width={13} height={13} />
        {/if}
        <span>{copied ? 'Copied' : 'Copy'}</span>
      </button>
    </div>

    <div class="json-tree" role="tree" aria-label="Row JSON view">
      <div class="brace">{'{'}</div>

      <!--
        Recursive snippet — renders a list of key/value entries at a given depth.
        Calls itself when an FK entry is expanded, incrementing depth each time.
      -->
      {#snippet entries(
        columns: string[],
        row: CellValue[],
        fkTs: Record<string, FkTarget>,
        children: Record<string, Expansion>,
        depth: number,
        parentConnId: string,
        parentDb: string,
      )}
        {@const anySiblingExpanded = Object.values(children).some((c) => c?.expanded)}
        {#each columns as col, i (col)}
          {@const value = row[i] ?? null}
          {@const hasFk = col in fkTs && value !== null}
          {@const child = children[col]}
          {@const isExpanded = child?.expanded ?? false}

          <div class="tree-entry" role="treeitem" aria-selected={false} aria-expanded={hasFk ? isExpanded : undefined}>
            <div class="entry-row">
              <div class="indent" style="--depth: {depth}"></div>

              {#if hasFk}
                <button
                  class="expand-btn"
                  class:expanded={isExpanded}
                  onclick={() => expand(children, col, fkTs[col], value, parentConnId, parentDb)}
                  disabled={child?.loading}
                  title={isExpanded ? 'Collapse' : 'Expand referenced row'}
                  aria-label={isExpanded ? 'Collapse' : 'Expand'}
                >
                  <PointerIcon />
                </button>
              {:else}
                <span class="expand-spacer"></span>
              {/if}

              <span class="json-key" class:child-key={depth > 1 && !anySiblingExpanded}>"{col}"</span>
              <span class="json-colon">:</span>

              <span
                class="json-value"
                class:jv-string={typeof value === 'string' && !hasFk}
                class:jv-number={typeof value === 'number'}
                class:jv-boolean={typeof value === 'boolean'}
                class:jv-null={value === null}
                class:jv-object={hasFk && isExpanded && !child?.loading}
              >
                {#if child?.loading}
                  <Spinner label="Loading" />
                {:else if hasFk && isExpanded}
                  {'{'}
                {:else if value === null}
                  null
                {:else if typeof value === 'string'}
                  "{value}"
                {:else}
                  {String(value)}
                {/if}
              </span>

              {#if hasFk && !isExpanded && !child?.loading && value !== null}
                {@const target = fkTs[col]}
                <span class="fk-hint" title="FK → {target.targetTable}.{target.targetColumn}">
                  → <span class="fk-table">{target.targetTable}</span>
                  {#if target.virtual}<span class="virtual-badge">virtual</span>{/if}
                </span>
              {/if}

              {#if i < columns.length - 1 && (!hasFk || !isExpanded || child?.loading)}
                <span class="json-comma">,</span>
              {/if}
            </div>

            {#if child?.expanded && !child.loading}
              <div class="child-block">
                {#if child.error}
                  <div class="child-message child-error">{child.error}</div>
                  <div class="entry-row">
                    <div class="indent" style="--depth: {depth}"></div>
                    <span class="expand-spacer"></span>
                    <span class="json-brace">{'}'}</span>
                    {#if i < columns.length - 1}<span class="json-comma">,</span>{/if}
                  </div>
                {:else if child.row !== null && child.row.length === 0}
                  <div class="child-message child-empty">No row found</div>
                  <div class="entry-row">
                    <div class="indent" style="--depth: {depth}"></div>
                    <span class="expand-spacer"></span>
                    <span class="json-brace">{'}'}</span>
                    {#if i < columns.length - 1}<span class="json-comma">,</span>{/if}
                  </div>
                {:else if child.row}
                  {@render entries(child.columnNames, child.row, child.fkTargets, child.children, depth + 1, child.connId, child.db)}
                  <div class="entry-row">
                    <div class="indent" style="--depth: {depth}"></div>
                    <span class="expand-spacer"></span>
                    <span class="json-brace">{'}'}</span>
                    {#if i < columns.length - 1}<span class="json-comma">,</span>{/if}
                  </div>
                {/if}
              </div>
            {/if}
          </div>
        {/each}
      {/snippet}

      {@render entries(
        sel.columns.map((c) => c.name),
        sel.row,
        fkTargets,
        expansions,
        1,
        sel.connectionId,
        sel.database,
      )}

      <div class="brace">{'}'}</div>
    </div>
  {/if}
</div>

<style>
  .json-panel {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  /* ── Empty state ── */

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

  /* ── Toolbar ── */

  .toolbar {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: 6px var(--spacing-2);
    min-height: 34px;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .ctx-label {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ctx-table {
    font-family: var(--font-family-mono);
    color: var(--color-text-secondary);
    font-weight: var(--font-weight-medium);
  }

  .toolbar-gap { flex: 1; }

  .icon-btn {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    flex-shrink: 0;
    transition: background var(--transition-fast), color var(--transition-fast);
  }

  .icon-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .copy-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 3px var(--spacing-2);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    background: transparent;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast);
    white-space: nowrap;
  }

  .copy-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  /* ── JSON tree ── */

  .json-tree {
    flex: 1;
    overflow-y: auto;
    overflow-x: auto;
    padding: var(--spacing-2) var(--spacing-2) var(--spacing-4);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    line-height: 1.7;
    -webkit-user-select: text;
    user-select: text;
  }

  .brace {
    color: var(--color-text-secondary);
    padding-left: 12px;
  }

  .tree-entry {
    display: flex;
    flex-direction: column;
  }

  .entry-row {
    display: flex;
    align-items: center;
    gap: 2px;
    min-height: 20px;
  }

  /* depth 1 = 0px, depth 2 = 12px, depth N = (N-1)*12px */
  .indent {
    width: calc((var(--depth) - 1) * 12px);
    flex-shrink: 0;
  }

  .expand-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    color: var(--color-text-muted);
    cursor: pointer;
    border-radius: var(--radius-xs);
    background: transparent;
    transition: background var(--transition-fast), color var(--transition-fast);
  }

  .expand-btn:hover:not(:disabled) {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .expand-btn:disabled { cursor: default; }

  .expand-btn :global(svg) {
    transition: transform 150ms ease;
    transform-origin: center;
  }

  .expand-btn.expanded :global(svg) {
    transform: rotate(90deg);
  }

  .expand-spacer {
    width: 14px;
    flex-shrink: 0;
  }

  /* ── JSON syntax tokens ── */

  .json-key {
    color: var(--color-text-secondary);
    font-weight: var(--font-weight-medium);
    white-space: nowrap;
  }

  .child-key {
    color: var(--color-text-muted);
    font-weight: normal;
  }

  .json-colon {
    color: var(--color-text-muted);
    margin-right: 2px;
  }

  .json-comma { color: var(--color-text-muted); }
  .json-brace { color: var(--color-text-secondary); }

  .json-value {
    white-space: pre;
    display: flex;
    align-items: center;
  }

  .jv-string { color: var(--color-success, #22c55e); }
  .jv-number { color: color-mix(in srgb, var(--color-accent) 90%, white); }
  .jv-boolean { color: var(--color-warning, #f59e0b); }
  .jv-null { color: var(--color-text-muted); font-style: italic; }
  .jv-object { color: var(--color-text-secondary); }

  /* ── FK hint ── */

  .fk-hint {
    font-size: 10px;
    color: var(--color-text-muted);
    margin-left: 4px;
    white-space: nowrap;
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .fk-table { color: var(--color-accent); }

  .virtual-badge {
    font-size: 9px;
    padding: 0 4px;
    background: var(--color-accent-subtle);
    color: var(--color-accent);
    border-radius: var(--radius-lg);
    font-weight: var(--font-weight-semibold);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    font-family: var(--font-family-ui);
    margin-left: 2px;
  }

  /* ── Child block ── */

  .child-block {
    display: flex;
    flex-direction: column;
  }

  .child-message {
    font-size: var(--font-size-xs);
    padding: 2px var(--spacing-2) 2px 28px;
    font-family: var(--font-family-ui);
  }

  .child-error {
    color: var(--color-danger);
    font-style: italic;
  }

  .child-empty {
    color: var(--color-text-muted);
    font-style: italic;
  }
</style>
