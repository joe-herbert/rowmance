import { invoke } from '@tauri-apps/api/core';

export interface SpeedAnalysisEntry {
  id: string;
  connectionId: string;
  sql: string;
  executedAt: string;
  totalUs: number;
  poolAcquireUs: number;
  dbSwitchUs: number;
  executionUs: number;
  resultProcessingUs: number;
  rowCount: number | null;
}

export function speedAnalysisList(): Promise<SpeedAnalysisEntry[]> {
  return invoke('speed_analysis_list');
}

export function speedAnalysisClear(): Promise<void> {
  return invoke('speed_analysis_clear');
}
