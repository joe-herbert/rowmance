/** Typed wrappers around the Tauri `virtual_relations::*` commands. */
import { invoke } from '@tauri-apps/api/core';
import type { VirtualRelation, ColumnRef, PolymorphicVirtualRelation } from '$lib/types';

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

// ── Polymorphic virtual relations ─────────────────────────────────────────────

export interface PolymorphicMappingInput {
  typeValue: string;
  to: ColumnRef;
}

export interface PolymorphicVirtualRelationInput {
  label?: string | null;
  connectionId: string;
  database: string;
  table: string;
  typeColumn: string;
  valueColumn: string;
  mappings: PolymorphicMappingInput[];
}

export async function listPolymorphicVirtualRelations(): Promise<PolymorphicVirtualRelation[]> {
  return invoke<PolymorphicVirtualRelation[]>('polymorphic_vr_list');
}

export async function createPolymorphicVirtualRelation(
  input: PolymorphicVirtualRelationInput,
): Promise<PolymorphicVirtualRelation> {
  return invoke<PolymorphicVirtualRelation>('polymorphic_vr_create', { input });
}

export async function updatePolymorphicVirtualRelation(
  id: string,
  input: PolymorphicVirtualRelationInput,
): Promise<PolymorphicVirtualRelation> {
  return invoke<PolymorphicVirtualRelation>('polymorphic_vr_update', { id, input });
}

export async function deletePolymorphicVirtualRelation(id: string): Promise<void> {
  return invoke<void>('polymorphic_vr_delete', { id });
}
