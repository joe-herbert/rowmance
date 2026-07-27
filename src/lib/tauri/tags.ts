/** Typed wrappers around the Tauri `tags::*` commands. */
import { invoke } from '@tauri-apps/api/core';
import type { Tag, TagInput } from '$lib/types';

export async function listTags(): Promise<Tag[]> {
  return invoke<Tag[]>('tags_list');
}

export async function createTag(input: TagInput): Promise<Tag> {
  return invoke<Tag>('tags_create', { input });
}

export async function updateTag(id: string, input: TagInput): Promise<Tag> {
  return invoke<Tag>('tags_update', { id, input });
}

export async function deleteTag(id: string): Promise<void> {
  return invoke<void>('tags_delete', { id });
}

export async function reorderTags(ids: string[]): Promise<void> {
  return invoke<void>('tags_reorder', { ids });
}
