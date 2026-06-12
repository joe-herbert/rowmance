/** Typed wrappers around the Tauri `updater::*` commands. */
import { invoke } from '@tauri-apps/api/core';
import type { UpdateCheckResult } from '$lib/types';

export async function updaterCheck(): Promise<UpdateCheckResult> {
  return invoke<UpdateCheckResult>('updater_check');
}

export async function updaterInstall(): Promise<void> {
  return invoke<void>('updater_install');
}
