import type { VirtualRelation, ColumnRef, PolymorphicVirtualRelation } from '$lib/types';
import * as vrApi from '$lib/tauri/virtual_relations';

let relations = $state<VirtualRelation[]>([]);
let polymorphicRelations = $state<PolymorphicVirtualRelation[]>([]);

vrApi.listVirtualRelations().then((loaded) => {
  relations = loaded;
});

vrApi.listPolymorphicVirtualRelations().then((loaded) => {
  polymorphicRelations = loaded;
});

export function useVirtualRelations() {
  return {
    get relations() {
      return relations;
    },

    get polymorphicRelations() {
      return polymorphicRelations;
    },

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

    async addPolymorphic(input: {
      label?: string;
      connectionId: string;
      database: string;
      table: string;
      typeColumn: string;
      valueColumn: string;
      mappings: { typeValue: string; to: ColumnRef }[];
    }): Promise<PolymorphicVirtualRelation> {
      const pvr = await vrApi.createPolymorphicVirtualRelation({
        label: input.label ?? null,
        connectionId: input.connectionId,
        database: input.database,
        table: input.table,
        typeColumn: input.typeColumn,
        valueColumn: input.valueColumn,
        mappings: input.mappings,
      });
      polymorphicRelations = [...polymorphicRelations, pvr];
      return pvr;
    },

    async updatePolymorphic(
      id: string,
      input: {
        label?: string;
        connectionId: string;
        database: string;
        table: string;
        typeColumn: string;
        valueColumn: string;
        mappings: { typeValue: string; to: ColumnRef }[];
      },
    ): Promise<PolymorphicVirtualRelation> {
      const pvr = await vrApi.updatePolymorphicVirtualRelation(id, {
        label: input.label ?? null,
        connectionId: input.connectionId,
        database: input.database,
        table: input.table,
        typeColumn: input.typeColumn,
        valueColumn: input.valueColumn,
        mappings: input.mappings,
      });
      polymorphicRelations = polymorphicRelations.map((r) => (r.id === id ? pvr : r));
      return pvr;
    },

    async removePolymorphic(id: string): Promise<void> {
      await vrApi.deletePolymorphicVirtualRelation(id);
      polymorphicRelations = polymorphicRelations.filter((r) => r.id !== id);
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
          (r.from.connectionId === connectionId &&
            r.from.database === database &&
            r.from.table === table &&
            r.from.column === column) ||
          (r.to.connectionId === connectionId &&
            r.to.database === database &&
            r.to.table === table &&
            r.to.column === column),
      );
    },

    /** Whether any virtual relation has the given column on the "from" (child/referencing) side. */
    hasForwardFrom(connectionId: string, database: string, table: string, column: string): boolean {
      return relations.some(
        (r) =>
          r.from.connectionId === connectionId &&
          r.from.database === database &&
          r.from.table === table &&
          r.from.column === column,
      );
    },

    /** Whether any virtual relation involves any column of the given table (as "from" side). */
    hasAnyForTable(connectionId: string, database: string, table: string): boolean {
      return relations.some(
        (r) =>
          r.from.connectionId === connectionId &&
          r.from.database === database &&
          r.from.table === table,
      );
    },

    /** Find polymorphic VRs where the given column is the value column. */
    polymorphicForValueColumn(
      connectionId: string,
      database: string,
      table: string,
      column: string,
    ): PolymorphicVirtualRelation[] {
      return polymorphicRelations.filter(
        (r) =>
          r.connectionId === connectionId &&
          r.database === database &&
          r.table === table &&
          r.valueColumn === column,
      );
    },

    /** Whether any polymorphic VR uses this column as its value column. */
    hasPolymorphicValueColumn(
      connectionId: string,
      database: string,
      table: string,
      column: string,
    ): boolean {
      return polymorphicRelations.some(
        (r) =>
          r.connectionId === connectionId &&
          r.database === database &&
          r.table === table &&
          r.valueColumn === column,
      );
    },

    /** Resolve a polymorphic VR: given the value column and the type column's value, return the target ColumnRef. */
    resolvePolymorphic(
      connectionId: string,
      database: string,
      table: string,
      valueColumn: string,
      typeValue: string,
    ): { target: ColumnRef; typeColumn: string } | null {
      const pvr = polymorphicRelations.find(
        (r) =>
          r.connectionId === connectionId &&
          r.database === database &&
          r.table === table &&
          r.valueColumn === valueColumn,
      );
      if (!pvr) return null;
      const mapping = pvr.mappings.find((m) => m.typeValue === typeValue);
      if (!mapping) return null;
      return { target: mapping.to, typeColumn: pvr.typeColumn };
    },

    /** Find the polymorphic VR for a given value column (first match). */
    findPolymorphicForValueColumn(
      connectionId: string,
      database: string,
      table: string,
      valueColumn: string,
    ): PolymorphicVirtualRelation | null {
      return (
        polymorphicRelations.find(
          (r) =>
            r.connectionId === connectionId &&
            r.database === database &&
            r.table === table &&
            r.valueColumn === valueColumn,
        ) ?? null
      );
    },
  };
}
