import type { VirtualRelation, ColumnRef } from '$lib/types';

const STORAGE_KEY = 'rowmance_virtual_relations';

function loadFromStorage(): VirtualRelation[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

function saveToStorage(relations: VirtualRelation[]): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(relations));
}

let relations = $state<VirtualRelation[]>(loadFromStorage());

export function useVirtualRelations() {
  return {
    get relations() { return relations; },

    add(vr: Omit<VirtualRelation, 'id'>): VirtualRelation {
      const newVr: VirtualRelation = { ...vr, id: crypto.randomUUID() };
      relations = [...relations, newVr];
      saveToStorage(relations);
      return newVr;
    },

    remove(id: string): void {
      relations = relations.filter((r) => r.id !== id);
      saveToStorage(relations);
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
