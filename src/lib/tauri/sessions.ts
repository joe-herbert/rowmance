import { invoke } from '@tauri-apps/api/core';

export async function sessionAcquire(connectionId: string, sessionId: string): Promise<void> {
  return invoke<void>('session_acquire', { connectionId, sessionId });
}

export async function sessionRelease(sessionId: string): Promise<void> {
  return invoke<void>('session_release', { sessionId });
}
