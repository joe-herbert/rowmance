import type { VirtualRelation, ColumnRef } from '$lib/types';
import * as vrApi from '$lib/tauri/virtual_relations';

let relations = $state<VirtualRelation[]>([]);

vrApi.listVirtualRelations().then((loaded) => {
  relations = loaded;
});

export function useVirtualRelations() {
  return {
    get relations() { return relations; },

    async add(input: { from: ColumnRef; to: ColumnRef; label?: string }): Promise<VirtualRelation> {
      const vr = await vrApi.createVirtualRelation({
        from: input.from,
        to: input.to,
        label: input.label ?? null,
      });
      relations = [...relations, vr];
      return vr;
    },

    async remove(id: string): Promise<void> {
      await vrApi.deleteVirtualRelation(id);
      relations = relations.filter((r) => r.id !== id);
    },

    /** Find all virtual relations where the given column is the "from" side. */
    forwardFrom(ref: ColumnRef): VirtualRelation[] {
      return relations.filter(
        (r) =>
          r.from.connectionId === ref.connectionId &&
          r.from.database === ref.database &&
          r.from.table === ref.table &&
          r.from.column === ref.column,
      );
    },

    /** Find all virtual relations where the given column is the "to" side. */
    reverseFrom(ref: ColumnRef): VirtualRelation[] {
      return relations.filter(
        (r) =>
          r.to.connectionId === ref.connectionId &&
          r.to.database === ref.database &&
          r.to.table === ref.table &&
          r.to.column === ref.column,
      );
    },

    /** Whether any virtual relation involves the given table column (either side). */
    hasAny(connectionId: string, database: string, table: string, column: string): boolean {
      return relations.some(
        (r) =>
          (r.from.connectionId === connectionId && r.from.database === database && r.from.table === table && r.from.column === column) ||
          (r.to.connectionId === connectionId && r.to.database === database && r.to.table === table && r.to.column === column),
      );
    },

    /** Whether any virtual relation involves any column of the given table (as "from" side). */
    hasAnyForTable(connectionId: string, database: string, table: string): boolean {
      return relations.some(
        (r) =>
          r.from.connectionId === connectionId && r.from.database === database && r.from.table === table,
      );
    },
  };
}
