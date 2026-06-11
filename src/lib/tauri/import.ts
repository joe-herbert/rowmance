/** Typed wrappers around the Tauri `import::*` commands. */
import { invoke } from '@tauri-apps/api/core';

export interface InferredColumn {
  name: string;
  inferredType: 'integer' | 'float' | 'text' | 'date';
}

export interface CsvPreview {
  columns: InferredColumn[];
  previewRows: string[][];
}

export interface ColumnOverride {
  name: string;
  dbType: string;
}

export async function importCsvPreview(filePath: string): Promise<CsvPreview> {
  return invoke<CsvPreview>('import_csv_preview', { filePath });
}

export async function importCsvExecute(
  connectionId: string,
  filePath: string,
  tableName: string,
  createTable: boolean,
  columnOverrides: ColumnOverride[],
): Promise<number> {
  return invoke<number>('import_csv_execute', {
    connectionId,
    filePath,
    tableName,
    createTable,
    columnOverrides,
  });
}

export async function importSqlFile(
  connectionId: string,
  filePath: string,
  stopOnError: boolean,
): Promise<number> {
  return invoke<number>('import_sql_file', { connectionId, filePath, stopOnError });
}
