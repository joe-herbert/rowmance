/** Typed wrappers around the Tauri `history::*` commands. */
import { invoke } from '@tauri-apps/api/core';
import type { QueryHistoryEntry } from '$lib/types';

export async function listHistory(
  connectionId: string,
  limit = 100,
  offset = 0,
): Promise<QueryHistoryEntry[]> {
  return invoke<QueryHistoryEntry[]>('history_list', { connectionId, limit, offset });
}

export async function clearHistory(connectionId: string): Promise<void> {
  return invoke<void>('history_clear', { connectionId });
}

export async function deleteHistoryEntry(id: string): Promise<void> {
  return invoke<void>('history_delete', { id });
}
