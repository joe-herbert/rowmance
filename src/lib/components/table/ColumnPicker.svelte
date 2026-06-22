<!--
  ColumnPicker — floating popover for toggling column visibility.
  Clicking outside the panel calls onClose.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import type { ColumnMeta } from '$lib/types';
  import Checkbox from '$lib/components/ui/Checkbox.svelte';

  interface Props {
    columns: ColumnMeta[];
    hiddenColumns: Set<string>;
    onToggle: (_columnName: string) => void;
    onClose: () => void;
  }

  let { columns, hiddenColumns, onToggle, onClose }: Props = $props();

  let panelEl = $state<HTMLDivElement | null>(null);

  function handleDocumentClick(e: MouseEvent): void {
    if (panelEl && !panelEl.contains(e.target as Node)) {
      onClose();
    }
  }

  onMount(() => {
    // Delay so the triggering click doesn't immediately close the panel
    const timer = setTimeout(() => {
      document.addEventListener('click', handleDocumentClick);
    }, 0);

    return () => {
      clearTimeout(timer);
      document.removeEventListener('click', handleDocumentClick);
    };
  });
</script>

<div bind:this={panelEl} class="column-picker" role="dialog" aria-label="Column visibility">
  <div class="picker-header">
    <span class="picker-title">Columns</span>
    <button class="close-btn" onclick={onClose} aria-label="Close column picker"><svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" aria-hidden="true"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg></button>
  </div>

  <ul class="column-list" role="list">
    {#each columns as col (col.name)}
      {@const hidden = hiddenColumns.has(col.name)}
      <li class="column-row" role="listitem">
        <button class="column-label" onclick={() => onToggle(col.name)} aria-pressed={!hidden}>
          <Checkbox
            size="sm"
            checked={!hidden}
            onchange={() => {}}
            aria-hidden="true"
            tabindex={-1}
          />
          <span class="column-name" class:column-hidden={hidden}>{col.name}</span>
          <span class="column-type">{col.dataType}</span>
          {#if col.isPrimaryKey}
            <span class="badge pk-badge" title="Primary key">PK</span>
          {/if}
          {#if col.isForeignKey}
            <span class="badge fk-badge" title="Foreign key">FK</span>
          {/if}
        </button>
      </li>
    {/each}
  </ul>
</div>

<style>
  .column-picker {
    position: absolute;
    z-index: 200;
    background: var(--color-bg-overlay);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-overlay);
    min-width: 220px;
    max-width: 320px;
    max-height: 400px;
    display: flex;
    flex-direction: column;
    overflow: clip;
  }

  .picker-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--spacing-2) var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    flex-shrink: 0;
  }

  .picker-title {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .close-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    padding: 0;
    background: transparent;
    border: none;
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    cursor: pointer;
    border-radius: var(--radius-sm);
    transition: background var(--transition-fast);
    line-height: 1;
  }

  .close-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .column-list {
    list-style: none;
    margin: 0;
    padding: var(--spacing-1) 0;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: var(--color-scrollbar-thumb) var(--color-scrollbar-track);
  }

  .column-row {
    display: block;
  }

  .column-label {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-1) var(--spacing-3);
    cursor: pointer;
    transition: background var(--transition-fast);
    font-size: var(--font-size-sm);
    width: 100%;
    background: transparent;
    border: none;
    text-align: left;
    font-family: inherit;
  }

  .column-label:hover {
    background: var(--color-bg-hover);
  }

  .column-name {
    flex: 1;
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .column-name.column-hidden {
    color: var(--color-text-muted);
    text-decoration: line-through;
  }

  .column-type {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .badge {
    flex-shrink: 0;
    font-size: 9px;
    font-weight: var(--font-weight-semibold);
    padding: 1px var(--spacing-1);
    border-radius: var(--radius-sm);
    font-family: var(--font-family-mono);
    line-height: 1.4;
  }

  .pk-badge {
    background: var(--color-warning-subtle);
    color: var(--color-warning);
    border: 1px solid var(--color-warning);
  }

  .fk-badge {
    background: var(--color-accent-subtle);
    color: var(--color-accent);
    border: 1px solid var(--color-accent);
  }
</style>
