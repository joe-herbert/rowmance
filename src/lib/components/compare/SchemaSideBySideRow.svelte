<!--
  SchemaSideBySideRow — one object (column, index, FK, check constraint, trigger,
  or view) rendered as two columns: what's actually on the left side and what's
  actually on the right side, independent of sync direction. Used by
  SchemaDiffTree when mode="sideBySide".
-->
<script lang="ts">
  import type { DiffStatus } from '$lib/utils/schema-diff';

  interface FieldDiffLike {
    field: string;
    left: unknown;
    right: unknown;
  }

  interface ObjectDiffLike {
    status: DiffStatus;
    left: unknown;
    right: unknown;
    fieldDiffs: FieldDiffLike[];
  }

  interface Props {
    label: string;
    diff: ObjectDiffLike;
  }

  const { label, diff }: Props = $props();

  const changedFields = $derived(new Set(diff.fieldDiffs.map((fd) => fd.field)));

  function formatValue(v: unknown): string {
    if (v === null || v === undefined) return '∅';
    if (Array.isArray(v)) return v.length ? v.join(', ') : '∅';
    if (typeof v === 'boolean') return v ? 'true' : 'false';
    const s = String(v);
    return s === '' ? '∅' : s;
  }

  /** Every field except the identity fields, as "field=value" parts — generic across
   *  ColumnInfo/IndexInfo/ForeignKeyInfo/CheckConstraintInfo/TriggerInfo/ViewInfo so this
   *  component doesn't need a per-object-type formatter. */
  function parts(obj: unknown): { field: string; text: string }[] {
    if (!obj || typeof obj !== 'object') return [];
    return Object.entries(obj as Record<string, unknown>)
      .filter(([k]) => k !== 'name' && k !== 'tableName' && k !== 'constraintName')
      .map(([field, v]) => ({ field, text: `${field}=${formatValue(v)}` }));
  }

  const leftParts = $derived(parts(diff.left));
  const rightParts = $derived(parts(diff.right));
</script>

<div class="sbs-row status-{diff.status}">
  <div class="sbs-cell">
    {#if diff.left}
      <span class="sbs-name mono">{label}</span>
      <span class="sbs-summary">
        {#each leftParts as p (p.field)}
          <span class="sbs-part" class:changed={changedFields.has(p.field)}>{p.text}</span>
        {/each}
      </span>
    {:else}
      <span class="sbs-empty">—</span>
    {/if}
  </div>
  <div class="sbs-cell">
    {#if diff.right}
      <span class="sbs-name mono">{label}</span>
      <span class="sbs-summary">
        {#each rightParts as p (p.field)}
          <span class="sbs-part" class:changed={changedFields.has(p.field)}>{p.text}</span>
        {/each}
      </span>
    {:else}
      <span class="sbs-empty">—</span>
    {/if}
  </div>
</div>

<style>
  .sbs-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--spacing-2);
    border-radius: var(--radius-sm);
  }

  .sbs-row:hover {
    background: var(--color-bg-hover);
  }

  .sbs-cell {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 6px;
    padding: 3px var(--spacing-2);
    font-size: var(--font-size-xs);
    min-width: 0;
  }

  .sbs-cell:first-child {
    padding-left: calc(var(--spacing-2) + 18px);
  }

  .mono {
    font-family: var(--font-family-mono);
  }

  .sbs-name {
    color: var(--color-text-primary);
    flex-shrink: 0;
  }

  .sbs-summary {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    min-width: 0;
    color: var(--color-text-muted);
  }

  .sbs-part.changed {
    color: var(--color-warning);
    font-weight: var(--font-weight-medium);
  }

  .sbs-empty {
    color: var(--color-text-disabled);
  }

  .status-added .sbs-cell:last-child {
    background: var(--color-success-subtle);
  }

  .status-removed .sbs-cell:first-child {
    background: var(--color-danger-subtle);
  }

  .status-changed .sbs-cell {
    background: var(--color-warning-subtle);
  }
</style>
