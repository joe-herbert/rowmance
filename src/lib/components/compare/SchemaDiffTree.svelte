<!--
  SchemaDiffTree — renders a full SchemaDiff (database-to-database compare) or a
  single TableDiff (table-to-table compare) as a collapsible tree of
  tables/views > groups (Columns/Indexes/Foreign Keys) > rows.

  Two display modes, controlled by `mode`:
  - 'diff' (default): drift-focused. Unchanged items are collapsed behind
    small "N unchanged" toggles so differences are the prominent thing on
    screen; only tables/views with actual changes are expanded by default.
  - 'sideBySide': structure-focused. Every table/view and every row (changed
    or not) is shown and expanded up front, each row rendered as two columns
    (what's on the left vs what's on the right) via SchemaSideBySideRow, so
    the two full structures can be eyeballed directly rather than just their
    diff summary.
-->
<script lang="ts">
  import ChevronIcon from '$lib/components/icons/ChevronIcon.svelte';
  import SchemaDiffRow from '$lib/components/compare/SchemaDiffRow.svelte';
  import SchemaSideBySideRow from '$lib/components/compare/SchemaSideBySideRow.svelte';
  import type { SchemaDiff, TableDiff, ObjectDiff, DiffStatus } from '$lib/utils/schema-diff';

  interface Props {
    diff: SchemaDiff | TableDiff;
    mode?: 'diff' | 'sideBySide';
  }

  const { diff, mode = 'diff' }: Props = $props();
  const sideBySide = $derived(mode === 'sideBySide');

  function isSchemaDiff(d: SchemaDiff | TableDiff): d is SchemaDiff {
    return 'tables' in d;
  }

  function isNonUnchanged(d: { status: DiffStatus }): boolean {
    return d.status !== 'unchanged';
  }

  function counts(items: { status: DiffStatus }[]): { added: number; removed: number; changed: number } {
    let added = 0;
    let removed = 0;
    let changed = 0;
    for (const i of items) {
      if (i.status === 'added') added++;
      else if (i.status === 'removed') removed++;
      else if (i.status === 'changed') changed++;
    }
    return { added, removed, changed };
  }

  function tableCounts(t: TableDiff) {
    return counts([
      ...t.columns,
      ...t.indexes,
      ...t.foreignKeys,
      ...t.checkConstraints,
      ...t.triggers,
    ]);
  }

  // ── Expand/collapse state ────────────────────────────────────────────────────

  let expandedTables = $state<Set<string>>(new Set());
  let showUnchangedTables = $state(false);
  let showUnchangedViews = $state(false);
  // Per-table-per-group "show unchanged" toggles, keyed by `${tableKey}:${group}`.
  let showUnchangedGroups = $state<Set<string>>(new Set());

  function toggleTable(key: string) {
    const next = new Set(expandedTables);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    expandedTables = next;
  }

  function toggleUnchangedGroup(key: string) {
    const next = new Set(showUnchangedGroups);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    showUnchangedGroups = next;
  }

  const singleTable = $derived(isSchemaDiff(diff) ? null : diff);
  const schemaDiff = $derived(isSchemaDiff(diff) ? diff : null);

  const sortedTables = $derived(
    schemaDiff
      ? [...schemaDiff.tables].sort((a, b) => a.tableName.localeCompare(b.tableName))
      : [],
  );
  // In sideBySide mode every table/view is shown (no unchanged/hidden split) — the whole
  // point is to see the full structures, not just what differs.
  const changedTables = $derived(
    sideBySide ? sortedTables : sortedTables.filter((t) => t.status !== 'unchanged'),
  );
  const unchangedTables = $derived(
    sideBySide ? [] : sortedTables.filter((t) => t.status === 'unchanged'),
  );

  const changedViews = $derived(
    schemaDiff ? (sideBySide ? schemaDiff.views : schemaDiff.views.filter(isNonUnchanged)) : [],
  );
  const unchangedViews = $derived(
    sideBySide ? [] : schemaDiff ? schemaDiff.views.filter((v) => v.status === 'unchanged') : [],
  );

  // Auto-expand a lone table-to-table diff — there's nothing else to navigate to. In
  // sideBySide mode, auto-expand everything so the full structures are visible up front.
  $effect(() => {
    if (singleTable) {
      expandedTables = new Set([singleTable.tableName]);
    } else if (sideBySide) {
      expandedTables = new Set([...sortedTables.map((t) => t.tableName), '__views__']);
    }
  });
</script>

