<!--
  DashboardWidget — renders a single dashboard widget.
  Fetches query data and displays it as count, table, bar chart, or line chart.
-->
<script lang="ts">
  import type { DashboardWidget } from '$lib/types';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import * as queryApi from '$lib/tauri/query';
  import { onMount } from 'svelte';
  import DragHandleIcon from '$lib/components/icons/DragHandleIcon.svelte';
  import EditIcon from '$lib/components/icons/EditIcon.svelte';
  import TrashIcon from '$lib/components/icons/TrashIcon.svelte';
  import ExternalLinkIcon from '$lib/components/icons/ExternalLinkIcon.svelte';
  import RotateCcwIcon from '$lib/components/icons/RotateCcwIcon.svelte';
  import CloseCircleIcon from '$lib/components/icons/CloseCircleIcon.svelte';
  import ResizeIcon from '$lib/components/icons/ResizeIcon.svelte';
  import Spinner from '$lib/components/ui/Spinner.svelte';

  interface Props {
    widget: DashboardWidget;
    editMode: boolean;
    onEdit: () => void;
    onDelete: () => void;
    onDragStart?: (_e: PointerEvent) => void;
    onResizeStart?: (_e: PointerEvent) => void;
  }

  const { widget, editMode, onEdit, onDelete, onDragStart, onResizeStart }: Props = $props();

  const connectionsStore = useConnections();
  const panelStore = usePanels();

  function openInEditor() {
    panelStore.openInFocused({
      kind: 'query_editor',
      connectionId: widget.connectionId,
      database: widget.database ?? undefined,
      initialSql: widget.sql,
    });
  }

  type Row = (string | number | boolean | null)[];

  let loading = $state(true);
  let error = $state<string | null>(null);
  let columns = $state<string[]>([]);
  let rows = $state<Row[]>([]);

  async function fetchData() {
    if (!widget.sql.trim()) {
      loading = false;
      return;
    }
    if (!connectionsStore.isActive(widget.connectionId)) {
      error = 'Connection not active';
      loading = false;
      return;
    }

    loading = true;
    error = null;
    try {
      const result = await queryApi.executeQuery(
        widget.connectionId,
        widget.sql,
        0,
        widget.displayType === 'count' ? 1 : widget.displayType === 'table' ? 200 : 100,
        widget.database,
      );
      if (result.error) {
        error = result.error;
      } else {
        columns = result.columns.map((c) => c.name);
        rows = result.rows;
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(fetchData);

  $effect(() => {
    void widget.sql;
    void widget.connectionId;
    void widget.database;
    if (connectionsStore.isActive(widget.connectionId)) {
      fetchData();
    }
  });

  // ── Single value display ─────────────────────────────────────────────────

  const countValue = $derived.by(() => {
    if (widget.displayType !== 'count') return null;
    const raw = rows[0]?.[0];
    if (raw === null || raw === undefined) return '—';
    const fmt = widget.singleValueFormat ?? 'auto';
    const str = String(raw);
    const n = Number(raw);
    const isNum = !isNaN(n) && str.trim() !== '';

    if (fmt === 'text') return str;

    if (fmt === 'boolean') {
      const lower = str.toLowerCase();
      if (lower === '1' || lower === 'true' || lower === 'yes') return 'Yes';
      if (lower === '0' || lower === 'false' || lower === 'no') return 'No';
      return str;
    }

    if (fmt === 'date' || fmt === 'datetime') {
      const d = new Date(str);
      if (!isNaN(d.getTime())) {
        return fmt === 'date' ? d.toLocaleDateString() : d.toLocaleString();
      }
      return str;
    }

    if (fmt === 'currency' && isNum) {
      const currency = widget.singleValueCurrency ?? 'GBP';
      return new Intl.NumberFormat(undefined, { style: 'currency', currency }).format(n);
    }

    if (fmt === 'percent' && isNum) {
      return new Intl.NumberFormat(undefined, {
        style: 'percent',
        maximumFractionDigits: 2,
      }).format(n);
    }

    if (fmt === 'number' && isNum) {
      return new Intl.NumberFormat().format(n);
    }

    if (fmt === 'number_compact' && isNum) {
      if (n >= 1_000_000_000) return `${(n / 1_000_000_000).toFixed(1)}B`;
      if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
      if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
      return String(n);
    }

    // auto
    if (isNum) {
      if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
      if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
      return new Intl.NumberFormat().format(n);
    }
    const d = new Date(str);
    if (!isNaN(d.getTime()) && /\d{4}-\d{2}-\d{2}/.test(str)) return d.toLocaleDateString();
    if (str.toLowerCase() === 'true' || str.toLowerCase() === 'false') {
      return str.toLowerCase() === 'true' ? 'Yes' : 'No';
    }
    return str;
  });

  // ── Chart data ───────────────────────────────────────────────────────────

  interface ChartPoint {
    label: string;
    value: number;
  }

  const chartData = $derived.by((): ChartPoint[] => {
    if (widget.displayType !== 'bar_chart' && widget.displayType !== 'line_chart') return [];
    return rows.map((row) => ({
      label: String(row[0] ?? ''),
      value: parseFloat(String(row[1] ?? '0')) || 0,
    }));
  });

  const chartMax = $derived(Math.max(...chartData.map((p) => p.value), 0) || 1);

  const CHART_H = 140;
  const CHART_PAD_LEFT = 36;
  const CHART_PAD_BOTTOM = 24;

  function barPoints(idx: number, total: number, chartWidth: number) {
    const usableW = chartWidth - CHART_PAD_LEFT;
    const barW = Math.max(4, usableW / total - 4);
    const x = CHART_PAD_LEFT + idx * (usableW / total) + (usableW / total - barW) / 2;
    const barH = ((chartData[idx]?.value ?? 0) / chartMax) * (CHART_H - CHART_PAD_BOTTOM - 10);
    const y = CHART_H - CHART_PAD_BOTTOM - barH;
    return { x, y, w: barW, h: barH };
  }

  function linePath(chartWidth: number): string {
    if (chartData.length === 0) return '';
    const usableW = chartWidth - CHART_PAD_LEFT;
    const pts = chartData.map((p, i) => {
      const x = CHART_PAD_LEFT + (i / Math.max(chartData.length - 1, 1)) * usableW;
      const y =
        CHART_H - CHART_PAD_BOTTOM - (p.value / chartMax) * (CHART_H - CHART_PAD_BOTTOM - 10);
      return `${x},${y}`;
    });
    return `M ${pts.join(' L ')}`;
  }

  let chartEl = $state<SVGSVGElement | undefined>(undefined);
  let chartWidth = $state(300);

  $effect(() => {
    if (!chartEl) return;
    const ro = new ResizeObserver((entries) => {
      chartWidth = entries[0]?.contentRect.width ?? 300;
    });
    ro.observe(chartEl);
    return () => ro.disconnect();
  });

  function formatLabel(s: string, maxLen = 8): string {
    return s.length > maxLen ? s.slice(0, maxLen - 1) + '…' : s;
  }

  function formatYTick(v: number): string {
    if (v >= 1_000_000) return `${(v / 1_000_000).toFixed(0)}M`;
    if (v >= 1_000) return `${(v / 1_000).toFixed(0)}K`;
    return String(Math.round(v));
  }

  // ── Countdown ────────────────────────────────────────────────────────────

  let now = $state(Date.now());
  let countdownInterval: ReturnType<typeof setInterval> | null = null;

  $effect(() => {
    if (widget.displayType === 'countdown') {
      now = Date.now();
      countdownInterval = setInterval(() => {
        now = Date.now();
      }, 1000);
      return () => {
        if (countdownInterval) clearInterval(countdownInterval);
      };
    } else {
      if (countdownInterval) {
        clearInterval(countdownInterval);
        countdownInterval = null;
      }
    }
  });

  interface CountdownUnit {
    value: number;
    label: string;
    pad: boolean;
  }
  interface CountdownParts {
    units: CountdownUnit[];
    expired: boolean;
    targetLabel: string;
  }

  const countdownValue = $derived.by((): CountdownParts | null => {
    if (widget.displayType !== 'countdown') return null;
    const raw = rows[0]?.[0];
    if (raw === null || raw === undefined) return null;
    const target = new Date(String(raw));
    if (isNaN(target.getTime())) return null;
    const expired = target.getTime() <= now;
    const base = expired ? target : new Date(now);
    const end = expired ? new Date(now) : target;

    let years = end.getFullYear() - base.getFullYear();
    let months = end.getMonth() - base.getMonth();
    let days = end.getDate() - base.getDate();
    let hours = end.getHours() - base.getHours();
    let minutes = end.getMinutes() - base.getMinutes();
    let seconds = end.getSeconds() - base.getSeconds();

    if (seconds < 0) {
      seconds += 60;
      minutes--;
    }
    if (minutes < 0) {
      minutes += 60;
      hours--;
    }
    if (hours < 0) {
      hours += 24;
      days--;
    }
    if (days < 0) {
      const prevMonthDays = new Date(end.getFullYear(), end.getMonth(), 0).getDate();
      days += prevMonthDays;
      months--;
    }
    if (months < 0) {
      months += 12;
      years--;
    }

    const all: CountdownUnit[] = [
      { value: years, label: years === 1 ? 'yr' : 'yrs', pad: false },
      { value: months, label: months === 1 ? 'mo' : 'mos', pad: false },
      { value: days, label: days === 1 ? 'day' : 'days', pad: false },
      { value: hours, label: 'hrs', pad: true },
      { value: minutes, label: 'min', pad: true },
      { value: seconds, label: 'sec', pad: true },
    ];

    const firstNonZero = all.findIndex((u) => u.value > 0);
    const units =
      firstNonZero === -1 ? [{ value: 0, label: 'sec', pad: false }] : all.slice(firstNonZero);

    return { units, expired, targetLabel: target.toLocaleString() };
  });
</script>

<div class="widget" style="--w: {widget.w}; --h: {widget.h};">
  <div class="widget-header">
    {#if editMode}
      <button class="drag-handle" aria-label="Drag to reorder" onpointerdown={onDragStart}>
        <DragHandleIcon />
      </button>
    {/if}
    <span class="widget-title">{widget.title}</span>
    <div class="widget-actions">
      {#if editMode}
        <button class="action-btn" onclick={onEdit} title="Edit widget" type="button">
          <EditIcon width={12} height={12} strokeWidth={2} />
        </button>
        <button
          class="action-btn action-btn--danger"
          onclick={onDelete}
          title="Delete widget"
          type="button"
        >
          <TrashIcon width={12} height={12} strokeWidth={2} />
        </button>
      {:else}
        <button
          class="action-btn"
          onclick={openInEditor}
          title="Open in query editor"
          type="button"
        >
          <ExternalLinkIcon width={12} height={12} strokeWidth={2} />
        </button>
        <button class="refresh-btn" onclick={fetchData} title="Refresh" type="button">
          <RotateCcwIcon width={11} height={11} strokeWidth={2} />
        </button>
      {/if}
    </div>
  </div>

  <div class="widget-body">
    {#if loading}
      <div class="state-placeholder">
        <Spinner size={16} label="Loading" />
      </div>
    {:else if error}
      <div class="state-placeholder state-placeholder--error">
        <CloseCircleIcon width={16} height={16} />
        <span>{error}</span>
      </div>
    {:else if widget.displayType === 'count'}
      <div class="count-display">
        <span class="count-value">{countValue}</span>
        {#if columns[0]}
          <span class="count-label">{columns[0]}</span>
        {/if}
      </div>
    {:else if widget.displayType === 'table'}
      {#if rows.length === 0}
        <div class="state-placeholder">No data</div>
      {:else}
        <div class="table-wrap">
          <table class="data-table">
            <thead>
              <tr>
                {#each columns as col}
                  <th>{col}</th>
                {/each}
              </tr>
            </thead>
            <tbody>
              {#each rows as row}
                <tr>
                  {#each row as cell}
                    <td>{cell === null ? 'NULL' : String(cell)}</td>
                  {/each}
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    {:else if widget.displayType === 'bar_chart' || widget.displayType === 'line_chart'}
      {#if chartData.length === 0}
        <div class="state-placeholder">No data</div>
      {:else}
        <div class="chart-wrap">
          <svg
            bind:this={chartEl}
            class="chart-svg"
            width="100%"
            height={CHART_H}
            viewBox="0 0 {chartWidth} {CHART_H}"
            aria-label="{widget.title} chart"
          >
            <!-- Y-axis ticks -->
            {#each [0, 0.25, 0.5, 0.75, 1] as frac}
              {@const y = CHART_H - CHART_PAD_BOTTOM - frac * (CHART_H - CHART_PAD_BOTTOM - 10)}
              <line x1={CHART_PAD_LEFT} y1={y} x2={chartWidth} y2={y} class="grid-line" />
              <text x={CHART_PAD_LEFT - 4} y={y + 4} class="axis-label" text-anchor="end">
                {formatYTick(frac * chartMax)}
              </text>
            {/each}

            {#if widget.displayType === 'bar_chart'}
              {#each chartData as point, i}
                {@const b = barPoints(i, chartData.length, chartWidth)}
                <rect x={b.x} y={b.y} width={b.w} height={b.h} class="bar" rx="2" />
                <text
                  x={b.x + b.w / 2}
                  y={CHART_H - CHART_PAD_BOTTOM + 14}
                  class="axis-label"
                  text-anchor="middle"
                >
                  {formatLabel(point.label)}
                </text>
              {/each}
            {:else}
              <!-- Line chart area fill -->
              {#if chartData.length > 1}
                {@const pathD = linePath(chartWidth)}
                {@const firstX = CHART_PAD_LEFT}
                {@const lastX = chartWidth}
                {@const baseY = CHART_H - CHART_PAD_BOTTOM}
                <path d="{pathD} L {lastX},{baseY} L {firstX},{baseY} Z" class="line-area" />
                <path d={pathD} class="line-path" />
              {/if}
              <!-- Data points -->
              {#each chartData as point, i}
                {@const x =
                  CHART_PAD_LEFT +
                  (i / Math.max(chartData.length - 1, 1)) * (chartWidth - CHART_PAD_LEFT)}
                {@const y =
                  CHART_H -
                  CHART_PAD_BOTTOM -
                  (point.value / chartMax) * (CHART_H - CHART_PAD_BOTTOM - 10)}
                <circle cx={x} cy={y} r="3" class="line-dot" />
                {#if i % Math.ceil(chartData.length / 8) === 0}
                  <text
                    {x}
                    y={CHART_H - CHART_PAD_BOTTOM + 14}
                    class="axis-label"
                    text-anchor="middle"
                  >
                    {formatLabel(point.label)}
                  </text>
                {/if}
              {/each}
            {/if}

            <!-- X axis -->
            <line
              x1={CHART_PAD_LEFT}
              y1={CHART_H - CHART_PAD_BOTTOM}
              x2={chartWidth}
              y2={CHART_H - CHART_PAD_BOTTOM}
              class="axis-line"
            />
          </svg>
        </div>
      {/if}
    {:else if widget.displayType === 'countdown'}
      {#if !countdownValue}
        <div class="state-placeholder">No date value</div>
      {:else if countdownValue.expired}
        <div class="countdown-display">
          <div class="countdown-expired">Expired</div>
          <div class="countdown-target">{countdownValue.targetLabel}</div>
        </div>
      {:else}
        <div class="countdown-display">
          <div class="countdown-parts">
            {#each countdownValue.units as unit, i}
              {#if i > 0}<span class="countdown-sep">:</span>{/if}
              <div class="countdown-part">
                <span class="countdown-num"
                  >{unit.pad ? String(unit.value).padStart(2, '0') : unit.value}</span
                >
                <span class="countdown-unit">{unit.label}</span>
              </div>
            {/each}
          </div>
          <div class="countdown-target">{countdownValue.targetLabel}</div>
        </div>
      {/if}
    {/if}
  </div>

  {#if editMode}
    <button
      class="resize-handle"
      aria-label="Resize widget"
      onpointerdown={onResizeStart}
      type="button"
    >
      <ResizeIcon />
    </button>
  {/if}
</div>

<style>
  .widget {
    height: 100%;
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    position: relative;
    transition: box-shadow var(--transition-fast);
  }

  .resize-handle {
    position: absolute;
    bottom: 0;
    right: 0;
    width: 20px;
    height: 20px;
    display: grid;
    place-items: center;
    background: none;
    border: none;
    cursor: nwse-resize;
    color: var(--color-text-muted);
    border-radius: var(--radius-sm) 0 var(--radius-sm) 0;
    opacity: 0;
    transition:
      opacity var(--transition-fast),
      color var(--transition-fast);
    z-index: 10;
    padding: 0;
  }

  .widget:hover .resize-handle {
    opacity: 1;
  }

  .resize-handle:hover {
    color: var(--color-accent);
  }

  .widget:hover {
    box-shadow: var(--shadow-sm);
  }

  .widget-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-primary);
    flex-shrink: 0;
    min-height: 36px;
  }

  .drag-handle {
    color: var(--color-text-muted);
    cursor: grab;
    display: flex;
    align-items: center;
    padding: 2px;
    border-radius: var(--radius-sm);
    background: none;
    border: none;
    flex-shrink: 0;
  }

  .drag-handle:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-secondary);
  }

  .widget-title {
    flex: 1;
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .widget-actions {
    display: flex;
    align-items: center;
    gap: 2px;
    flex-shrink: 0;
  }

  .action-btn,
  .refresh-btn {
    width: 24px;
    height: 24px;
    display: grid;
    place-items: center;
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--color-text-muted);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .action-btn:hover,
  .refresh-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .action-btn--danger:hover {
    background: var(--color-danger-subtle);
    color: var(--color-danger);
  }

  .widget-body {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .widget-body :global(*) {
    -webkit-user-select: text;
    user-select: text;
  }

  /* ── State placeholders ──────────────────────────────────────────────────── */

  .state-placeholder {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--spacing-2);
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    font-style: italic;
  }

  .state-placeholder--error {
    color: var(--color-danger);
    font-style: normal;
    font-size: var(--font-size-xs);
    padding: var(--spacing-2);
    text-align: center;
  }

  /* ── Count ───────────────────────────────────────────────────────────────── */

  .count-display {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--spacing-1);
    padding: var(--spacing-4);
  }

  .count-value {
    font-size: 3rem;
    font-weight: 700;
    color: var(--color-accent);
    line-height: 1;
    letter-spacing: -0.02em;
    text-align: center;
  }

  .count-label {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    text-align: center;
  }

  /* ── Countdown ───────────────────────────────────────────────────────────── */

  .countdown-display {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--spacing-3);
    padding: var(--spacing-4);
  }

  .countdown-parts {
    display: flex;
    align-items: flex-end;
    gap: var(--spacing-2);
  }

  .countdown-part {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }

  .countdown-num {
    font-size: 2.2rem;
    font-weight: 700;
    color: var(--color-accent);
    line-height: 1;
    letter-spacing: -0.02em;
    font-variant-numeric: tabular-nums;
  }

  .countdown-unit {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .countdown-sep {
    font-size: 1.8rem;
    font-weight: 300;
    color: var(--color-text-muted);
    line-height: 1;
    padding-bottom: 18px;
  }

  .countdown-target {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    text-align: center;
  }

  .countdown-expired {
    font-size: 2rem;
    font-weight: 700;
    color: var(--color-text-muted);
    letter-spacing: -0.02em;
  }

  /* ── Table ───────────────────────────────────────────────────────────────── */

  .table-wrap {
    flex: 1;
    overflow: auto;
  }

  .data-table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--font-size-xs);
  }

  .data-table th {
    position: sticky;
    top: 0;
    background: var(--color-bg-tertiary);
    padding: 5px var(--spacing-2);
    text-align: left;
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-secondary);
    border-bottom: 1px solid var(--color-border);
    white-space: nowrap;
  }

  .data-table td {
    padding: 4px var(--spacing-2);
    color: var(--color-text-primary);
    border-bottom: 1px solid var(--color-border);
    white-space: nowrap;
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
    font-family: var(--font-family-mono);
    font-size: 11.5px;
  }

  .data-table tr:last-child td {
    border-bottom: none;
  }

  /* ── Charts ──────────────────────────────────────────────────────────────── */

  .chart-wrap {
    flex: 1;
    padding: var(--spacing-2) var(--spacing-2) 0;
    min-height: 0;
    overflow: hidden;
  }

  .chart-svg {
    display: block;
    overflow: visible;
  }

  .grid-line {
    stroke: var(--color-border);
    stroke-width: 1;
  }

  .axis-line {
    stroke: var(--color-border-strong, var(--color-border));
    stroke-width: 1.5;
  }

  .axis-label {
    font-size: 9px;
    fill: var(--color-text-muted);
    font-family: var(--font-family-ui);
  }

  .bar {
    fill: var(--color-accent);
    opacity: 0.8;
    transition: opacity var(--transition-fast);
  }

  .bar:hover {
    opacity: 1;
  }

  .line-path {
    fill: none;
    stroke: var(--color-accent);
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .line-area {
    fill: var(--color-accent);
    opacity: 0.1;
  }

  .line-dot {
    fill: var(--color-accent);
    stroke: var(--color-bg-primary);
    stroke-width: 1.5;
  }
</style>
