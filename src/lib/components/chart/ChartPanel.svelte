<!--
  ChartPanel — live chart builder shown in the right sidebar and as a full tab.
  Reads from the chartData store, which is updated by TableBrowser and QueryEditor.
-->
<script lang="ts">
  import { onMount, onDestroy, tick, untrack } from 'svelte';
  import { Chart, registerables } from 'chart.js';
  import type { ChartConfiguration, ChartType as ChartJsType } from 'chart.js';
  import { useChartData } from '$lib/stores/chartData.svelte';
  import Select from '$lib/components/ui/Select.svelte';
  import { save } from '@tauri-apps/plugin-dialog';
  import { writeFile } from '@tauri-apps/plugin-fs';
  import { useToast } from '$lib/stores/toast.svelte';
  import type { ColumnMeta } from '$lib/types';

  Chart.register(...registerables);

  type CellValue = string | number | boolean | null;

  type ChartTypeKey =
    | 'bar-vertical'
    | 'bar-horizontal'
    | 'line'
    | 'area'
    | 'pie'
    | 'scatter'
    | 'bubble'
    | 'stacked-bar'
    | 'grouped-bar'
    | 'histogram'
    | 'dot-plot'
    | 'dual-axis'
    | 'heatmap'
    | 'box-plot';

  interface Props {
    compact?: boolean;
    onOpenAsTab?: (_config: SerializedChartConfig) => void;
    initialConfig?: SerializedChartConfig | null;
  }

  export interface SerializedChartConfig {
    title: string;
    chartType: ChartTypeKey;
    xColumn: string;
    yColumn: string;
    y2Column: string;
    colorColumn: string;
    sizeColumn: string;
    useSelectedOnly: boolean;
    bins: number;
  }

  const { compact = false, onOpenAsTab, initialConfig = null }: Props = $props();

  const chartStore = useChartData();
  const toast = useToast();

  const CHART_TYPES: { key: ChartTypeKey; label: string }[] = [
    { key: 'bar-vertical', label: 'Vertical Bar' },
    { key: 'bar-horizontal', label: 'Horizontal Bar' },
    { key: 'stacked-bar', label: 'Stacked Bar' },
    { key: 'grouped-bar', label: 'Grouped Bar' },
    { key: 'line', label: 'Line' },
    { key: 'area', label: 'Area' },
    { key: 'dual-axis', label: 'Dual Axis' },
    { key: 'pie', label: 'Pie' },
    { key: 'scatter', label: 'Scatter' },
    { key: 'bubble', label: 'Bubble' },
    { key: 'dot-plot', label: 'Dot Plot' },
    { key: 'histogram', label: 'Histogram' },
    { key: 'box-plot', label: 'Box Plot' },
    { key: 'heatmap', label: 'Heatmap' },
  ];

  const _cfg = untrack(() => initialConfig);
  let chartType = $state<ChartTypeKey>(_cfg?.chartType ?? 'bar-vertical');
  let xColumn = $state<string>(_cfg?.xColumn ?? '');
  let yColumn = $state<string>(_cfg?.yColumn ?? '');
  let y2Column = $state<string>(_cfg?.y2Column ?? '');
  let colorColumn = $state<string>(_cfg?.colorColumn ?? '');
  let sizeColumn = $state<string>(_cfg?.sizeColumn ?? '');
  let useSelectedOnly = $state<boolean>(_cfg?.useSelectedOnly ?? false);
  let bins = $state<number>(_cfg?.bins ?? 20);

  let canvasEl = $state<HTMLCanvasElement | null>(null);
  let heatmapEl = $state<HTMLCanvasElement | null>(null);
  let chartInstance: Chart | null = null;
  let renderKey = $state(0);

  const sourceData = $derived(chartStore.current);

  const columns = $derived(sourceData?.columns ?? []);
  const allRows = $derived(sourceData?.allRows ?? []);
  const selectedRows = $derived(sourceData?.selectedRows ?? []);
  const activeRows = $derived(
    useSelectedOnly && selectedRows.length > 0 ? selectedRows : allRows,
  );

  const numericColumns = $derived(
    columns.filter((c) => isNumericType(c.dataType)),
  );
  const allColumns = $derived(columns);

  const chartTypeOptions = $derived(CHART_TYPES.map((ct) => ({ value: ct.key, label: ct.label })));
  const allColumnOptions = $derived(allColumns.map((c) => ({ value: c.name, label: c.name })));
  const numericColumnOptions = $derived(numericColumns.map((c) => ({ value: c.name, label: c.name })));

  function isNumericType(dt: string): boolean {
    const t = dt.toLowerCase();
    return (
      t.includes('int') ||
      t.includes('float') ||
      t.includes('double') ||
      t.includes('decimal') ||
      t.includes('numeric') ||
      t.includes('real') ||
      t.includes('number') ||
      t === 'bigint' ||
      t === 'smallint' ||
      t === 'tinyint'
    );
  }

  function colIndex(name: string): number {
    return columns.findIndex((c) => c.name === name);
  }

  function colValues(rows: CellValue[][], name: string): CellValue[] {
    const idx = colIndex(name);
    if (idx < 0) return [];
    return rows.map((r) => r[idx]);
  }

  function toNumber(v: CellValue): number | null {
    if (v === null || v === '') return null;
    const n = Number(v);
    return isNaN(n) ? null : n;
  }

  // ── Palette ──────────────────────────────────────────────────────────────────

  const PALETTE = [
    '#6366f1',
    '#10b981',
    '#f59e0b',
    '#ef4444',
    '#3b82f6',
    '#8b5cf6',
    '#ec4899',
    '#14b8a6',
    '#f97316',
    '#84cc16',
    '#06b6d4',
    '#e11d48',
  ];

  function paletteColor(i: number, alpha = 1): string {
    const hex = PALETTE[i % PALETTE.length];
    if (alpha === 1) return hex;
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    return `rgba(${r},${g},${b},${alpha})`;
  }

  // ── Smart defaults ───────────────────────────────────────────────────────────

  $effect(() => {
    if (!sourceData) return;
    const cols = sourceData.columns;

    if (!xColumn || !cols.find((c) => c.name === xColumn)) {
      const firstNonNum = cols.find((c) => !isNumericType(c.dataType));
      xColumn = firstNonNum?.name ?? cols[0]?.name ?? '';
    }
    if (!yColumn || !cols.find((c) => c.name === yColumn)) {
      const firstNum = cols.find((c) => isNumericType(c.dataType));
      yColumn = firstNum?.name ?? cols[1]?.name ?? cols[0]?.name ?? '';
    }
    if (y2Column && !cols.find((c) => c.name === y2Column)) y2Column = '';
    if (colorColumn && !cols.find((c) => c.name === colorColumn)) colorColumn = '';
    if (sizeColumn && !cols.find((c) => c.name === sizeColumn)) sizeColumn = '';
  });

  // ── Chart rebuild ────────────────────────────────────────────────────────────

  $effect(() => {
    // Reactive dependencies — touching them ensures we re-render when they change
    void chartType;
    void xColumn;
    void yColumn;
    void y2Column;
    void colorColumn;
    void sizeColumn;
    void useSelectedOnly;
    void bins;
    void activeRows;
    renderKey;

    if (chartType === 'heatmap') {
      destroyChart();
      tick().then(() => renderHeatmap());
      return;
    }

    if (!canvasEl) return;
    tick().then(() => buildChart());
  });

  function destroyChart() {
    if (chartInstance) {
      chartInstance.destroy();
      chartInstance = null;
    }
  }

  onDestroy(() => destroyChart());

  function buildChart() {
    if (!canvasEl) return;
    destroyChart();

    const config = makeChartConfig();
    if (!config) return;

    chartInstance = new Chart(canvasEl, config);
  }

  function makeChartConfig(): ChartConfiguration | null {
    const rows = activeRows;
    if (rows.length === 0) return null;

    switch (chartType) {
      case 'bar-vertical':
        return makeBarConfig(rows, false, false);
      case 'bar-horizontal':
        return makeBarConfig(rows, true, false);
      case 'stacked-bar':
        return makeStackedBarConfig(rows);
      case 'grouped-bar':
        return makeGroupedBarConfig(rows);
      case 'line':
        return makeLineConfig(rows, false);
      case 'area':
        return makeLineConfig(rows, true);
      case 'pie':
        return makePieConfig(rows);
      case 'scatter':
        return makeScatterConfig(rows);
      case 'bubble':
        return makeBubbleConfig(rows);
      case 'dot-plot':
        return makeDotPlotConfig(rows);
      case 'histogram':
        return makeHistogramConfig(rows);
      case 'dual-axis':
        return makeDualAxisConfig(rows);
      case 'box-plot':
        return makeBoxPlotConfig(rows);
      default:
        return null;
    }
  }

  function getLabels(rows: CellValue[][]): string[] {
    const vals = colValues(rows, xColumn);
    return vals.map((v) => (v === null ? 'NULL' : String(v)));
  }

  function getYValues(rows: CellValue[][]): (number | null)[] {
    return colValues(rows, yColumn).map(toNumber);
  }

  const commonOptions = {
    responsive: true,
    maintainAspectRatio: false,
    plugins: {
      legend: { labels: { color: 'var(--color-text-secondary)', font: { size: 11 } } },
      tooltip: { mode: 'index' as const, intersect: false },
    },
    scales: {
      x: {
        ticks: { color: 'var(--color-text-muted)', font: { size: 11 } },
        grid: { color: 'rgba(127,127,127,0.1)' },
      },
      y: {
        ticks: { color: 'var(--color-text-muted)', font: { size: 11 } },
        grid: { color: 'rgba(127,127,127,0.1)' },
      },
    },
  } as const;

  function makeBarConfig(rows: CellValue[][], horizontal: boolean, _stacked: boolean): ChartConfiguration {
    const labels = getLabels(rows);
    const yVals = getYValues(rows);

    return {
      type: 'bar',
      data: {
        labels,
        datasets: [
          {
            label: yColumn,
            data: yVals,
            backgroundColor: yVals.map((_, i) => paletteColor(i, 0.75)),
            borderColor: yVals.map((_, i) => paletteColor(i)),
            borderWidth: 1,
          },
        ],
      },
      options: {
        ...commonOptions,
        indexAxis: horizontal ? 'y' : 'x',
      },
    };
  }

  function makeStackedBarConfig(rows: CellValue[][]): ChartConfiguration {
    const xVals = colValues(rows, xColumn);
    const labels = [...new Set(xVals.map((v) => (v === null ? 'NULL' : String(v))))];

    // Group by colorColumn if set, else just single series
    if (colorColumn) {
      const seriesVals = colValues(rows, colorColumn);
      const seriesKeys = [...new Set(seriesVals.map((v) => (v === null ? 'NULL' : String(v))))];

      const datasets = seriesKeys.map((key, ki) => {
        const data = labels.map((lbl) => {
          let sum = 0;
          for (let i = 0; i < rows.length; i++) {
            const xVal = xVals[i] === null ? 'NULL' : String(xVals[i]);
            const sVal = seriesVals[i] === null ? 'NULL' : String(seriesVals[i]);
            if (xVal === lbl && sVal === key) {
              const n = toNumber(rows[i][colIndex(yColumn)]);
              if (n !== null) sum += n;
            }
          }
          return sum;
        });
        return {
          label: key,
          data,
          backgroundColor: paletteColor(ki, 0.75),
          borderColor: paletteColor(ki),
          borderWidth: 1,
          stack: 'stack',
        };
      });

      return {
        type: 'bar',
        data: { labels, datasets },
        options: { ...commonOptions, scales: { ...commonOptions.scales, x: { ...commonOptions.scales.x, stacked: true }, y: { ...commonOptions.scales.y, stacked: true } } },
      };
    }

    return makeBarConfig(rows, false, true);
  }

  function makeGroupedBarConfig(rows: CellValue[][]): ChartConfiguration {
    if (!colorColumn) return makeBarConfig(rows, false, false);

    const xVals = colValues(rows, xColumn);
    const seriesVals = colValues(rows, colorColumn);
    const labels = [...new Set(xVals.map((v) => (v === null ? 'NULL' : String(v))))];
    const seriesKeys = [...new Set(seriesVals.map((v) => (v === null ? 'NULL' : String(v))))];

    const datasets = seriesKeys.map((key, ki) => {
      const data = labels.map((lbl) => {
        let sum = 0;
        let count = 0;
        for (let i = 0; i < rows.length; i++) {
          const xVal = xVals[i] === null ? 'NULL' : String(xVals[i]);
          const sVal = seriesVals[i] === null ? 'NULL' : String(seriesVals[i]);
          if (xVal === lbl && sVal === key) {
            const n = toNumber(rows[i][colIndex(yColumn)]);
            if (n !== null) { sum += n; count++; }
          }
        }
        return count > 0 ? sum : null;
      });
      return {
        label: key,
        data,
        backgroundColor: paletteColor(ki, 0.75),
        borderColor: paletteColor(ki),
        borderWidth: 1,
      };
    });

    return {
      type: 'bar',
      data: { labels, datasets },
      options: commonOptions,
    };
  }

  function makeLineConfig(rows: CellValue[][], fill: boolean): ChartConfiguration {
    const labels = getLabels(rows);
    const yVals = getYValues(rows);
    return {
      type: 'line',
      data: {
        labels,
        datasets: [
          {
            label: yColumn,
            data: yVals,
            borderColor: paletteColor(0),
            backgroundColor: fill ? paletteColor(0, 0.15) : 'transparent',
            fill,
            tension: 0.3,
            pointRadius: 3,
          },
        ],
      },
      options: commonOptions,
    };
  }

  function makePieConfig(rows: CellValue[][]): ChartConfiguration {
    const labels = getLabels(rows);
    const yVals = getYValues(rows);
    return {
      type: 'pie',
      data: {
        labels,
        datasets: [
          {
            data: yVals,
            backgroundColor: yVals.map((_, i) => paletteColor(i, 0.8)),
            borderColor: yVals.map((_, i) => paletteColor(i)),
            borderWidth: 1,
          },
        ],
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: {
          legend: { labels: { color: 'var(--color-text-secondary)', font: { size: 11 } } },
        },
      },
    };
  }

  function makeScatterConfig(rows: CellValue[][]): ChartConfiguration {
    const xIdx = colIndex(xColumn);
    const yIdx = colIndex(yColumn);
    const data = rows
      .map((r) => ({ x: toNumber(r[xIdx]), y: toNumber(r[yIdx]) }))
      .filter((p): p is { x: number; y: number } => p.x !== null && p.y !== null);

    return {
      type: 'scatter',
      data: {
        datasets: [
          {
            label: `${xColumn} vs ${yColumn}`,
            data,
            backgroundColor: paletteColor(0, 0.6),
            pointRadius: 4,
          },
        ],
      },
      options: commonOptions,
    };
  }

  function makeBubbleConfig(rows: CellValue[][]): ChartConfiguration {
    const xIdx = colIndex(xColumn);
    const yIdx = colIndex(yColumn);
    const sIdx = sizeColumn ? colIndex(sizeColumn) : -1;

    const data = rows
      .map((r) => {
        const x = toNumber(r[xIdx]);
        const y = toNumber(r[yIdx]);
        const rawR = sIdx >= 0 ? toNumber(r[sIdx]) : 10;
        return x !== null && y !== null ? { x, y, r: Math.max(2, Math.min(30, (rawR ?? 10))) } : null;
      })
      .filter((p): p is NonNullable<typeof p> => p !== null);

    return {
      type: 'bubble',
      data: {
        datasets: [{ label: yColumn, data, backgroundColor: paletteColor(0, 0.5), borderColor: paletteColor(0) }],
      },
      options: commonOptions,
    };
  }

  function makeDotPlotConfig(rows: CellValue[][]): ChartConfiguration {
    const labels = getLabels(rows);
    const yVals = getYValues(rows);
    return {
      type: 'scatter',
      data: {
        datasets: [
          {
            label: yColumn,
            data: labels.map((_, i) => ({ x: i, y: yVals[i] })).filter((p): p is { x: number; y: number } => p.y !== null),
            backgroundColor: paletteColor(0, 0.7),
            pointRadius: 5,
          },
        ],
      },
      options: {
        ...commonOptions,
        scales: {
          x: {
            type: 'linear',
            ticks: {
              color: 'var(--color-text-muted)',
              font: { size: 11 },
              callback: (val) => labels[val as number] ?? val,
            },
            grid: { color: 'rgba(127,127,127,0.1)' },
          },
          y: commonOptions.scales.y,
        },
      },
    };
  }

  function makeHistogramConfig(rows: CellValue[][]): ChartConfiguration {
    const vals = getYValues(rows).filter((v): v is number => v !== null);
    if (vals.length === 0) return makeBarConfig(rows, false, false);

    const min = Math.min(...vals);
    const max = Math.max(...vals);
    const binCount = Math.min(bins, vals.length);
    const binWidth = (max - min) / binCount || 1;

    const counts = new Array(binCount).fill(0);
    for (const v of vals) {
      const b = Math.min(Math.floor((v - min) / binWidth), binCount - 1);
      counts[b]++;
    }
    const labels = counts.map((_, i) => {
      const lo = min + i * binWidth;
      const hi = lo + binWidth;
      return `${lo.toFixed(1)}–${hi.toFixed(1)}`;
    });

    return {
      type: 'bar',
      data: {
        labels,
        datasets: [
          {
            label: `${yColumn} (count)`,
            data: counts,
            backgroundColor: paletteColor(0, 0.75),
            borderColor: paletteColor(0),
            borderWidth: 1,
            barPercentage: 1,
            categoryPercentage: 1,
          },
        ],
      },
      options: { ...commonOptions, scales: { ...commonOptions.scales, x: { ...commonOptions.scales.x, offset: false } } },
    };
  }

  function makeDualAxisConfig(rows: CellValue[][]): ChartConfiguration {
    const labels = getLabels(rows);
    const y1Vals = getYValues(rows);
    const y2Idx = y2Column ? colIndex(y2Column) : -1;
    const y2Vals = y2Idx >= 0 ? rows.map((r) => toNumber(r[y2Idx])) : [];

    const datasets: ChartConfiguration['data']['datasets'] = [
      {
        type: 'bar',
        label: yColumn,
        data: y1Vals,
        backgroundColor: paletteColor(0, 0.65),
        borderColor: paletteColor(0),
        borderWidth: 1,
        yAxisID: 'y',
      },
    ];

    if (y2Vals.length > 0) {
      datasets.push({
        type: 'line',
        label: y2Column,
        data: y2Vals,
        borderColor: paletteColor(1),
        backgroundColor: 'transparent',
        tension: 0.3,
        pointRadius: 3,
        yAxisID: 'y2',
      } as any);
    }

    return {
      type: 'bar',
      data: { labels, datasets },
      options: {
        ...commonOptions,
        scales: {
          x: commonOptions.scales.x,
          y: { ...commonOptions.scales.y, position: 'left' },
          y2: {
            position: 'right',
            ticks: { color: 'var(--color-text-muted)', font: { size: 11 } },
            grid: { drawOnChartArea: false },
          },
        },
      },
    };
  }

  function makeBoxPlotConfig(rows: CellValue[][]): ChartConfiguration {
    // Compute 5-number summary per group (or a single group if no colorColumn)
    const groups: Map<string, number[]> = new Map();

    if (colorColumn) {
      const groupVals = colValues(rows, colorColumn);
      const yVals = getYValues(rows);
      for (let i = 0; i < rows.length; i++) {
        const g = groupVals[i] === null ? 'NULL' : String(groupVals[i]);
        const y = yVals[i];
        if (y === null) continue;
        const arr = groups.get(g) ?? [];
        arr.push(y);
        groups.set(g, arr);
      }
    } else {
      const yVals = getYValues(rows).filter((v): v is number => v !== null);
      groups.set(yColumn, yVals);
    }

    const labels = [...groups.keys()];

    // Compute quartiles for whisker-plot-style rendering via scatter + error bars hack
    // We'll use scatter points at Q1, median, Q3 and line for min/max
    const summaries = labels.map((lbl) => {
      const arr = (groups.get(lbl) ?? []).sort((a, b) => a - b);
      const q = (p: number) => {
        const idx = p * (arr.length - 1);
        const lo = Math.floor(idx);
        const hi = Math.ceil(idx);
        return arr[lo] + (arr[hi] - arr[lo]) * (idx - lo);
      };
      return { min: arr[0] ?? 0, q1: q(0.25), median: q(0.5), q3: q(0.75), max: arr[arr.length - 1] ?? 0 };
    });

    // Render as a simple bar chart showing Q1-Q3 range with a dot at median
    const data = summaries.map((s) => s.median);
    const minData = summaries.map((s) => s.min);
    const maxData = summaries.map((s) => s.max);

    return {
      type: 'bar',
      data: {
        labels,
        datasets: [
          {
            label: 'Median',
            data,
            backgroundColor: paletteColor(0, 0.7),
            borderColor: paletteColor(0),
            borderWidth: 2,
          },
          {
            label: 'Min',
            data: minData,
            type: 'line' as any,
            borderColor: paletteColor(1),
            backgroundColor: 'transparent',
            pointRadius: 4,
            tension: 0,
            borderDash: [4, 2],
          },
          {
            label: 'Max',
            data: maxData,
            type: 'line' as any,
            borderColor: paletteColor(2),
            backgroundColor: 'transparent',
            pointRadius: 4,
            tension: 0,
            borderDash: [4, 2],
          },
        ] as any,
      },
      options: commonOptions,
    };
  }

  // ── Heatmap (custom canvas) ──────────────────────────────────────────────────

  function renderHeatmap() {
    const canvas = heatmapEl;
    if (!canvas) return;
    const rows = activeRows;
    if (rows.length === 0) return;

    const xVals = colValues(rows, xColumn).map((v) => (v === null ? 'NULL' : String(v)));
    const yVals = colValues(rows, yColumn).map((v) => (v === null ? 'NULL' : String(v)));
    const vIdx = colorColumn ? colIndex(colorColumn) : -1;
    const vVals = vIdx >= 0 ? rows.map((r) => toNumber(r[vIdx])) : rows.map((r) => toNumber(r[colIndex(yColumn)]));

    const xLabels = [...new Set(xVals)];
    const yLabels = [...new Set(yVals)];

    // Aggregate by (x,y) → sum
    const cellMap = new Map<string, number>();
    for (let i = 0; i < rows.length; i++) {
      const key = `${xVals[i]}|${yVals[i]}`;
      const v = vVals[i] ?? 0;
      cellMap.set(key, (cellMap.get(key) ?? 0) + v);
    }

    const allCellVals = [...cellMap.values()];
    const minVal = Math.min(...allCellVals);
    const maxVal = Math.max(...allCellVals);

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const MARGIN_LEFT = 80;
    const MARGIN_TOP = 20;
    const MARGIN_BOTTOM = 60;
    const MARGIN_RIGHT = 20;

    const W = canvas.offsetWidth || 400;
    const H = canvas.offsetHeight || 300;
    canvas.width = W;
    canvas.height = H;

    const cellW = (W - MARGIN_LEFT - MARGIN_RIGHT) / xLabels.length;
    const cellH = (H - MARGIN_TOP - MARGIN_BOTTOM) / yLabels.length;

    ctx.clearRect(0, 0, W, H);

    // Draw cells
    for (let xi = 0; xi < xLabels.length; xi++) {
      for (let yi = 0; yi < yLabels.length; yi++) {
        const v = cellMap.get(`${xLabels[xi]}|${yLabels[yi]}`) ?? 0;
        const t = maxVal === minVal ? 0.5 : (v - minVal) / (maxVal - minVal);
        ctx.fillStyle = heatColor(t);
        ctx.fillRect(
          MARGIN_LEFT + xi * cellW,
          MARGIN_TOP + yi * cellH,
          cellW - 1,
          cellH - 1,
        );
      }
    }

    // X labels
    ctx.fillStyle = 'var(--color-text-muted)';
    ctx.font = '10px sans-serif';
    ctx.textAlign = 'center';
    for (let xi = 0; xi < xLabels.length; xi++) {
      ctx.save();
      ctx.translate(MARGIN_LEFT + xi * cellW + cellW / 2, H - MARGIN_BOTTOM + 14);
      ctx.rotate(-Math.PI / 4);
      ctx.fillText(truncate(xLabels[xi], 12), 0, 0);
      ctx.restore();
    }

    // Y labels
    ctx.textAlign = 'right';
    for (let yi = 0; yi < yLabels.length; yi++) {
      ctx.fillText(truncate(yLabels[yi], 12), MARGIN_LEFT - 6, MARGIN_TOP + yi * cellH + cellH / 2 + 4);
    }
  }

  function heatColor(t: number): string {
    // Blue → Purple → Red
    const r = Math.round(t * 220 + (1 - t) * 30);
    const g = Math.round((1 - t) * 80);
    const b = Math.round((1 - t) * 200);
    return `rgb(${r},${g},${b})`;
  }

  function truncate(s: string, max: number): string {
    return s.length > max ? s.slice(0, max - 1) + '…' : s;
  }

  // ── Export ───────────────────────────────────────────────────────────────────

  async function exportChart(format: 'png' | 'jpg' | 'pdf') {
    const canvas = chartType === 'heatmap' ? heatmapEl : canvasEl;
    if (!canvas) return;

    try {
      if (format === 'pdf') {
        const { jsPDF } = await import('jspdf');
        const imgData = canvas.toDataURL('image/png');
        const pdf = new jsPDF({ orientation: 'landscape', unit: 'pt', format: 'a4' });
        const W = pdf.internal.pageSize.getWidth();
        const H = pdf.internal.pageSize.getHeight();
        const ar = canvas.width / canvas.height;
        const w = Math.min(W - 40, H * ar - 40);
        const h = w / ar;
        pdf.addImage(imgData, 'PNG', 20, 20, w, h);
        const bytes = pdf.output('arraybuffer');
        const filePath = await save({ filters: [{ name: 'PDF', extensions: ['pdf'] }], defaultPath: 'chart.pdf' });
        if (filePath) {
          await writeFile(filePath, new Uint8Array(bytes));
          toast.addToast('Chart exported as PDF', 'success', 2000);
        }
        return;
      }

      const mimeType = format === 'jpg' ? 'image/jpeg' : 'image/png';
      const dataUrl = canvas.toDataURL(mimeType, 0.95);
      const base64 = dataUrl.split(',')[1];
      const bytes = Uint8Array.from(atob(base64), (c) => c.charCodeAt(0));
      const filePath = await save({
        filters: [{ name: format.toUpperCase(), extensions: [format] }],
        defaultPath: `chart.${format}`,
      });
      if (filePath) {
        await writeFile(filePath, bytes);
        toast.addToast(`Chart exported as ${format.toUpperCase()}`, 'success', 2000);
      }
    } catch (err) {
      toast.addToast(`Export failed: ${String(err)}`, 'error', 3000);
    }
  }

  function getConfig(): SerializedChartConfig {
    const rawSource = sourceData?.source?.split(' — ')[0] ?? '';
    const title = rawSource.split('.').pop() ?? rawSource;
    return { title, chartType, xColumn, yColumn, y2Column, colorColumn, sizeColumn, useSelectedOnly, bins };
  }

  // ── Column selector helpers ───────────────────────────────────────────────

  const needsX = $derived(!['histogram'].includes(chartType));
  const needsY2 = $derived(chartType === 'dual-axis');
  const needsColor = $derived(['stacked-bar', 'grouped-bar', 'heatmap', 'box-plot'].includes(chartType));
  const needsSize = $derived(chartType === 'bubble');
  const yLabel = $derived(chartType === 'pie' ? 'Value' : chartType === 'histogram' ? 'Column' : 'Y Axis');
  const xLabel = $derived(chartType === 'scatter' || chartType === 'bubble' ? 'X Axis (numeric)' : 'X Axis');
  const colorLabel = $derived(chartType === 'heatmap' ? 'Value' : 'Group by');
  const yColumns = $derived(['scatter', 'bubble', 'dot-plot', 'histogram', 'dual-axis', 'box-plot'].includes(chartType) ? numericColumns : allColumns);
  const yColumnOptions = $derived(yColumns.map((c) => ({ value: c.name, label: c.name })));