{#snippet row(label: string, item: ObjectDiff<any>)}
  {#if sideBySide}
    <SchemaSideBySideRow {label} diff={item} />
  {:else}
    <SchemaDiffRow {label} diff={item} />
  {/if}
{/snippet}

{#snippet groupSection(groupKey: string, title: string, items: ObjectDiff<any>[])}
  {#if items.length > 0}
    {@const nonUnchanged = sideBySide ? items : items.filter(isNonUnchanged)}
    {@const unchanged = sideBySide ? [] : items.filter((i) => i.status === 'unchanged')}
    {@const showUnchanged = showUnchangedGroups.has(groupKey)}
    <div class="group">
      <div class="group-title">{title}</div>
      {#each nonUnchanged as item (item.key)}
        {@render row(item.left?.name ?? item.right?.name ?? item.key, item)}
      {/each}
      {#if unchanged.length > 0}
        <button class="unchanged-toggle" onclick={() => toggleUnchangedGroup(groupKey)}>
          {showUnchanged ? 'Hide' : 'Show'}
          {unchanged.length} unchanged
        </button>
        {#if showUnchanged}
          {#each unchanged as item (item.key)}
            {@render row(item.left?.name ?? item.right?.name ?? item.key, item)}
          {/each}
        {/if}
      {/if}
    </div>
  {/if}
{/snippet}

{#snippet constraintGroupSection(groupKey: string, title: string, items: ObjectDiff<any>[])}
  {#if items.length > 0}
    {@const nameOf = (d: ObjectDiff<any>) => d.left?.constraintName ?? d.right?.constraintName ?? d.key}
    {@const nonUnchanged = sideBySide ? items : items.filter(isNonUnchanged)}
    {@const unchanged = sideBySide ? [] : items.filter((i) => i.status === 'unchanged')}
    {@const showUnchanged = showUnchangedGroups.has(groupKey)}
    <div class="group">
      <div class="group-title">{title}</div>
      {#each nonUnchanged as item (item.key)}
        {@render row(nameOf(item), item)}
      {/each}
      {#if unchanged.length > 0}
        <button class="unchanged-toggle" onclick={() => toggleUnchangedGroup(groupKey)}>
          {showUnchanged ? 'Hide' : 'Show'}
          {unchanged.length} unchanged
        </button>
        {#if showUnchanged}
          {#each unchanged as item (item.key)}
            {@render row(nameOf(item), item)}
          {/each}
        {/if}
      {/if}
    </div>
  {/if}
{/snippet}

{#snippet tableGroups(t: TableDiff)}
  {@render groupSection(`${t.tableName}:columns`, 'Columns', t.columns)}
  {@render groupSection(`${t.tableName}:indexes`, 'Indexes', t.indexes)}
  {@render constraintGroupSection(`${t.tableName}:fks`, 'Foreign Keys', t.foreignKeys)}
  {@render constraintGroupSection(`${t.tableName}:checks`, 'Check Constraints', t.checkConstraints)}
  {@render groupSection(`${t.tableName}:triggers`, 'Triggers', t.triggers)}
{/snippet}

<div class="diff-tree">
  {#if singleTable}
    <div class="single-table-groups">
      {@render tableGroups(singleTable)}
    </div>
  {:else if schemaDiff}
    {#each changedTables as t (t.tableName)}
      {@const c = tableCounts(t)}
      {@const expanded = expandedTables.has(t.tableName)}
      <div class="table-section">
        <button class="table-header" onclick={() => toggleTable(t.tableName)}>
          <span class="chevron" class:open={expanded}>
            <ChevronIcon direction="right" width={12} height={12} strokeWidth={2.2} />
          </span>
          <span class="table-name mono">{t.tableName}</span>
          {#if t.status === 'added'}
            <span class="table-status-badge status-added">added</span>
          {:else if t.status === 'removed'}
            <span class="table-status-badge status-removed">removed</span>
          {:else}
            <span class="counts-badges">
              {#if c.added > 0}<span class="count-badge count-added">+{c.added}</span>{/if}
              {#if c.removed > 0}<span class="count-badge count-removed">-{c.removed}</span>{/if}
              {#if c.changed > 0}<span class="count-badge count-changed">~{c.changed}</span>{/if}
            </span>
          {/if}
        </button>
        {#if expanded}
          <div class="table-body">
            {@render tableGroups(t)}
          </div>
        {/if}
      </div>
    {/each}

    {#if changedViews.length > 0}
      {@const expanded = expandedTables.has('__views__')}
      {@const c = counts(changedViews)}
      <div class="table-section">
        <button class="table-header" onclick={() => toggleTable('__views__')}>
          <span class="chevron" class:open={expanded}>
            <ChevronIcon direction="right" width={12} height={12} strokeWidth={2.2} />
          </span>
          <span class="table-name mono">Views</span>
          <span class="counts-badges">
            {#if c.added > 0}<span class="count-badge count-added">+{c.added}</span>{/if}
            {#if c.removed > 0}<span class="count-badge count-removed">-{c.removed}</span>{/if}
            {#if c.changed > 0}<span class="count-badge count-changed">~{c.changed}</span>{/if}
          </span>
        </button>
        {#if expanded}
          <div class="table-body">
            <div class="group">
              {#each changedViews as v (v.key)}
                {@render row(v.left?.name ?? v.right?.name ?? v.key, v)}
              {/each}
              {#if unchangedViews.length > 0}
                <button class="unchanged-toggle" onclick={() => (showUnchangedViews = !showUnchangedViews)}>
                  {showUnchangedViews ? 'Hide' : 'Show'}
                  {unchangedViews.length} unchanged
                </button>
                {#if showUnchangedViews}
                  {#each unchangedViews as v (v.key)}
                    {@render row(v.left?.name ?? v.right?.name ?? v.key, v)}
                  {/each}
                {/if}
              {/if}
            </div>
          </div>
        {/if}
      </div>
    {/if}

    {#if unchangedTables.length > 0}
      <button class="unchanged-toggle unchanged-toggle--top" onclick={() => (showUnchangedTables = !showUnchangedTables)}>
        {showUnchangedTables ? 'Hide' : 'Show'}
        {unchangedTables.length} unchanged table{unchangedTables.length === 1 ? '' : 's'}
      </button>
      {#if showUnchangedTables}
        {#each unchangedTables as t (t.tableName)}
          {@const expanded = expandedTables.has(t.tableName)}
          <div class="table-section">
            <button class="table-header" onclick={() => toggleTable(t.tableName)}>
              <span class="chevron" class:open={expanded}>
                <ChevronIcon direction="right" width={12} height={12} strokeWidth={2.2} />
              </span>
              <span class="table-name mono">{t.tableName}</span>
              <span class="table-status-badge status-unchanged">unchanged</span>
            </button>
            {#if expanded}
              <div class="table-body">
                {@render tableGroups(t)}
              </div>
            {/if}
          </div>
        {/each}
      {/if}
    {/if}
  {/if}
</div>

<style>
  .diff-tree {
    display: flex;
    flex-direction: column;
    padding: var(--spacing-1) 0;
  }

  .mono {
    font-family: var(--font-family-mono);
  }

  .table-section {
    border-bottom: 1px solid var(--color-border);
  }

  .table-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    width: 100%;
    padding: var(--spacing-1) var(--spacing-2);
    background: transparent;
    border: none;
    cursor: pointer;
    text-align: left;
    font-family: inherit;
  }

  .table-header:hover {
    background: var(--color-bg-hover);
  }

  .chevron {
    display: flex;
    flex-shrink: 0;
    color: var(--color-text-muted);
    transition: transform var(--transition-fast);
  }

  .chevron.open :global(svg) {
    transform: rotate(90deg);
  }

  .table-name {
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    font-weight: var(--font-weight-medium);
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .table-status-badge {
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 1px 6px;
    border-radius: var(--radius-sm);
    flex-shrink: 0;
  }

  .status-added {
    background: var(--color-success-subtle);
    color: var(--color-success);
  }

  .status-removed {
    background: var(--color-danger-subtle);
    color: var(--color-danger);
  }

  .status-unchanged {
    background: var(--color-bg-secondary);
    color: var(--color-text-muted);
  }

  .counts-badges {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
  }

  .count-badge {
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    padding: 1px 5px;
    border-radius: var(--radius-sm);
  }

  .count-added {
    background: var(--color-success-subtle);
    color: var(--color-success);
  }

  .count-removed {
    background: var(--color-danger-subtle);
    color: var(--color-danger);
  }

  .count-changed {
    background: var(--color-warning-subtle);
    color: var(--color-warning);
  }

  .table-body {
    padding: 2px 0 var(--spacing-1);
  }

  .single-table-groups {
    padding-top: var(--spacing-1);
  }

  .group {
    padding: 2px 0;
  }

  .group-title {
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 2px var(--spacing-2) 2px calc(var(--spacing-2) + 18px);
  }

  .unchanged-toggle {
    display: block;
    margin: 2px var(--spacing-2) 2px calc(var(--spacing-2) + 18px);
    padding: 1px 6px;
    font-size: 10.5px;
    color: var(--color-text-muted);
    background: transparent;
    border: 1px dashed var(--color-border);
    border-radius: var(--radius-sm);
    cursor: pointer;
  }

  .unchanged-toggle:hover {
    color: var(--color-text-primary);
    border-color: var(--color-border-strong);
  }

  .unchanged-toggle--top {
    margin: var(--spacing-2);
  }
</style>
