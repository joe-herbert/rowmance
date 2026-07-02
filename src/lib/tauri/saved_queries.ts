/** Typed wrappers around the Tauri `saved_queries::*` commands. */
import { invoke } from '@tauri-apps/api/core';
import type {
  SavedQueryFolder,
  SavedQuery,
  FileQuery,
  FileQueryFolder,
  FileQueryListResult,
} from '$lib/types';

export interface SavedQueryFolderInput {
  name: string;
  parentId?: string | null;
  position?: number;
}

export interface SavedQueryInput {
  connectionId?: string | null;
  folderId?: string | null;
  database?: string | null;
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

// ── File-based saved queries ──────────────────────────────────────────────────

export interface FileQueryInput {
  name: string;
  sql: string;
  connectionId?: string | null;
  folderId?: string | null;
  database?: string | null;
}

export async function fileGetDir(): Promise<string> {
  return invoke<string>('file_saved_queries_get_dir');
}

export async function fileListSavedQueries(): Promise<FileQueryListResult> {
  return invoke<FileQueryListResult>('file_saved_queries_list');
}

export async function fileCreateSavedQuery(input: FileQueryInput): Promise<FileQuery> {
  return invoke<FileQuery>('file_saved_queries_create', {
    folderId: input.folderId ?? null,
    name: input.name,
    sql: input.sql,
    connectionId: input.connectionId ?? null,
    database: input.database ?? null,
  });
}

export async function fileUpdateSavedQuery(
  id: string,
  input: FileQueryInput & { folderId?: string | null },
): Promise<FileQuery> {
  return invoke<FileQuery>('file_saved_queries_update', {
    id,
    name: input.name,
    sql: input.sql,
    connectionId: input.connectionId ?? null,
    folderId: input.folderId ?? null,
    database: input.database ?? null,
  });
}

export async function fileDeleteSavedQuery(id: string): Promise<void> {
  return invoke<void>('file_saved_queries_delete', { id });
}

export async function fileUpdatePositions(
  updates: { id: string; position: number }[],
): Promise<void> {
  return invoke<void>('file_saved_queries_update_positions', { updates });
}

export async function fileCreateFolder(
  name: string,
  parentId?: string | null,
): Promise<FileQueryFolder> {
  return invoke<FileQueryFolder>('file_saved_queries_create_folder', {
    parentId: parentId ?? null,
    name,
  });
}

export async function fileDeleteFolder(id: string): Promise<void> {
  return invoke<void>('file_saved_queries_delete_folder', { id });
}

export async function fileRenameFolder(id: string, name: string): Promise<FileQueryFolder> {
  return invoke<FileQueryFolder>('file_saved_queries_rename_folder', { id, name });
}

export async function fileAssignConnection(
  fileConnectionId: string,
  localConnectionId: string,
): Promise<void> {
  return invoke<void>('file_saved_queries_assign_connection', {
    fileConnectionId,
    localConnectionId,
  });
}
