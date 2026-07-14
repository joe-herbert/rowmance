<!--
  DashboardPanel — the main view for a single dashboard.
  Auto-connects needed connections, displays widget grid.
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import type { DashboardWidget } from '$lib/types';
  import { useDashboards } from '$lib/stores/dashboards.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { useToast } from '$lib/stores/toast.svelte';
  import DashboardWidgetComponent from './DashboardWidget.svelte';
  import WidgetEditor from './WidgetEditor.svelte';
  import IconPicker from './IconPicker.svelte';
  import { portal } from '$lib/actions/portal';

  interface Props {
    dashboardId: string;
  }

  const { dashboardId }: Props = $props();

  const dashboardsStore = useDashboards();
  const connectionsStore = useConnections();
  const toast = useToast();

  const dashboard = $derived(dashboardsStore.getById(dashboardId));

  let editMode = $state(false);
  let addingWidget = $state(false);
  let editingWidget = $state<DashboardWidget | null>(null);
  let connecting = $state(false);

  // ── Auto-connect needed connections on mount ──────────────────────────────

  onMount(() => {
    autoConnect();
  });

  $effect(() => {
    void dashboard?.widgets;
    untrack(autoConnect);
  });

  async function autoConnect() {
    if (!dashboard) return;
    const neededIds = [...new Set(dashboard.widgets.map((w) => w.connectionId))].filter(
      (id) => id && connectionsStore.getById(id),
    );
    const toConnect = neededIds.filter((id) => !connectionsStore.isActive(id));
    if (toConnect.length === 0) return;

    connecting = true;
    try {
      await Promise.all(toConnect.map((id) => connectionsStore.connect(id)));
    } catch {
      // individual errors are shown per-widget
    } finally {
      connecting = false;
    }
  }

  // ── Widget editor ─────────────────────────────────────────────────────────

  function handleAddWidget(input: Omit<DashboardWidget, 'id' | 'x' | 'y'>) {
    dashboardsStore.addWidget(dashboardId, input);
    addingWidget = false;
  }

  function handleEditWidget(input: Omit<DashboardWidget, 'id' | 'x' | 'y'>) {
    if (!editingWidget) return;
    dashboardsStore.updateWidget(dashboardId, editingWidget.id, input);
    editingWidget = null;
  }

  function handleDeleteWidget(widgetId: string) {
    dashboardsStore.deleteWidget(dashboardId, widgetId);
  }

  // ── Shared grid helpers ───────────────────────────────────────────────────

  const ROW_HEIGHT = 80;
  let gridEl = $state<HTMLDivElement | undefined>(undefined);

  function gridMetrics() {
    if (!gridEl) return null;
    const rect = gridEl.getBoundingClientRect();
    const style = getComputedStyle(gridEl);
    const pl = parseFloat(style.paddingLeft) || 0;
    const pr = parseFloat(style.paddingRight) || 0;
    const pt = parseFloat(style.paddingTop) || 0;
    const colGap = parseFloat(style.columnGap) || 0;
    const rowGap = parseFloat(style.rowGap) || 0;
    const innerW = rect.width - pl - pr;
    const colWidth = (innerW - 11 * colGap) / 12;
    return { rect, pl, pt, colWidth, colGap, rowGap };
  }

  function pointerToCell(e: PointerEvent) {
    const m = gridMetrics();
    if (!m) return null;
    const relX = e.clientX - m.rect.left - m.pl;
    const relY = e.clientY - m.rect.top - m.pt + gridEl!.scrollTop;
    const col = Math.max(1, Math.floor(relX / (m.colWidth + m.colGap)) + 1);
    const row = Math.max(1, Math.floor(relY / (ROW_HEIGHT + m.rowGap)) + 1);
    return { x: col, y: row };
  }

  // ── Drag-to-position ──────────────────────────────────────────────────────

  let draggingId = $state<string | null>(null);
  let dragPreviewX = $state(0);
  let dragPreviewY = $state(0);
  let dragOffsetX = 0;
  let dragOffsetY = 0;

  function startDrag(widgetId: string, e: PointerEvent) {
    const widget = dashboard?.widgets.find((w) => w.id === widgetId);
    if (!widget) return;
    e.preventDefault();
    const cell = pointerToCell(e);
    dragOffsetX = cell ? Math.max(0, Math.min(widget.w - 1, cell.x - widget.x)) : 0;
    dragOffsetY = cell ? Math.max(0, Math.min(widget.h - 1, cell.y - widget.y)) : 0;
    draggingId = widgetId;
    dragPreviewX = widget.x;
    dragPreviewY = widget.y;
  }

  $effect(() => {
    if (!draggingId) return;
    const widget = dashboard?.widgets.find((w) => w.id === draggingId);
    if (!widget) return;
    const others = dashboard?.widgets.filter((w) => w.id !== draggingId) ?? [];

    function wouldOverlap(x: number, y: number) {
      return others.some(
        (o) => x < o.x + o.w && x + widget!.w > o.x && y < o.y + o.h && y + widget!.h > o.y,
      );
    }

    function onMove(e: PointerEvent) {
      const cell = pointerToCell(e);
      if (!cell) return;
      const nx = Math.max(1, Math.min(13 - widget!.w, cell.x - dragOffsetX));
      const ny = Math.max(1, cell.y - dragOffsetY);
      if (!wouldOverlap(nx, ny)) {
        dragPreviewX = nx;
        dragPreviewY = ny;
      }
    }
    function onUp() {
      if (draggingId) {
        dashboardsStore.updateWidget(dashboardId, draggingId, {
          x: dragPreviewX,
          y: dragPreviewY,
        });
      }
      draggingId = null;
    }
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    return () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };
  });

  // ── Resize ────────────────────────────────────────────────────────────────

  let resizingId = $state<string | null>(null);
  let resizeStartW = 0, resizeStartH = 0;
  let resizeStartX = 0, resizeStartY = 0;
  let resizeColWidth = 0;
  let resizePreviewW = $state(0);
  let resizePreviewH = $state(0);

  function startResize(widgetId: string, e: PointerEvent) {
    const widget = dashboard?.widgets.find((w) => w.id === widgetId);
    if (!widget || !gridEl) return;
    e.preventDefault();
    e.stopPropagation();
    const m = gridMetrics();
    if (!m) return;
    resizeColWidth = m.colWidth;
    resizingId = widgetId;
    resizeStartW = widget.w;
    resizeStartH = widget.h;
    resizeStartX = e.clientX;
    resizeStartY = e.clientY;
    resizePreviewW = widget.w;
    resizePreviewH = widget.h;
  }

  $effect(() => {
    if (!resizingId) return;
    function onMove(e: PointerEvent) {
      const dx = e.clientX - resizeStartX;
      const dy = e.clientY - resizeStartY;
      resizePreviewW = Math.max(1, Math.min(12, Math.round(resizeStartW + dx / resizeColWidth)));
      resizePreviewH = Math.max(1, Math.round(resizeStartH + dy / ROW_HEIGHT));
    }
    function onUp() {
      if (resizingId) {
        dashboardsStore.updateWidget(dashboardId, resizingId, {
          w: resizePreviewW,
          h: resizePreviewH,
        });
      }
      resizingId = null;
    }
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    return () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };
  });

  // ── Name / icon editing ───────────────────────────────────────────────────

  let editName = $state('');
  let showIconPicker = $state(false);
  let iconBtnEl = $state<HTMLButtonElement | undefined>(undefined);
  let pickerX = $state(0);
  let pickerY = $state(0);

  $effect(() => {
    if (editMode && dashboard) editName = dashboard.name;
    if (!editMode) showIconPicker = false;
  });

  function openIconPicker() {
    if (!iconBtnEl) return;
    const r = iconBtnEl.getBoundingClientRect();
    pickerX = r.left;
    pickerY = r.bottom + 6;
    showIconPicker = true;
  }

  function commitName() {
    if (editName.trim() && editName.trim() !== dashboard?.name) {
      dashboardsStore.update(dashboardId, { name: editName.trim() });
    }
  }