</script>

<div class="chart-panel" class:compact>
  {#if !sourceData}
    <div class="empty-state">
      <div class="empty-icon" aria-hidden="true">
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round">
          <line x1="18" y1="20" x2="18" y2="10"/><line x1="12" y1="20" x2="12" y2="4"/><line x1="6" y1="20" x2="6" y2="14"/><line x1="2" y1="20" x2="22" y2="20"/>
        </svg>
      </div>
      <p>Click <strong>Chart</strong> in a table or query toolbar to visualize data here.</p>
    </div>
  {:else}
    <!-- Source info + open as tab -->
    <div class="chart-header">
      <span class="source-label" title={sourceData.source}>{sourceData.source}</span>
      <div class="header-actions">
        {#if selectedRows.length > 0}
          <label class="sel-toggle">
            <input type="checkbox" bind:checked={useSelectedOnly} />
            <span>Selected only ({selectedRows.length})</span>
          </label>
        {/if}
        {#if onOpenAsTab}
          <button class="icon-btn" title="Open as tab" onclick={() => onOpenAsTab?.(getConfig())}>
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/>
            </svg>
          </button>
        {/if}
      </div>
    </div>

    <!-- Chart type selector -->
    <div class="type-section">
      <Select bind:value={chartType} options={chartTypeOptions} size="sm" aria-label="Chart type" class="type-select-field" />
    </div>

    <!-- Column selectors -->
    <div class="cols-section">
      {#if needsX}
        <div class="col-row">
          <span class="col-label">{xLabel}</span>
          <Select bind:value={xColumn} options={allColumnOptions} size="sm" aria-label={xLabel} class="col-select-field" />
        </div>
      {/if}
      <div class="col-row">
        <span class="col-label">{yLabel}</span>
        <Select bind:value={yColumn} options={yColumnOptions} size="sm" aria-label={yLabel} class="col-select-field" />
      </div>
      {#if needsY2}
        <div class="col-row">
          <span class="col-label">Y2 Axis</span>
          <Select bind:value={y2Column} options={[{ value: '', label: '— none —' }, ...numericColumnOptions]} size="sm" aria-label="Y2 axis column" class="col-select-field" />
        </div>
      {/if}
      {#if needsColor}
        <div class="col-row">
          <span class="col-label">{colorLabel}</span>
          <Select bind:value={colorColumn} options={[{ value: '', label: '— none —' }, ...allColumnOptions]} size="sm" aria-label={colorLabel} class="col-select-field" />
        </div>
      {/if}
      {#if needsSize}
        <div class="col-row">
          <span class="col-label">Size</span>
          <Select bind:value={sizeColumn} options={[{ value: '', label: '— fixed —' }, ...numericColumnOptions]} size="sm" aria-label="Size column" class="col-select-field" />
        </div>
      {/if}
      {#if chartType === 'histogram'}
        <div class="col-row">
          <label class="col-label" for="chart-bins">Bins</label>
          <input id="chart-bins" class="bins-input" type="number" min="2" max="200" bind:value={bins} />
        </div>
      {/if}
    </div>

    <!-- Chart canvas -->
    <div class="canvas-wrap">
      {#if chartType === 'heatmap'}
        <canvas bind:this={heatmapEl} class="chart-canvas"></canvas>
      {:else}
        <canvas bind:this={canvasEl} class="chart-canvas"></canvas>
      {/if}
    </div>

    <!-- Export row -->
    <div class="export-row">
      <span class="export-label">Export</span>
      <button class="export-btn" onclick={() => exportChart('png')}>PNG</button>
      <button class="export-btn" onclick={() => exportChart('jpg')}>JPG</button>
      <button class="export-btn" onclick={() => exportChart('pdf')}>PDF</button>
    </div>
  {/if}
</div>

<style>
  .chart-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    overflow: hidden;
  }

  .empty-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--spacing-3);
    padding: var(--spacing-4);
    text-align: center;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    line-height: var(--line-height-normal);
  }

  .empty-icon {
    opacity: 0.4;
  }

  .empty-state p {
    margin: 0;
  }

  .chart-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: 4px var(--spacing-2);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
    min-height: 28px;
  }

  .source-label {
    flex: 1;
    font-size: 10.5px;
    font-weight: 600;
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    flex-shrink: 0;
  }

  .sel-toggle {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    cursor: pointer;
    user-select: none;
  }

  .sel-toggle input {
    margin: 0;
    cursor: pointer;
  }

  .icon-btn {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: var(--radius-sm);
    color: var(--color-text-muted);
    cursor: pointer;
    background: transparent;
    transition: background var(--transition-fast), color var(--transition-fast);
  }

  .icon-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .type-section {
    padding: 4px var(--spacing-2);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .type-section :global(.type-select-field) {
    width: 100%;
  }

  .cols-section {
    padding: var(--spacing-1) var(--spacing-2);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .col-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
  }

  .col-row :global(.col-select-field) {
    flex: 1;
    min-width: 0;
  }

  .col-label {
    font-size: 10px;
    color: var(--color-text-muted);
    width: 70px;
    flex-shrink: 0;
    text-align: right;
  }

  .bins-input {
    width: 60px;
    padding: 2px 4px;
    font-size: var(--font-size-xs);
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--color-text-primary);
    font-family: var(--font-family-mono);
  }

  .canvas-wrap {
    flex: 1;
    min-height: 0;
    position: relative;
    padding: var(--spacing-2);
  }

  .chart-canvas {
    width: 100%;
    height: 100%;
    display: block;
  }

  .export-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: 4px var(--spacing-2);
    border-top: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .export-label {
    font-size: 10px;
    color: var(--color-text-muted);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    margin-right: 2px;
  }

  .export-btn {
    font-size: var(--font-size-xs);
    padding: 2px 8px;
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--color-text-secondary);
    cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast);
  }

  .export-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }
</style>
