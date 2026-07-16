/** Typed wrappers around the Tauri `export::*` commands. */
import { invoke } from '@tauri-apps/api/core';

export type ExportFormat = 'csv' | 'json' | 'sql_insert' | 'sql_in_clause' | 'tab_separated';

export async function exportResultToClipboard(
  rows: unknown[][],
  columns: string[],
  format: ExportFormat,
  tableName?: string,
  connectionId?: string,
): Promise<void> {
  return invoke<void>('export_result_to_clipboard', { rows, columns, format, tableName, connectionId });
}

export async function exportResultToFile(
  rows: unknown[][],
  columns: string[],
  format: ExportFormat,
  filePath: string,
  tableName?: string,
  connectionId?: string,
): Promise<void> {
  return invoke<void>('export_result_to_file', { rows, columns, format, filePath, tableName, connectionId });
}
