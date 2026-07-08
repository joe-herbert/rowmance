/**
 * splitLeafSlots.svelte.ts
 *
 * Module-level registry of leaf slot placeholder elements.
 * SplitNodeRenderer registers/unregisters slot elements here via a Svelte action.
 * SplitPanel reads the map to set up ResizeObserver on each slot.
 */

let slotMap = $state(new Map<string, HTMLElement>());

export function useLeafSlots() {
  return {
    get slots() {
      return slotMap;
    },
    register(splitId: string, el: HTMLElement) {
      const next = new Map(slotMap);
      next.set(splitId, el);
      slotMap = next;
    },
    unregister(splitId: string) {
      const next = new Map(slotMap);
      next.delete(splitId);
      slotMap = next;
    },
    unregisterIfOwner(splitId: string, el: HTMLElement) {
      if (slotMap.get(splitId) === el) {
        const next = new Map(slotMap);
        next.delete(splitId);
        slotMap = next;
      }
    },
  };
}
