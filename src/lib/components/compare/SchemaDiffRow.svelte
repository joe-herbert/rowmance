<!--
  SchemaDiffRow — a single dense row rendering one ObjectDiff<T> entry
  (a column, index, foreign key, or view) inside SchemaDiffTree.
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
    fieldDiffs: FieldDiffLike[];
    crossEngineCaveat?: boolean;
  }

  interface Props {
    label: string;
    diff: ObjectDiffLike;
  }

  const { label, diff }: Props = $props();

  const statusLabel = $derived(
    diff.crossEngineCaveat
      ? 'review'
      : diff.status === 'added'
        ? 'added'
        : diff.status === 'removed'
          ? 'removed'
          : diff.status === 'changed'
            ? 'changed'
            : 'unchanged',
  );

  function formatValue(v: unknown): string {
    if (v === null || v === undefined) return '∅';
    if (Array.isArray(v)) return v.length ? v.join(', ') : '∅';
    if (typeof v === 'boolean') return v ? 'true' : 'false';
    const s = String(v);
    return s === '' ? '∅' : s;
  }
</script>

<div class="diff-row" class:review={diff.crossEngineCaveat}>
  <span class="status-badge status-{statusLabel}">{statusLabel}</span>
  <span class="diff-label mono">{label}</span>
  {#if diff.status === 'changed' && diff.fieldDiffs.length > 0}
    <div class="field-diffs">
      {#each diff.fieldDiffs as fd (fd.field)}
        <span class="field-diff">
          <span class="field-name">{fd.field}</span>
          <span class="field-value field-value--left mono">{formatValue(fd.left)}</span>
          <span class="field-arrow">→</span>
          <span class="field-value field-value--right mono">{formatValue(fd.right)}</span>
        </span>
      {/each}
    </div>
  {/if}
</div>

<style>
  .diff-row {
    display: flex;
    align-items: baseline;
    gap: var(--spacing-2);
    padding: 3px var(--spacing-2) 3px calc(var(--spacing-2) + 18px);
    font-size: var(--font-size-xs);
    flex-wrap: wrap;
    border-radius: var(--radius-sm);
  }

  .diff-row:hover {
    background: var(--color-bg-hover);
  }

  .status-badge {
    flex-shrink: 0;
    font-size: 9.5px;
    font-weight: var(--font-weight-semibold);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 1px 5px;
    border-radius: var(--radius-sm);
    min-width: 46px;
    text-align: center;
  }

  .status-added {
    background: var(--color-success-subtle);
    color: var(--color-success);
  }

  .status-removed {
    background: var(--color-danger-subtle);
    color: var(--color-danger);
  }

  .status-changed {
    background: var(--color-warning-subtle);
    color: var(--color-warning);
  }

  .status-review {
    background: var(--color-bg-secondary);
    color: var(--color-text-muted);
  }

  .status-unchanged {
    background: var(--color-bg-secondary);
    color: var(--color-text-muted);
  }

  .diff-label {
    color: var(--color-text-primary);
    flex-shrink: 0;
  }

  .mono {
    font-family: var(--font-family-mono);
  }

  .field-diffs {
    display: flex;
    flex-wrap: wrap;
    gap: var(--spacing-2);
    min-width: 0;
  }

  .field-diff {
    display: inline-flex;
    align-items: baseline;
    gap: 4px;
    color: var(--color-text-muted);
  }

  .field-name {
    font-size: 10px;
    color: var(--color-text-muted);
  }

  .field-value {
    font-size: var(--font-size-xs);
  }

  .field-value--left {
    color: var(--color-danger);
  }

  .field-value--right {
    color: var(--color-success);
  }

  .field-arrow {
    color: var(--color-text-disabled);
    font-size: 10px;
  }

  .review .status-badge {
    font-style: italic;
  }
</style>
