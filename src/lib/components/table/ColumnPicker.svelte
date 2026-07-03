<!--
  ColumnPicker — floating popover for toggling column visibility and reordering columns.
  Clicking outside the panel calls onClose.
-->
<script lang="ts">
  import { onMount, tick } from 'svelte';
  import type { ColumnMeta } from '$lib/types';
  import Checkbox from '$lib/components/ui/Checkbox.svelte';
  import ContextMenu from '$lib/components/ui/ContextMenu.svelte';
  import CtxItem from '$lib/components/ui/CtxItem.svelte';

  interface Props {
    columns: ColumnMeta[];
    hiddenColumns: Set<string>;
    onToggle: (_columnName: string) => void;
    onClose: () => void;
    columnOrder?: string[];
    onReorder?: (_order: string[]) => void;
    onReset?: () => void;
    columnRenames?: Record<string, string>;
    onRename?: (_colName: string, _label: string) => void;
  }

  let {
    columns,
    hiddenColumns,
    onToggle,
    onClose,
    columnOrder,
    onReorder,
    onReset,
    columnRenames = {},
    onRename,
  }: Props = $props();

  let panelEl = $state<HTMLDivElement | null>(null);

  const orderedColumns = $derived.by((): ColumnMeta[] => {
    if (!columnOrder || columnOrder.length === 0) return columns;
    const colMap = new Map(columns.map((c) => [c.name, c]));
    const ordered = columnOrder.map((n) => colMap.get(n)).filter(Boolean) as ColumnMeta[];
    const inOrder = new Set(columnOrder);
    const remaining = columns.filter((c) => !inOrder.has(c.name));
    return [...ordered, ...remaining];
  });

  // ── Drag-to-reorder (pointer events, same pattern as DataTable header drag) ─

  let dragFromIndex = $state<number | null>(null);
  let isDragging = $state(false);
  let dragDropTarget = $state<{ index: number; position: 'before' | 'after' } | null>(null);
  let dragPointerStartY = 0;

  // Single insert-position index (0 = before first, N = after last).
  const insertAt = $derived.by(() => {
    if (!dragDropTarget || !isDragging) return null;
    return dragDropTarget.position === 'after' ? dragDropTarget.index + 1 : dragDropTarget.index;
  });

  $effect(() => {
    if (dragFromIndex === null) return;

    function onMove(e: PointerEvent) {
      if (!isDragging && Math.abs(e.clientY - dragPointerStartY) > 4) {
        isDragging = true;
      }
      if (!isDragging) return;

      const el = document.elementFromPoint(e.clientX, e.clientY);
      const li = el?.closest<HTMLElement>('[data-drag-index]');
      const targetStr = li?.dataset.dragIndex;
      if (targetStr === undefined) {
        dragDropTarget = null;
        return;
      }
      const targetIndex = parseInt(targetStr, 10);
      if (targetIndex === dragFromIndex) {
        dragDropTarget = null;
        return;
      }
      const rect = li!.getBoundingClientRect();
      const position = e.clientY < rect.top + rect.height / 2 ? 'before' : 'after';
      dragDropTarget = { index: targetIndex, position };
    }

    function onUp() {
      if (isDragging && dragDropTarget !== null && dragFromIndex !== null) {
        const from = dragFromIndex;
        const { index: toIndex, position } = dragDropTarget;
        let toIdx = position === 'after' ? toIndex + 1 : toIndex;
        if (from < toIndex) toIdx -= 1;
        if (toIdx !== from) {
          const newOrder = [...orderedColumns];
          const [moved] = newOrder.splice(from, 1);
          newOrder.splice(toIdx, 0, moved);
          onReorder?.(newOrder.map((c) => c.name));
        }
      }
      dragFromIndex = null;
      isDragging = false;
      dragDropTarget = null;
    }

    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);

    return () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };
  });

  function onDragHandlePointerDown(e: PointerEvent, index: number): void {
    if (e.button !== 0) return;
    dragPointerStartY = e.clientY;
    dragFromIndex = index;
    e.preventDefault();
  }

  // ── Rename ────────────────────────────────────────────────────────────────

  let renamingColumn = $state<string | null>(null);
  let renameValue = $state('');
  let renameInputEl = $state<HTMLInputElement | null>(null);
  let colContextMenu = $state<{ x: number; y: number; colName: string } | null>(null);

  function startRename(colName: string): void {
    colContextMenu = null;
    renamingColumn = colName;
    renameValue = columnRenames[colName] ?? colName;
    tick().then(() => {
      renameInputEl?.select();
    });
  }

  function commitRename(): void {
    if (renamingColumn === null) return;
    const colName = renamingColumn;
    const trimmed = renameValue.trim();
    renamingColumn = null;
    if (!onRename) return;
    if (trimmed === '' || trimmed === colName) {
      onRename(colName, colName);
    } else {
      onRename(colName, trimmed);
    }
  }

  function cancelRename(): void {
    renamingColumn = null;
  }

  function handleRenameKeydown(e: KeyboardEvent): void {
    if (e.key === 'Enter') {
      e.preventDefault();
      commitRename();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      cancelRename();
    }
  }

  function openColContextMenu(e: MouseEvent, colName: string): void {
    if (isDragging) return;
    e.preventDefault();
    e.stopPropagation();
    colContextMenu = { x: e.clientX, y: e.clientY, colName };
  }

  function handleDocumentClick(e: MouseEvent): void {
    if (colContextMenu !== null) {
      colContextMenu = null;
      return;
    }
    if (renamingColumn !== null) return;
    if (panelEl && !panelEl.contains(e.target as Node)) {
      onClose();
    }
  }

  onMount(() => {
    const timer = setTimeout(() => {
      document.addEventListener('click', handleDocumentClick);
    }, 0);

    return () => {
      clearTimeout(timer);
      document.removeEventListener('click', handleDocumentClick);
    };
  });
