<!--
  DashboardPanel — the main view for a single dashboard.
  Auto-connects needed connections, displays widget grid.
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import type { DashboardWidget } from '$lib/types';
  import { useDashboards } from '$lib/stores/dashboards.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import DashboardWidgetComponent from './DashboardWidget.svelte';
  import WidgetEditor from './WidgetEditor.svelte';
  import IconPicker from './IconPicker.svelte';
  import { portal } from '$lib/actions/portal';
  import { exportDashboard } from '$lib/utils/dashboard-io';
  import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
  import EditIcon from '$lib/components/icons/EditIcon.svelte';
  import CheckIcon from '$lib/components/icons/CheckIcon.svelte';
  import DownloadIcon from '$lib/components/icons/DownloadIcon.svelte';
  import PlusIcon from '$lib/components/icons/PlusIcon.svelte';
  import GridIcon from '$lib/components/icons/GridIcon.svelte';
  import TrashIcon from '$lib/components/icons/TrashIcon.svelte';

  interface Props {
    dashboardId: string;
  }

  const { dashboardId }: Props = $props();

  const dashboardsStore = useDashboards();
  const connectionsStore = useConnections();

  const dashboard = $derived(dashboardsStore.getById(dashboardId));

  let editMode = $state(false);
  let addingWidget = $state(false);
  let editingWidget = $state<DashboardWidget | null>(null);
  let connecting = $state(false);

  // ── Auto-connect needed connections on mount ──────────────────────────────

  onMount(() => {
    autoConnect();
    void dashboardsStore.touchViewed(dashboardId);
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

  let confirmDeleteWidgetId = $state<string | null>(null);

  function handleDeleteWidget(widgetId: string) {
    confirmDeleteWidgetId = widgetId;
  }

  function confirmDeleteWidget() {
    if (!confirmDeleteWidgetId) return;
    dashboardsStore.deleteWidget(dashboardId, confirmDeleteWidgetId);
    confirmDeleteWidgetId = null;
  }

  // ── Dashboard variables ────────────────────────────────────────────────────

  function addVariable() {
    dashboardsStore.addVariable(dashboardId, { name: '', value: '' });
  }

  function commitVariableName(index: number, value: string) {
    dashboardsStore.updateVariable(dashboardId, index, { name: value.trim().toUpperCase() });
  }

  function commitVariableValue(index: number, value: string) {
    dashboardsStore.updateVariable(dashboardId, index, { value });
  }

  function removeVariable(index: number) {
    dashboardsStore.removeVariable(dashboardId, index);
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
  let dragSwapTargetId = $state<string | null>(null);
  let dragOffsetX = 0;
  let dragOffsetY = 0;
  let dragStartX = 0;
  let dragStartY = 0;
  let dragStartW = 0;
  let dragStartH = 0;

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
    dragStartX = widget.x;
    dragStartY = widget.y;
    dragStartW = widget.w;
    dragStartH = widget.h;
    dragSwapTargetId = null;
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

    // The widget whose own cells the pointer is actually over right now —
    // this is the swap target, not merely whatever the dragged rect covers.
    function widgetUnderPointer(cell: { x: number; y: number }) {
      return others.find(
        (o) => cell.x >= o.x && cell.x < o.x + o.w && cell.y >= o.y && cell.y < o.y + o.h,
      );
    }

    function onMove(e: PointerEvent) {
      const cell = pointerToCell(e);
      if (!cell) return;
      const nx = Math.max(1, Math.min(13 - widget!.w, cell.x - dragOffsetX));
      const ny = Math.max(1, cell.y - dragOffsetY);
      const hovered = widgetUnderPointer(cell);
      if (hovered) {
        // Swapping — keep the dragged widget's slot at its original position
        // rather than following the pointer onto the target.
        dragSwapTargetId = hovered.id;
        dragPreviewX = dragStartX;
        dragPreviewY = dragStartY;
      } else {
        dragSwapTargetId = null;
        if (!wouldOverlap(nx, ny)) {
          dragPreviewX = nx;
          dragPreviewY = ny;
        }
      }
    }
    function onUp() {
      if (draggingId) {
        if (dragSwapTargetId) {
          const target = dashboard?.widgets.find((w) => w.id === dragSwapTargetId);
          if (target) {
            // Swap both position and size so each widget exactly fills the
            // other's former slot — otherwise mismatched sizes would overlap.
            dashboardsStore.updateWidget(dashboardId, draggingId, {
              x: target.x,
              y: target.y,
              w: target.w,
              h: target.h,
            });
            dashboardsStore.updateWidget(dashboardId, dragSwapTargetId, {
              x: dragStartX,
              y: dragStartY,
              w: dragStartW,
              h: dragStartH,
            });
          }
        } else {
          dashboardsStore.updateWidget(dashboardId, draggingId, {
            x: dragPreviewX,
            y: dragPreviewY,
          });
        }
      }
      draggingId = null;
      dragSwapTargetId = null;
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
  let resizeStartW = 0,
    resizeStartH = 0;
  let resizeStartX = 0,
    resizeStartY = 0;
  let resizeColWidth = 0;
  let resizePreviewW = $state(0);
  let resizePreviewH = $state(0);
  let resizePushPreview = $state(new Map<string, { x: number; y: number }>());

  interface Rect {
    x: number;
    y: number;
    w: number;
    h: number;
  }

  function rectsOverlap(a: Rect, b: Rect) {
    return a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y;
  }

  // Grows `movedRect` in place of `movedId` and pushes any widgets it now
  // overlaps straight down, cascading until nothing overlaps.
  function resolvePushDown(widgets: DashboardWidget[], movedId: string, movedRect: Rect) {
    const rects = new Map<string, Rect>(
      widgets.map((w) => [w.id, { x: w.x, y: w.y, w: w.w, h: w.h }]),
    );
    rects.set(movedId, movedRect);

    let changed = true;
    while (changed) {
      changed = false;
      for (const [id, rect] of rects) {
        if (id === movedId) continue;
        for (const [otherId, other] of rects) {
          if (otherId === id) continue;
          if (rectsOverlap(rect, other)) {
            const newY = other.y + other.h;
            if (newY > rect.y) {
              rect.y = newY;
              changed = true;
            }
          }
        }
      }
    }
    return rects;
  }

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
    resizePushPreview = new Map();
  }

  $effect(() => {
    if (!resizingId) return;
    const widget = dashboard?.widgets.find((w) => w.id === resizingId);
    if (!widget) return;

    function onMove(e: PointerEvent) {
      const dx = e.clientX - resizeStartX;
      const dy = e.clientY - resizeStartY;
      resizePreviewW = Math.max(1, Math.min(12, Math.round(resizeStartW + dx / resizeColWidth)));
      resizePreviewH = Math.max(1, Math.round(resizeStartH + dy / ROW_HEIGHT));

      const movedRect: Rect = {
        x: widget!.x,
        y: widget!.y,
        w: resizePreviewW,
        h: resizePreviewH,
      };
      const resolved = resolvePushDown(dashboard!.widgets, resizingId!, movedRect);
      const positions = new Map<string, { x: number; y: number }>();
      for (const w of dashboard!.widgets) {
        if (w.id === resizingId) continue;
        const r = resolved.get(w.id)!;
        if (r.x !== w.x || r.y !== w.y) positions.set(w.id, { x: r.x, y: r.y });
      }
      resizePushPreview = positions;
    }
    function onUp() {
      if (resizingId) {
        dashboardsStore.updateWidget(dashboardId, resizingId, {
          w: resizePreviewW,
          h: resizePreviewH,
        });
        for (const [id, pos] of resizePushPreview) {
          dashboardsStore.updateWidget(dashboardId, id, { x: pos.x, y: pos.y });
        }
      }
      resizingId = null;
      resizePushPreview = new Map();
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

  function commitColor(color: string) {
    dashboardsStore.update(dashboardId, { color: color || null });
  }

  async function handleExport() {
    if (!dashboard) return;
    await exportDashboard(dashboard, (id) => connectionsStore.getById(id));
  }
</script>

{#if !dashboard}
  <div class="not-found">Dashboard not found.</div>
{:else}
  <div class="dashboard-panel" style:--dash-accent={dashboard.color}>
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
            <EditIcon width={9} height={9} strokeWidth={2.5} />
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
          onkeydown={(e) => {
            if (e.key === 'Enter') {
              commitName();
              (e.target as HTMLElement).blur();
            }
          }}
        />
        <!-- Accent colour -->
        <div class="color-row" title="Accent colour">
          <input
            class="color-input"
            type="color"
            value={dashboard.color ?? '#4f46e5'}
            oninput={(e) => commitColor(e.currentTarget.value)}
            aria-label="Dashboard accent colour"
          />
          {#if dashboard.color}
            <button
              type="button"
              class="color-clear"
              onclick={() => commitColor('')}
              aria-label="Clear accent colour">Clear</button
            >
          {/if}
        </div>
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
        {#if editMode}
          <button
            class="header-btn"
            onclick={handleExport}
            title="Export dashboard to file"
            type="button"
          >
            <DownloadIcon width={13} height={13} />
            Export
          </button>
        {/if}
        <button
          class="header-btn"
          class:header-btn--active={editMode}
          onclick={() => (editMode = !editMode)}
          title={editMode ? 'Done editing' : 'Edit layout, name and icon'}
          type="button"
        >
          {#if editMode}
            <CheckIcon width={13} height={13} strokeWidth={2} />
            Done
          {:else}
            <EditIcon width={13} height={13} strokeWidth={2} />
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
            <PlusIcon width={13} height={13} strokeWidth={2.2} />
            Add Widget
          </button>
        {/if}
      </div>
    </div>

    <!-- Variables bar -->
    {#if editMode}
      <div class="variables-bar">
        {#each dashboard.variables as variable, i}
          <div class="variable-control">
            <input
              class="variable-name-input"
              type="text"
              placeholder="NAME"
              value={variable.name}
              aria-label="Variable name"
              onchange={(e) => commitVariableName(i, e.currentTarget.value)}
            />
            <input
              class="variable-value-input"
              type="text"
              placeholder="value"
              value={variable.value}
              aria-label="{variable.name || 'Variable'} value"
              onchange={(e) => commitVariableValue(i, e.currentTarget.value)}
            />
            <button
              class="variable-remove-btn"
              type="button"
              onclick={() => removeVariable(i)}
              aria-label="Remove variable"
            >
              <TrashIcon width={11} height={11} strokeWidth={2} />
            </button>
          </div>
        {/each}
        <button class="variable-add-btn" type="button" onclick={addVariable}>
          <PlusIcon width={11} height={11} strokeWidth={2.2} /> Add variable
        </button>
      </div>
    {/if}

    <!-- Widget grid -->
    <div bind:this={gridEl} class="widget-grid">
      {#if dashboard.widgets.length === 0}
        <div class="empty-grid">
          <GridIcon width={40} height={40} strokeWidth={1.2} />
          <p>No widgets yet.</p>
          <button
            class="empty-add-btn"
            onclick={() => {
              editMode = true;
              addingWidget = true;
            }}
            type="button"
          >
            Add a widget
          </button>
        </div>
      {:else}
        {#each dashboard.widgets as widget (widget.id)}
          {@const isDragging = draggingId === widget.id}
          {@const isResizing = resizingId === widget.id}
          {@const isSwapTarget = dragSwapTargetId === widget.id}
          {@const pushed = resizingId ? resizePushPreview.get(widget.id) : undefined}
          {@const slotX = isDragging ? dragPreviewX : (pushed?.x ?? widget.x)}
          {@const slotY = isDragging ? dragPreviewY : (pushed?.y ?? widget.y)}
          {@const slotW = isResizing ? resizePreviewW : widget.w}
          {@const slotH = isResizing ? resizePreviewH : widget.h}
          <div
            class="widget-slot"
            class:is-dragging={isDragging}
            class:is-resizing={isResizing}
            class:is-pushed={!!pushed}
            class:is-swap-target={isSwapTarget}
            data-widget-id={widget.id}
            style="grid-column: {slotX} / span {slotW}; grid-row: {slotY} / span {slotH};"
          >
            <DashboardWidgetComponent
              {widget}
              dashboardId={dashboard.id}
              dashboardLastViewedAt={dashboard.lastViewedAt}
              dashboardVariables={dashboard?.variables ?? []}
              {editMode}
              onEdit={() => (editingWidget = widget)}
              onDuplicate={() => dashboardsStore.duplicateWidget(dashboardId, widget.id)}
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
    dashboardVariables={dashboard?.variables ?? []}
    accentColor={dashboard?.color}
    onsave={handleAddWidget}
    oncancel={() => (addingWidget = false)}
  />
{/if}

{#if editingWidget}
  <WidgetEditor
    widget={editingWidget}
    dashboardVariables={dashboard?.variables ?? []}
    accentColor={dashboard?.color}
    onsave={handleEditWidget}
    oncancel={() => (editingWidget = null)}
  />
{/if}

{#if confirmDeleteWidgetId}
  <ConfirmDialog
    title="Delete widget"
    message="Delete this widget? This cannot be undone."
    confirmText="Delete"
    cancelText="Cancel"
    danger
    onconfirm={confirmDeleteWidget}
    oncancel={() => (confirmDeleteWidgetId = null)}
  />
{/if}

<style>
  .dashboard-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
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
    color: var(--dash-accent, var(--color-accent));
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
    color: var(--dash-accent, var(--color-accent));
    transition:
      border-color var(--transition-fast),
      background var(--transition-fast);
  }

  .dash-icon--btn:hover {
    border-color: var(--dash-accent, var(--color-accent));
    background: color-mix(in srgb, var(--dash-accent, var(--color-accent)) 12%, var(--color-bg-hover));
  }

  .icon-edit-hint {
    position: absolute;
    bottom: -3px;
    right: -3px;
    width: 12px;
    height: 12px;
    background: var(--dash-accent, var(--color-accent));
    color: #fff;
    border-radius: 3px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .color-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    flex-shrink: 0;
  }

  .color-input {
    width: 28px;
    height: 28px;
    padding: 2px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-primary);
    cursor: pointer;
  }

  .color-clear {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    text-decoration: underline;
    white-space: nowrap;
  }

  .color-clear:hover {
    color: var(--color-text-secondary);
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
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    padding: 2px var(--spacing-2);
    color: var(--color-text-primary);
    outline: none;
    min-width: 120px;
    max-width: 260px;
  }

  .rename-input:focus {
    border-color: var(--dash-accent, var(--color-accent));
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
    background: var(--dash-accent, var(--color-accent));
    color: #fff;
    border-color: transparent;
  }

  .header-btn--accent:hover {
    background: var(--dash-accent, var(--color-accent));
    opacity: 0.9;
    color: #fff;
  }

  /* ── Variables bar ───────────────────────────────────────────────────────── */

  .variables-bar {
    display: flex;
    align-items: center;
    align-content: flex-start;
    flex-wrap: wrap;
    gap: var(--spacing-3);
    padding: var(--spacing-2) var(--spacing-4);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    flex-shrink: 0;
    max-height: 50%;
    overflow-y: auto;
  }

  .variable-control {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .variable-name-input,
  .variable-value-input {
    padding: 4px 8px;
    font-size: var(--font-size-xs);
    background: var(--color-bg-input, var(--color-bg-primary));
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--color-text-primary);
    outline: none;
  }

  .variable-name-input {
    width: 110px;
    font-family: var(--font-family-mono);
    text-transform: uppercase;
  }

  .variable-value-input {
    width: 140px;
  }

  .variable-name-input:focus,
  .variable-value-input:focus {
    border-color: var(--dash-accent, var(--color-accent));
  }

  .variable-remove-btn {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--color-text-muted);
    cursor: pointer;
  }

  .variable-remove-btn:hover {
    background: var(--color-danger-subtle);
    color: var(--color-danger);
  }

  .variable-add-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 10px;
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    background: var(--color-bg-primary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    color: var(--color-text-secondary);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .variable-add-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
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
    transition:
      opacity var(--transition-fast),
      outline var(--transition-fast);
  }

  .widget-slot.is-dragging {
    opacity: 0.4;
  }

  .widget-slot.is-resizing {
    outline: 2px solid var(--dash-accent, var(--color-accent));
    outline-offset: 2px;
    border-radius: var(--radius-md);
  }

  .widget-slot.is-pushed {
    transition:
      opacity var(--transition-fast),
      outline var(--transition-fast),
      grid-row var(--transition-fast);
  }

  .widget-slot.is-swap-target {
    outline: 2px dashed var(--dash-accent, var(--color-accent));
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
    background: var(--dash-accent, var(--color-accent));
    color: #fff;
    border: none;
    border-radius: var(--radius-md);
    cursor: pointer;
    font-weight: var(--font-weight-medium);
  }
</style>
