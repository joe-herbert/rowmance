/** Typed wrappers around the Tauri `virtual_relations::*` commands. */
import { invoke } from '@tauri-apps/api/core';
import type { VirtualRelation, ColumnRef } from '$lib/types';

export interface VirtualRelationInput {
  label?: string | null;
  from: ColumnRef;
  to: ColumnRef;
}

export async function listVirtualRelations(): Promise<VirtualRelation[]> {
  return invoke<VirtualRelation[]>('virtual_relations_list');
}

export async function createVirtualRelation(input: VirtualRelationInput): Promise<VirtualRelation> {
  return invoke<VirtualRelation>('virtual_relations_create', { input });
}

export async function updateVirtualRelation(
  id: string,
  input: VirtualRelationInput,
): Promise<VirtualRelation> {
  return invoke<VirtualRelation>('virtual_relations_update', { id, input });
}

export async function deleteVirtualRelation(id: string): Promise<void> {
  return invoke<void>('virtual_relations_delete', { id });
}