</script>

<div
  bind:this={panelEl}
  class="column-picker"
  class:is-dragging={isDragging}
  role="dialog"
  aria-label="Column visibility"
>
  <div class="picker-header">
    <span class="picker-title">Columns</span>
    <div class="picker-header-actions">
      {#if onReset}
        <button
          class="reset-btn"
          onclick={onReset}
          title="Show all columns in default order"
          aria-label="Reset columns">Reset</button
        >
      {/if}
      <button class="close-btn" onclick={onClose} aria-label="Close column picker"
        ><svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.5"
          stroke-linecap="round"
          aria-hidden="true"
          ><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg
        ></button
      >
    </div>
  </div>

  <ul class="column-list" role="list">
    {#each orderedColumns as col, index (col.name)}
      {@const hidden = hiddenColumns.has(col.name)}
      {@const label = columnRenames[col.name] ?? col.name}
      {@const isRenamed =
        columnRenames[col.name] !== undefined && columnRenames[col.name] !== col.name}
      {@const isRenaming = renamingColumn === col.name}
      {#if insertAt === index}
        <li class="drop-line" role="presentation"></li>
      {/if}
      <li
        class="column-row"
        class:dragging={dragFromIndex === index && isDragging}
        data-drag-index={index}
        role="listitem"
        oncontextmenu={(e) => openColContextMenu(e, col.name)}
      >
        {#if onReorder}
          <span
            class="drag-handle"
            aria-hidden="true"
            onpointerdown={(e) => onDragHandlePointerDown(e, index)}
          >
            <svg width="10" height="14" viewBox="0 0 10 14" fill="currentColor" aria-hidden="true">
              <circle cx="3" cy="2.5" r="1.2" /><circle cx="7" cy="2.5" r="1.2" />
              <circle cx="3" cy="7" r="1.2" /><circle cx="7" cy="7" r="1.2" />
              <circle cx="3" cy="11.5" r="1.2" /><circle cx="7" cy="11.5" r="1.2" />
            </svg>
          </span>
        {/if}
        {#if isRenaming}
          <div class="rename-row">
            <input
              bind:this={renameInputEl}
              class="rename-input"
              type="text"
              bind:value={renameValue}
              onkeydown={handleRenameKeydown}
              onblur={commitRename}
              aria-label="Rename column {col.name}"
            />
          </div>
        {:else}
          <button
            class="column-label"
            onclick={() => {
              if (!isDragging) onToggle(col.name);
            }}
            aria-pressed={!hidden}
          >
            <Checkbox size="sm" checked={!hidden} onchange={() => {}} />
            <span class="column-name" class:column-hidden={hidden} class:column-renamed={isRenamed}
              >{label}</span
            >
            <span class="column-type">{col.dataType}</span>
            {#if col.isPrimaryKey}
              <span class="badge pk-badge" title="Primary key">PK</span>
            {/if}
            {#if col.isForeignKey}
              <span class="badge fk-badge" title="Foreign key">FK</span>
            {/if}
          </button>
        {/if}
      </li>
    {/each}
    {#if insertAt === orderedColumns.length}
      <li class="drop-line" role="presentation"></li>
    {/if}
  </ul>
</div>

<ContextMenu
  x={colContextMenu?.x ?? 0}
  y={colContextMenu?.y ?? 0}
  open={colContextMenu !== null}
  onclose={() => (colContextMenu = null)}
  minWidth={140}
  zIndex={400}
>
  <CtxItem onclick={() => startRename(colContextMenu!.colName)}>Rename column</CtxItem>
  {#if colContextMenu !== null && columnRenames[colContextMenu.colName] !== undefined && columnRenames[colContextMenu.colName] !== colContextMenu.colName}
    <CtxItem onclick={() => {
      onRename?.(colContextMenu!.colName, colContextMenu!.colName);
      colContextMenu = null;
    }}>Reset name</CtxItem>
  {/if}
</ContextMenu>

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

  .is-dragging {
    cursor: grabbing;
    user-select: none;
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

  .picker-header-actions {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
  }

  .reset-btn {
    padding: 1px var(--spacing-2);
    background: transparent;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    color: var(--color-text-muted);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast),
      border-color var(--transition-fast);
    line-height: 1.4;
  }

  .reset-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
    border-color: var(--color-border-strong);
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
    display: flex;
    align-items: center;
  }

  .column-row.dragging {
    opacity: 0.4;
  }

  .drop-line {
    height: 2px;
    margin: -1px var(--spacing-2);
    background: var(--color-accent);
    border-radius: 1px;
    pointer-events: none;
    flex-shrink: 0;
  }

  .drag-handle {
    display: flex;
    align-items: center;
    padding: 0 var(--spacing-1) 0 var(--spacing-2);
    color: var(--color-text-muted);
    cursor: grab;
    flex-shrink: 0;
    opacity: 0.35;
    transition: opacity var(--transition-fast);
    touch-action: none;
  }

  .is-dragging .drag-handle {
    cursor: grabbing;
  }

  .column-row:hover .drag-handle {
    opacity: 1;
  }

  .column-label {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-1) var(--spacing-3) var(--spacing-1) var(--spacing-1);
    cursor: pointer;
    transition: background var(--transition-fast);
    font-size: var(--font-size-sm);
    flex: 1;
    background: transparent;
    border: none;
    text-align: left;
    font-family: inherit;
    min-width: 0;
  }

  .column-row:not(:has(.drag-handle)) .column-label {
    padding-left: var(--spacing-3);
  }

  .column-label:hover {
    background: var(--color-bg-hover);
  }

  .column-label :global(.checkbox-wrapper) {
    pointer-events: none;
  }

  .column-label:hover :global(.checkbox-box:not(.checkbox-box--checked)) {
    border-color: var(--color-accent);
    background: var(--color-accent-subtle);
  }

  .column-label:hover :global(.checkbox-box--checked) {
    background: var(--color-accent-hover);
    border-color: var(--color-accent-hover);
  }

  .is-dragging .column-label {
    pointer-events: none;
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

  .column-renamed {
    font-style: italic;
  }

  .rename-row {
    flex: 1;
    display: flex;
    align-items: center;
    padding: var(--spacing-1) var(--spacing-2) var(--spacing-1) var(--spacing-1);
  }

  .rename-input {
    flex: 1;
    padding: 2px var(--spacing-2);
    background: var(--color-bg-primary);
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-mono);
    color: var(--color-text-primary);
    outline: none;
    min-width: 0;
  }

</style>
