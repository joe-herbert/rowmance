/** Typed wrappers around the Tauri `themes::*` commands. */
import { invoke } from '@tauri-apps/api/core';
import type { ThemeMeta, ThemeData } from '$lib/types';

export async function themesList(): Promise<ThemeMeta[]> {
  return invoke<ThemeMeta[]>('themes_list');
}

export async function themesRead(name: string): Promise<ThemeData> {
  return invoke<ThemeData>('themes_read', { name });
}

export async function themesWrite(name: string, data: ThemeData): Promise<void> {
  return invoke<void>('themes_write', { name, data });
}

export async function themesDelete(name: string): Promise<void> {
  return invoke<void>('themes_delete', { name });
}

export async function themesDuplicate(source: string, newName: string): Promise<ThemeMeta> {
  return invoke<ThemeMeta>('themes_duplicate', { source, newName });
}

export async function themesRename(oldName: string, newName: string): Promise<ThemeMeta> {
  return invoke<ThemeMeta>('themes_rename', { oldName, newName });
}
