/** Typed wrappers around the Tauri `settings::*` commands. */
import { invoke } from '@tauri-apps/api/core';
import type { AppSettings } from '$lib/types';

export async function getAllSettings(): Promise<Partial<AppSettings>> {
  return invoke<Partial<AppSettings>>('settings_get_all');
}

export async function setSetting<K extends keyof AppSettings>(
  key: K,
  value: AppSettings[K],
): Promise<void> {
  return invoke<void>('settings_set', { key, value });
}

export async function resetSetting(key: keyof AppSettings): Promise<void> {
  return invoke<void>('settings_reset', { key });
}
