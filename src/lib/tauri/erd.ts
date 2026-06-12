/** Typed wrappers around the Tauri `erd::*` commands. */
import { invoke } from '@tauri-apps/api/core';
import type { ErdGraph } from '$lib/types';

export async function getErdGraph(connectionId: string, database: string): Promise<ErdGraph> {
  return invoke<ErdGraph>('erd_get_graph', { connectionId, database });
}
