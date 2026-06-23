/** Typed wrappers around the Tauri `saved_queries::*` commands. */
import { invoke } from '@tauri-apps/api/core';
import type { SavedQueryFolder, SavedQuery } from '$lib/types';

export interface SavedQueryFolderInput {
  name: string;
  parentId?: string | null;
  position?: number;
}

export interface SavedQueryInput {
  connectionId?: string | null;
  folderId?: string | null;
  name: string;
  sql: string;
  position?: number;
}

export async function listFolders(): Promise<SavedQueryFolder[]> {
  return invoke<SavedQueryFolder[]>('saved_queries_list_folders');
}

export async function createFolder(input: SavedQueryFolderInput): Promise<SavedQueryFolder> {
  return invoke<SavedQueryFolder>('saved_queries_create_folder', { input });
}

export async function updateFolder(
  id: string,
  input: SavedQueryFolderInput,
): Promise<SavedQueryFolder> {
  return invoke<SavedQueryFolder>('saved_queries_update_folder', { id, input });
}

export async function deleteFolder(id: string): Promise<void> {
  return invoke<void>('saved_queries_delete_folder', { id });
}

export async function listSavedQueries(folderId?: string | null): Promise<SavedQuery[]> {
  return invoke<SavedQuery[]>('saved_queries_list', { folderId: folderId ?? null });
}

export async function createSavedQuery(input: SavedQueryInput): Promise<SavedQuery> {
  return invoke<SavedQuery>('saved_queries_create', { input });
}

export async function updateSavedQuery(id: string, input: SavedQueryInput): Promise<SavedQuery> {
  return invoke<SavedQuery>('saved_queries_update', { id, input });
}

export async function deleteSavedQuery(id: string): Promise<void> {
  return invoke<void>('saved_queries_delete', { id });
}
