<!--
  SchemaComparePanel — top-level panel for the "compare schema structure"
  feature. Fetches both sides' schema data, runs the pure diff engine from
  $lib/utils/schema-diff, and renders a SchemaDiffTree plus a header showing
  source → target (swappable) and a "Generate SQL" action that opens
  SchemaSyncPreview.
-->
<script lang="ts">
  import { useConnections } from '$lib/stores/connections.svelte';
  import * as schemaApi from '$lib/tauri/schema';
  import type {
    BulkColumnInfo,
    CheckConstraintInfo,
    ColumnInfo,
    ForeignKeyInfo,
    IndexInfo,
    TableInfo,
    TriggerInfo,
    ViewInfo,
  } from '$lib/types';
  import {
    diffTable,
    diffDatabase,
    reorientSchemaDiff,
    reorientTableDiff,
    type SchemaDiff,
    type TableDiff,
    type TableSideData,
    type DatabaseSideData,
  } from '$lib/utils/schema-diff';
  import { generateSyncSql } from '$lib/utils/ddl-generation';
  import { errorMessage } from '$lib/utils/errors';
  import Loader from '$lib/components/ui/Loader.svelte';
  import CloseIcon from '$lib/components/icons/CloseIcon.svelte';
  import ArrowIcon from '$lib/components/icons/ArrowIcon.svelte';
  import SchemaDiffTree from '$lib/components/compare/SchemaDiffTree.svelte';
  import SchemaSyncPreview from '$lib/components/compare/SchemaSyncPreview.svelte';
  import ConnectionDot from '$lib/components/ui/ConnectionDot.svelte';
  import { connectionColor } from '$lib/utils/connectionColor';

  interface SideRef {
    connectionId: string;
    database: string;
    instanceDb?: string;
    table?: string;
  }

  interface Props {
    left: SideRef;
    right: SideRef;
  }

  const { left, right }: Props = $props();

  const connections = useConnections();

  type Direction = 'leftToRight' | 'rightToLeft';

  type ViewMode = 'diff' | 'sideBySide';

  let isLoading = $state(true);
  let loadError = $state<string | null>(null);
  let diffResult = $state<SchemaDiff | TableDiff | null>(null);
  let crossEngine = $state(false);
  let direction = $state<Direction>('leftToRight');
  let viewMode = $state<ViewMode>('diff');
  let syncSql = $state<string[]>([]);
  let showSyncPreview = $state(false);

  const leftProfile = $derived(connections.getById(left.connectionId));
  const rightProfile = $derived(connections.getById(right.connectionId));
  const isTableMode = $derived(!!left.table && !!right.table);

  const targetConnectionId = $derived(
    direction === 'leftToRight' ? right.connectionId : left.connectionId,
  );

  const hasDifferences = $derived.by(() => {
    if (!diffResult) return false;
    if ('tables' in diffResult) {
      return (
        diffResult.tables.some((t) => t.status !== 'unchanged') ||
        diffResult.views.some((v) => v.status !== 'unchanged')
      );
    }
    return diffResult.status !== 'unchanged';
  });

  /** The diff tree always reads from the current source → target point of view: 'added'
   *  means "will be created on the target", 'removed' means "will be dropped from the
   *  target", and changed rows show source's value before target's. `diffResult` itself is
   *  always computed against the fixed (left, right) pair, so it needs reorienting per the
   *  current direction — see the doc comment on reorientSchemaDiff/reorientTableDiff for why
   *  this isn't a simple unconditional flip. SQL generation keeps using the raw diffResult
   *  since it derives create/drop from `direction` itself. */
  const displayDiff = $derived.by(() => {
    if (!diffResult) return null;
    return 'tables' in diffResult
      ? reorientSchemaDiff(diffResult, direction)
      : reorientTableDiff(diffResult, direction);
  });

  function groupByTableName<T extends { tableName: string }>(items: T[]): Map<string, Omit<T, 'tableName'>[]> {
    const map = new Map<string, Omit<T, 'tableName'>[]>();
    for (const item of items) {
      const { tableName, ...rest } = item;
      const list = map.get(tableName);
      if (list) list.push(rest);
      else map.set(tableName, [rest]);
    }
    return map;
  }

  function buildTableDetails(
    tables: TableInfo[],
    columns: BulkColumnInfo[],
    indexes: Array<IndexInfo & { tableName: string }>,
    foreignKeys: Array<ForeignKeyInfo & { tableName: string }>,
    checkConstraints: CheckConstraintInfo[],
    triggers: TriggerInfo[],
  ): Map<string, TableSideData> {
    const colsByTable = groupByTableName(columns);
    const idxByTable = groupByTableName(indexes);
    const fksByTable = groupByTableName(foreignKeys);
    const checksByTable = groupByTableName(checkConstraints);
    const triggersByTable = groupByTableName(triggers);
    const map = new Map<string, TableSideData>();
    for (const t of tables) {
      map.set(t.name, {
        columns: (colsByTable.get(t.name) as ColumnInfo[] | undefined) ?? [],
        indexes: (idxByTable.get(t.name) as IndexInfo[] | undefined) ?? [],
        foreignKeys: (fksByTable.get(t.name) as ForeignKeyInfo[] | undefined) ?? [],
        checkConstraints:
          (checksByTable.get(t.name) as CheckConstraintInfo[] | undefined) ?? [],
        triggers: (triggersByTable.get(t.name) as TriggerInfo[] | undefined) ?? [],
      });
    }
    return map;
  }

  async function loadTableSide(ref: SideRef): Promise<TableSideData> {
    const [columns, indexes, foreignKeys, checkConstraints, triggers] = await Promise.all([
      schemaApi.listColumns(ref.connectionId, ref.database, ref.table!, ref.instanceDb),
      schemaApi.listIndexes(ref.connectionId, ref.database, ref.table!, ref.instanceDb),
      schemaApi.listForeignKeys(ref.connectionId, ref.database, ref.table!, ref.instanceDb),
      schemaApi.listCheckConstraints(ref.connectionId, ref.database, ref.table, ref.instanceDb),
      schemaApi.listTriggers(ref.connectionId, ref.database, ref.table, ref.instanceDb),
    ]);
    return { columns, indexes, foreignKeys, checkConstraints, triggers };
  }

  async function loadDatabaseSide(ref: SideRef): Promise<DatabaseSideData> {
    const [tables, columns, indexes, foreignKeys, views, checkConstraints, triggers] =
      await Promise.all([
        schemaApi.listTables(ref.connectionId, ref.database, ref.instanceDb),
        schemaApi.listAllColumns(ref.connectionId, ref.database, ref.instanceDb),
        schemaApi.listAllIndexes(ref.connectionId, ref.database, ref.instanceDb),
        schemaApi.listAllForeignKeys(ref.connectionId, ref.database, ref.instanceDb),
        schemaApi.listViews(ref.connectionId, ref.database, ref.instanceDb),
        schemaApi.listCheckConstraints(ref.connectionId, ref.database, undefined, ref.instanceDb),
        schemaApi.listTriggers(ref.connectionId, ref.database, undefined, ref.instanceDb),
      ]);
    return {
      tables,
      tableDetails: buildTableDetails(
        tables,
        columns,
        indexes,
        foreignKeys,
        checkConstraints,
        triggers,
      ),
      views,
    };
  }

  async function loadData() {
    isLoading = true;
    loadError = null;
    diffResult = null;
    try {
      const lProfile = connections.getById(left.connectionId);
      const rProfile = connections.getById(right.connectionId);
      if (!lProfile || !rProfile) {
        throw new Error('One or both connections could not be found.');
      }
      const ce = lProfile.dialectInfo.displayName !== rProfile.dialectInfo.displayName;
      crossEngine = ce;

      if (left.table && right.table) {
        const [leftSide, rightSide] = await Promise.all([
          loadTableSide(left),
          loadTableSide(right),
        ]);
        diffResult = diffTable(left.table, leftSide, rightSide, ce);
      } else {
        const [leftSide, rightSide] = await Promise.all([
          loadDatabaseSide(left),
          loadDatabaseSide(right),
        ]);
        diffResult = diffDatabase(leftSide, rightSide, ce);
      }
    } catch (err) {
      loadError = errorMessage(err);
    } finally {
      isLoading = false;
    }
  }

  $effect(() => {
    // Re-run whenever the identity of either side changes.
    void `${left.connectionId}:${left.database}:${left.instanceDb ?? ''}:${left.table ?? ''}`;
    void `${right.connectionId}:${right.database}:${right.instanceDb ?? ''}:${right.table ?? ''}`;
    loadData();
  });

  function asSchemaDiff(d: SchemaDiff | TableDiff): SchemaDiff {
    if ('tables' in d) return d;
    return { tables: [d], views: [], crossEngine };
  }

  function generateSql() {
    if (!diffResult || !leftProfile || !rightProfile) return;
    syncSql = generateSyncSql(
      asSchemaDiff(diffResult),
      direction,
      leftProfile.dialectInfo,
      rightProfile.dialectInfo,
      left.database,
      right.database,
    );
    showSyncPreview = true;
  }

  function sideLabel(ref: SideRef, profile: ReturnType<typeof connections.getById>): string {
    const identifier = ref.table ? `${ref.database}.${ref.table}` : ref.database;
    return `${profile?.name ?? ref.connectionId} › ${identifier}`;
  }

  const sourceProfile = $derived(direction === 'leftToRight' ? leftProfile : rightProfile);
  const targetProfile = $derived(direction === 'leftToRight' ? rightProfile : leftProfile);
  const sourceRef = $derived(direction === 'leftToRight' ? left : right);
  const targetRef = $derived(direction === 'leftToRight' ? right : left);
  const sourceLabel = $derived(sideLabel(sourceRef, sourceProfile));
  const targetLabel = $derived(sideLabel(targetRef, targetProfile));

  function swapDirection() {
    direction = direction === 'leftToRight' ? 'rightToLeft' : 'leftToRight';
  }
