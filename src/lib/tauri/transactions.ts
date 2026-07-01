/** Typed wrappers around the Tauri transaction commands. */
import { invoke } from '@tauri-apps/api/core';

export async function beginTransaction(connectionId: string, database?: string): Promise<void> {
  return invoke<void>('transaction_begin', { connectionId, database: database ?? null });
}

export async function commitTransaction(connectionId: string): Promise<void> {
  return invoke<void>('transaction_commit', { connectionId });
}

export async function rollbackTransaction(connectionId: string): Promise<void> {
  return invoke<void>('transaction_rollback', { connectionId });
}
