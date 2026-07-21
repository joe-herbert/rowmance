import type { PanelKind } from '$lib/types';

export function panelLabel(
  content: PanelKind,
  dashboardsById: Map<string, { name: string }>,
  qualifyTable = false,
): string {
  switch (content.kind) {
    case 'query_editor':
      return content.savedQueryName ?? 'Query';
    case 'table_browser':
      return qualifyTable ? `${content.database}.${content.table}` : content.table;
    case 'table_structure':
      return qualifyTable ? `${content.database}.${content.table}` : content.table;
    case 'ddl_viewer':
      return content.objectName;
    case 'erd':
      return 'ERD';
    case 'explain':
      return 'Explain';
    case 'settings':
      return 'Settings';
    case 'user_manager':
      return 'Users';
    case 'server_admin':
      return 'Server';
    case 'speed_analysis':
      return 'Speed Analysis';
    case 'release_notes':
      return `What's New in ${content.version}`;
    case 'connections':
      return 'Connections';
    case 'dashboard':
      return dashboardsById.get(content.dashboardId)?.name ?? 'Dashboard';
    case 'empty':
      return 'Empty';
  }
}

/**
 * Keys of the form `connectionId::table` for table tabs whose table name is open more than
 * once on the same connection (across different databases/schemas) — these should be qualified
 * with their database/schema name in the UI to disambiguate them.
 */
export function ambiguousTableKeys(items: { content: PanelKind }[]): Set<string> {
  const counts = new Map<string, number>();
  for (const { content } of items) {
    if (content.kind !== 'table_browser' && content.kind !== 'table_structure') continue;
    const key = `${content.connectionId}::${content.table}`;
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  const result = new Set<string>();
  for (const [key, count] of counts) {
    if (count > 1) result.add(key);
  }
  return result;
}

export function isTableAmbiguous(content: PanelKind, keys: Set<string>): boolean {
  if (content.kind !== 'table_browser' && content.kind !== 'table_structure') return false;
  return keys.has(`${content.connectionId}::${content.table}`);
}
