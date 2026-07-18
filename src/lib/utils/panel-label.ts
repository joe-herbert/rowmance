import type { PanelKind } from '$lib/types';

export function panelLabel(content: PanelKind, dashboardsById: Map<string, { name: string }>): string {
  switch (content.kind) {
    case 'query_editor':
      return content.savedQueryName ?? 'Query';
    case 'table_browser':
      return content.table;
    case 'table_structure':
      return content.table;
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
