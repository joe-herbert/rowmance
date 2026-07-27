/** Shared tree helpers for nested connection groups (folders-in-folders). */
import type { ConnectionGroup } from '$lib/types';

/** Group `groups` by `parentId`, each list sorted by position then name. */
export function groupsByParent(groups: ConnectionGroup[]): Map<string | null, ConnectionGroup[]> {
  const map = new Map<string | null, ConnectionGroup[]>();
  for (const g of groups) {
    const arr = map.get(g.parentId) ?? [];
    arr.push(g);
    map.set(g.parentId, arr);
  }
  for (const [key, arr] of map) {
    map.set(
      key,
      [...arr].sort(
        (a, b) => a.position - b.position || a.name.localeCompare(b.name, undefined, { sensitivity: 'base' }),
      ),
    );
  }
  return map;
}

/** Whether `folderId` is nested (at any depth) inside `ancestorId`. */
export function isDescendantGroup(
  groups: ConnectionGroup[],
  folderId: string,
  ancestorId: string,
): boolean {
  const folder = groups.find((g) => g.id === folderId);
  if (!folder || folder.parentId === null) return false;
  if (folder.parentId === ancestorId) return true;
  return isDescendantGroup(groups, folder.parentId, ancestorId);
}

/** All descendant group ids nested (at any depth) inside `groupId`, not including itself. */
export function collectDescendantGroupIds(groups: ConnectionGroup[], groupId: string): Set<string> {
  const result = new Set<string>();
  const stack = [groupId];
  while (stack.length > 0) {
    const current = stack.pop()!;
    for (const g of groups) {
      if (g.parentId === current && !result.has(g.id)) {
        result.add(g.id);
        stack.push(g.id);
      }
    }
  }
  return result;
}