</script>

<div class="schema-compare-panel">
  <div class="toolbar">
    <div class="compare-route">
      <ConnectionDot color={connectionColor(sourceProfile)} />
      <span class="side-path mono" title="Source — this side is not modified">{sourceLabel}</span>
      {#if sourceProfile}<span class="side-dialect">{sourceProfile.dialectInfo.displayName}</span>{/if}
      <button
        class="swap-btn"
        onclick={swapDirection}
        title={`"${sourceLabel}" will be copied over "${targetLabel}" — click to switch direction`}
        aria-label="Switch which side is copied from"
      >
        <ArrowIcon direction="right" width={12} height={12} />
        <span class="swap-btn-label">copies to</span>
      </button>
      <ConnectionDot color={connectionColor(targetProfile)} />
      <span class="side-path mono" title="Target — will be overwritten to match the source">{targetLabel}</span>
      {#if targetProfile}<span class="side-dialect">{targetProfile.dialectInfo.displayName}</span>{/if}
      {#if crossEngine}
        <span class="cross-engine-badge" title="Comparing two different database engines — some differences may be inherent to the engine, not real drift.">
          Cross-engine
        </span>
      {/if}
    </div>
    <div class="toolbar-spacer"></div>
    <div class="view-mode-toggle" role="group" aria-label="View mode">
      <button
        class="view-mode-btn"
        class:active={viewMode === 'diff'}
        onclick={() => (viewMode = 'diff')}
      >
        Diff
      </button>
      <button
        class="view-mode-btn"
        class:active={viewMode === 'sideBySide'}
        onclick={() => (viewMode = 'sideBySide')}
      >
        Side by side
      </button>
    </div>
    <button
      class="generate-btn"
      onclick={generateSql}
      disabled={isLoading || !!loadError || !diffResult || !hasDifferences}
    >
      Generate SQL
    </button>
  </div>

  <div class="content">
    {#if isLoading}
      <div class="state-overlay">
        <Loader />
      </div>
    {:else if loadError}
      <div class="state-overlay state-overlay--error">
        <span class="error-icon" aria-hidden="true"
          ><CloseIcon width={20} height={20} strokeWidth={2} /></span
        >
        <span class="error-text">{loadError}</span>
        <button class="retry-btn" onclick={loadData}>Retry</button>
      </div>
    {:else if viewMode === 'sideBySide' && diffResult}
      <div class="sbs-header">
        <span class="sbs-header-cell mono">{sideLabel(left, leftProfile)}</span>
        <span class="sbs-header-cell mono">{sideLabel(right, rightProfile)}</span>
      </div>
      <div class="tree-scroll">
        <SchemaDiffTree diff={diffResult} mode="sideBySide" />
      </div>
    {:else if !diffResult || !hasDifferences}
      <div class="state-overlay">
        <span class="no-diff-text">No differences found.</span>
      </div>
    {:else if displayDiff}
      <div class="tree-scroll">
        <SchemaDiffTree diff={displayDiff} />
      </div>
    {/if}
  </div>

  {#if showSyncPreview}
    <SchemaSyncPreview
      sql={syncSql}
      targetConnectionId={targetConnectionId}
      sourceLabel={sourceLabel}
      targetLabel={targetLabel}
      onclose={() => (showSyncPreview = false)}
    />
  {/if}
</div>

<style>
  .schema-compare-panel {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    overflow: hidden;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: var(--spacing-3);
    padding: 0 var(--spacing-3);
    min-height: var(--toolbar-height);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .compare-route {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-1) 0;
    min-width: 0;
    flex-wrap: wrap;
  }

  .side-path {
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 260px;
  }

  .side-dialect {
    font-size: 10px;
    color: var(--color-text-muted);
    background: var(--color-bg-secondary);
    border-radius: var(--radius-sm);
    padding: 1px 5px;
    flex-shrink: 0;
  }

  .swap-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    flex-shrink: 0;
    padding: 3px 8px;
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    color: var(--color-text-secondary);
    font-size: 10.5px;
    white-space: nowrap;
    cursor: pointer;
    transition: all var(--transition-fast);
  }

  .swap-btn-label {
    font-family: var(--font-family-ui);
  }

  .swap-btn:hover {
    background: var(--color-bg-hover);
    border-color: var(--color-border-strong);
    color: var(--color-accent);
  }

  .mono {
    font-family: var(--font-family-mono);
  }

  .cross-engine-badge {
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    color: var(--color-warning);
    background: var(--color-warning-subtle);
    border-radius: var(--radius-sm);
    padding: 2px 7px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .toolbar-spacer {
    flex: 1;
    min-width: var(--spacing-2);
  }

  .view-mode-toggle {
    display: flex;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    overflow: hidden;
    flex-shrink: 0;
  }

  .view-mode-btn {
    padding: 4px 10px;
    background: var(--color-bg-primary);
    border: none;
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
    cursor: pointer;
    white-space: nowrap;
    transition: all var(--transition-fast);
  }

  .view-mode-btn + .view-mode-btn {
    border-left: 1px solid var(--color-border);
  }

  .view-mode-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .view-mode-btn.active {
    background: var(--color-accent-subtle);
    color: var(--color-accent);
    font-weight: var(--font-weight-medium);
  }

  .sbs-header {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .sbs-header-cell {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .generate-btn {
    padding: 5px 12px;
    height: calc(var(--toolbar-height) - var(--spacing-3));
    background: var(--color-accent);
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-md);
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    color: white;
    cursor: pointer;
    flex-shrink: 0;
    transition: opacity var(--transition-fast);
  }

  .generate-btn:hover:not(:disabled) {
    opacity: 0.88;
  }

  .generate-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .content {
    flex: 1;
    overflow: hidden;
    position: relative;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .tree-scroll {
    flex: 1;
    overflow: auto;
    min-height: 0;
  }

  .state-overlay {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--spacing-2);
    padding: var(--spacing-4);
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .state-overlay--error {
    color: var(--color-danger);
  }

  .error-icon {
    font-size: var(--font-size-lg);
  }

  .error-text {
    text-align: center;
    max-width: 300px;
    line-height: var(--line-height-normal);
    word-break: break-word;
    -webkit-user-select: text;
    user-select: text;
  }

  .no-diff-text {
    font-style: italic;
  }

  .retry-btn {
    margin-top: var(--spacing-1);
    padding: var(--spacing-1) var(--spacing-3);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
    background: var(--color-bg-primary);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .retry-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }
</style>
