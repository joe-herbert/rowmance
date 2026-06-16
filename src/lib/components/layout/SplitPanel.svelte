<!--
  SplitPanel — renders the main area as a grid of up to 2×2 panels.
  The layout reacts to the SplitMode from the panels store.
  Drag dividers to resize; panels route their content type to the Panel component.
-->
<script lang="ts">
  import Panel from './Panel.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';

  const panelStore = usePanels();

  // ── Resize state ──────────────────────────────────────────────────────────

  /** Column split position as a percentage (only used in horizontal/quad modes). */
  let colSplitPct = $state(50);
  /** Row split position as a percentage (only used in vertical/quad modes). */
  let rowSplitPct = $state(50);

  type ResizeDimension = 'col' | 'row';
  let resizing = $state<ResizeDimension | null>(null);
  let resizeStartPos = $state(0);
  let resizeStartPct = $state(0);

  let containerEl = $state<HTMLElement | null>(null);

  function onColDividerPointerDown(event: PointerEvent) {
    resizing = 'col';
    resizeStartPos = event.clientX;
    resizeStartPct = colSplitPct;
    (event.target as HTMLElement).setPointerCapture(event.pointerId);
  }

  function onRowDividerPointerDown(event: PointerEvent) {
    resizing = 'row';
    resizeStartPos = event.clientY;
    resizeStartPct = rowSplitPct;
    (event.target as HTMLElement).setPointerCapture(event.pointerId);
  }

  function onPointerMove(event: PointerEvent) {
    if (!resizing || !containerEl) return;
    const rect = containerEl.getBoundingClientRect();

    if (resizing === 'col') {
      const delta = ((event.clientX - resizeStartPos) / rect.width) * 100;
      colSplitPct = Math.max(20, Math.min(80, resizeStartPct + delta));
    } else {
      const delta = ((event.clientY - resizeStartPos) / rect.height) * 100;
      rowSplitPct = Math.max(20, Math.min(80, resizeStartPct + delta));
    }
  }

  function onPointerUp() {
    resizing = null;
  }

  // ── CSS grid template derived from split mode ─────────────────────────────

  const gridTemplateColumns = $derived(() => {
    const { splitMode } = panelStore;
    if (splitMode === 'horizontal' || splitMode === 'quad') {
      return `${colSplitPct}% 4px calc(${100 - colSplitPct}% - 4px)`;
    }
    return '1fr';
  });

  const gridTemplateRows = $derived(() => {
    const { splitMode } = panelStore;
    if (splitMode === 'vertical' || splitMode === 'quad') {
      return `${rowSplitPct}% 4px calc(${100 - rowSplitPct}% - 4px)`;
    }
    return '1fr';
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="split-panel"
  bind:this={containerEl}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  style="
    grid-template-columns: {gridTemplateColumns()};
    grid-template-rows: {gridTemplateRows()};
  "
>
  {#if panelStore.splitMode === 'none'}
    <!-- Single panel -->
    <Panel index={0} panel={panelStore.panels[0]} isFocused={panelStore.focusedIndex === 0} />
  {:else if panelStore.splitMode === 'horizontal'}
    <!-- Two columns -->
    <Panel index={0} panel={panelStore.panels[0]} isFocused={panelStore.focusedIndex === 0} />
    <div
      class="divider divider--col"
      class:dragging={resizing === 'col'}
      role="separator"
      aria-orientation="vertical"
      onpointerdown={onColDividerPointerDown}
    ></div>
    <Panel index={1} panel={panelStore.panels[1]} isFocused={panelStore.focusedIndex === 1} />
  {:else if panelStore.splitMode === 'vertical'}
    <!-- Two rows -->
    <Panel index={0} panel={panelStore.panels[0]} isFocused={panelStore.focusedIndex === 0} />
    <div
      class="divider divider--row"
      class:dragging={resizing === 'row'}
      role="separator"
      aria-orientation="horizontal"
      onpointerdown={onRowDividerPointerDown}
    ></div>
    <Panel index={1} panel={panelStore.panels[1]} isFocused={panelStore.focusedIndex === 1} />
  {:else if panelStore.splitMode === 'quad'}
    <!-- 2×2 grid — panels: TL, TR, BL, BR -->
    <Panel index={0} panel={panelStore.panels[0]} isFocused={panelStore.focusedIndex === 0} />
    <div
      class="divider divider--col"
      class:dragging={resizing === 'col'}
      role="separator"
      aria-orientation="vertical"
      style="grid-row: 1 / 4;"
      onpointerdown={onColDividerPointerDown}
    ></div>
    <Panel
      index={1}
      panel={panelStore.panels[1] ?? panelStore.panels[0]}
      isFocused={panelStore.focusedIndex === 1}
    />
    <div
      class="divider divider--row"
      class:dragging={resizing === 'row'}
      role="separator"
      aria-orientation="horizontal"
      style="grid-column: 1 / 4;"
      onpointerdown={onRowDividerPointerDown}
    ></div>
    <Panel
      index={2}
      panel={panelStore.panels[2] ?? panelStore.panels[0]}
      isFocused={panelStore.focusedIndex === 2}
    />
    <div class="divider-spacer"></div>
    <Panel
      index={3}
      panel={panelStore.panels[3] ?? panelStore.panels[0]}
      isFocused={panelStore.focusedIndex === 3}
    />
  {/if}
</div>

<style>
  .split-panel {
    display: grid;
    width: 100%;
    height: 100%;
    overflow: hidden;
  }

  .divider--col {
    cursor: col-resize;
    background: var(--color-border);
    transition: background var(--transition-fast);
    z-index: 5;
    width: 1px;
    margin: 0 1.5px;
  }

  .divider--row {
    cursor: row-resize;
    background: var(--color-border);
    transition: background var(--transition-fast);
    z-index: 5;
    height: 1px;
    margin: 1.5px 0;
  }

  .divider--col:hover,
  .divider--col.dragging,
  .divider--row:hover,
  .divider--row.dragging {
    background: var(--color-accent);
    opacity: 0.5;
  }

  .divider-spacer {
    background: var(--color-border);
  }
</style>