</script>

{#if !dashboard}
  <div class="not-found">Dashboard not found.</div>
{:else}
  <div class="dashboard-panel">
    <!-- Header -->
    <div class="dash-header">
      {#if editMode}
        <!-- Icon picker trigger -->
        <button
          bind:this={iconBtnEl}
          class="dash-icon dash-icon--btn"
          title="Change icon"
          type="button"
          onclick={openIconPicker}
        >
          {@html dashboard.icon}
          <span class="icon-edit-hint">
            <svg width="9" height="9" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
          </span>
        </button>
        {#if showIconPicker}
          <div
            class="icon-picker-popover"
            role="dialog"
            aria-label="Choose icon"
            style:left="{pickerX}px"
            style:top="{pickerY}px"
            use:portal
          >
            <IconPicker
              value={dashboard.icon}
              onchange={(svg) => {
                dashboardsStore.update(dashboardId, { icon: svg });
                showIconPicker = false;
              }}
            />
          </div>
          <button
            class="icon-picker-backdrop"
            type="button"
            aria-label="Close icon picker"
            onclick={() => (showIconPicker = false)}
            use:portal
          ></button>
        {/if}
        <!-- Inline name edit -->
        <input
          class="rename-input"
          type="text"
          bind:value={editName}
          onblur={commitName}
          onkeydown={(e) => { if (e.key === 'Enter') { commitName(); (e.target as HTMLElement).blur(); } }}
        />
      {:else}
        <div class="dash-icon" aria-hidden="true">
          {@html dashboard.icon}
        </div>
        <span class="dash-name">{dashboard.name}</span>
      {/if}

      {#if connecting}
        <span class="connecting-badge">Connecting…</span>
      {/if}

      <div class="dash-header-actions">
        <button
          class="header-btn"
          class:header-btn--active={editMode}
          onclick={() => (editMode = !editMode)}
          title={editMode ? 'Done editing' : 'Edit layout, name and icon'}
          type="button"
        >
          {#if editMode}
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><polyline points="20 6 9 17 4 12"/></svg>
            Done
          {:else}
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
              <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
            </svg>
            Edit
          {/if}
        </button>
        {#if editMode}
          <button
            class="header-btn header-btn--accent"
            onclick={() => (addingWidget = true)}
            title="Add widget"
            type="button"
          >
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
            Add Widget
          </button>
        {/if}
      </div>
    </div>

    <!-- Widget grid -->
    <div bind:this={gridEl} class="widget-grid">
      {#if dashboard.widgets.length === 0}
        <div class="empty-grid">
          <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/></svg>
          <p>No widgets yet.</p>
          <button
            class="empty-add-btn"
            onclick={() => { editMode = true; addingWidget = true; }}
            type="button"
          >
            Add a widget
          </button>
        </div>
      {:else}
        {#each dashboard.widgets as widget (widget.id)}
          {@const isDragging = draggingId === widget.id}
          {@const isResizing = resizingId === widget.id}
          {@const slotX = isDragging ? dragPreviewX : widget.x}
          {@const slotY = isDragging ? dragPreviewY : widget.y}
          {@const slotW = isResizing ? resizePreviewW : widget.w}
          {@const slotH = isResizing ? resizePreviewH : widget.h}
          <div
            class="widget-slot"
            class:is-dragging={isDragging}
            class:is-resizing={isResizing}
            data-widget-id={widget.id}
            style="grid-column: {slotX} / span {slotW}; grid-row: {slotY} / span {slotH};"
          >
            <DashboardWidgetComponent
              {widget}
              {editMode}
              onEdit={() => (editingWidget = widget)}
              onDelete={() => handleDeleteWidget(widget.id)}
              onDragStart={(e) => startDrag(widget.id, e)}
              onResizeStart={(e) => startResize(widget.id, e)}
            />
          </div>
        {/each}
      {/if}
    </div>
  </div>
{/if}

{#if addingWidget}
  <WidgetEditor
    widget={null}
    onsave={handleAddWidget}
    oncancel={() => (addingWidget = false)}
  />
{/if}

{#if editingWidget}
  <WidgetEditor
    widget={editingWidget}
    onsave={handleEditWidget}
    oncancel={() => (editingWidget = null)}
  />
{/if}

<style>
  .dashboard-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    background: var(--color-bg-primary);
  }

  .not-found {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--color-text-muted);
    font-style: italic;
  }

  /* ── Header ──────────────────────────────────────────────────────────────── */

  .dash-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-3);
    padding: var(--spacing-3) var(--spacing-4);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    flex-shrink: 0;
  }

  .dash-icon {
    display: flex;
    align-items: center;
    color: var(--color-accent);
    flex-shrink: 0;
    width: 20px;
    height: 20px;
  }

  .dash-icon--btn {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-sm);
    border: 1px solid var(--color-border);
    background: var(--color-bg-primary);
    cursor: pointer;
    position: relative;
    flex-shrink: 0;
    color: var(--color-accent);
    transition: border-color var(--transition-fast), background var(--transition-fast);
  }

  .dash-icon--btn:hover {
    border-color: var(--color-accent);
    background: var(--color-accent-subtle, var(--color-bg-hover));
  }

  .icon-edit-hint {
    position: absolute;
    bottom: -3px;
    right: -3px;
    width: 12px;
    height: 12px;
    background: var(--color-accent);
    color: #fff;
    border-radius: 3px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  :global(.icon-picker-popover) {
    position: fixed;
    z-index: 200;
    background: var(--color-bg-overlay);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-lg);
    padding: var(--spacing-3);
    width: 220px;
  }

  :global(.icon-picker-backdrop) {
    position: fixed;
    inset: 0;
    z-index: 199;
    background: transparent;
    border: none;
    cursor: default;
  }

  .dash-name {
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .rename-input {
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-semibold);
    background: var(--color-bg-input, var(--color-bg-secondary));
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-sm);
    padding: 2px var(--spacing-2);
    color: var(--color-text-primary);
    outline: none;
    min-width: 120px;
    max-width: 260px;
  }

  .connecting-badge {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
  }

  .dash-header-actions {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
  }

  .header-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 5px 10px;
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    border-radius: var(--radius-md);
    border: 1px solid var(--color-border);
    background: var(--color-bg-primary);
    color: var(--color-text-secondary);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast),
      border-color var(--transition-fast);
  }

  .header-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .header-btn--active {
    background: var(--color-success-subtle);
    color: var(--color-success);
    border-color: var(--color-success);
  }

  .header-btn--accent {
    background: var(--color-accent);
    color: #fff;
    border-color: transparent;
  }

  .header-btn--accent:hover {
    opacity: 0.9;
    color: #fff;
  }

  /* ── Grid ────────────────────────────────────────────────────────────────── */

  .widget-grid {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-4);
    display: grid;
    grid-template-columns: repeat(12, 1fr);
    grid-auto-rows: 80px;
    gap: var(--spacing-3);
  }

  .widget-slot {
    transition: opacity var(--transition-fast), outline var(--transition-fast);
  }

  .widget-slot.is-dragging {
    opacity: 0.4;
  }

  .widget-slot.is-resizing {
    outline: 2px solid var(--color-accent);
    outline-offset: 2px;
    border-radius: var(--radius-md);
  }

  /* ── Empty state ─────────────────────────────────────────────────────────── */

  .empty-grid {
    grid-column: 1 / span 12;
    grid-row: 1 / span 8;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--spacing-3);
    color: var(--color-text-muted);
    text-align: center;
  }

  .empty-grid p {
    font-size: var(--font-size-sm);
    margin: 0;
  }

  .empty-add-btn {
    padding: 6px 16px;
    font-size: var(--font-size-sm);
    background: var(--color-accent);
    color: #fff;
    border: none;
    border-radius: var(--radius-md);
    cursor: pointer;
    font-weight: var(--font-weight-medium);
  }
</style>
