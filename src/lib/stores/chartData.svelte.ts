import type { ColumnMeta } from '$lib/types';

type CellValue = string | number | boolean | null;

export interface ChartSourceData {
  columns: ColumnMeta[];
  allRows: CellValue[][];
  selectedRows: CellValue[][];
  source: string;
  sourceId: string;
}

let chartSource = $state<ChartSourceData | null>(null);

export function useChartData() {
  return {
    get current(): ChartSourceData | null {
      return chartSource;
    },
    update(data: ChartSourceData) {
      chartSource = data;
    },
    updateRows(sourceId: string, allRows: CellValue[][], selectedRows: CellValue[][]) {
      if (chartSource?.sourceId === sourceId) {
        chartSource = { ...chartSource, allRows, selectedRows };
      }
    },
    clear(sourceId: string) {
      if (chartSource?.sourceId === sourceId) {
        chartSource = null;
      }
    },
    /** Push data and open the chart panel in the right sidebar. */
    openChart(data: ChartSourceData) {
      chartSource = data;
      document.dispatchEvent(new CustomEvent('open-right-panel', { detail: 'chart' }));
    },
  };
}
