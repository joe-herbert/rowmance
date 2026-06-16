import type { ColumnMeta } from '$lib/types';

type CellValue = string | number | boolean | null;

export interface CellSelection {
  connectionId: string;
  database: string;
  table: string;
  columnName: string;
  cellValue: CellValue;
  row: CellValue[];
  columns: ColumnMeta[];
}

let current = $state<CellSelection | null>(null);

export function useCellSelection() {
  return {
    get current() { return current; },
    set(s: CellSelection | null) { current = s; },
  };
}
