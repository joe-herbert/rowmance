import { save as saveDialog, open as openDialog } from '@tauri-apps/plugin-dialog';
import { writeTextFile, readTextFile } from '@tauri-apps/plugin-fs';
import type { Dashboard, DashboardWidget, ConnectionProfile } from '$lib/types';

// ── Export types ──────────────────────────────────────────────────────────────

interface ExportConnection {
  originalId: string;
  fingerprint: string;
  name: string;
  dbType: string;
  host: string;
  port: number;
  database: string;
  username: string;
}

interface ExportWidget extends Omit<DashboardWidget, 'connectionId'> {
  connectionKey: string;
}

interface DashboardExportFile {
  version: 1;
  exportedAt: string;
  dashboard: {
    name: string;
    icon: string;
    widgets: ExportWidget[];
  };
  connections: Record<string, ExportConnection>;
}

function makeFingerprint(c: ConnectionProfile): string {
  return `${c.dbType}:${c.host}:${c.port}:${c.database}`;
}

// ── Export ────────────────────────────────────────────────────────────────────

export async function exportDashboard(
  dashboard: Dashboard,
  getConnectionById: (id: string) => ConnectionProfile | undefined,
): Promise<void> {
  const connections: Record<string, ExportConnection> = {};

  const widgets: ExportWidget[] = dashboard.widgets.map((w) => {
    const conn = getConnectionById(w.connectionId);
    const fingerprint = conn ? makeFingerprint(conn) : w.connectionId;
    const key = conn ? fingerprint : `unknown:${w.connectionId}`;

    if (!connections[key]) {
      connections[key] = {
        originalId: w.connectionId,
        fingerprint,
        name: conn?.name ?? 'Unknown connection',
        dbType: conn?.dbType ?? 'postgres',
        host: conn?.host ?? '',
        port: conn?.port ?? 5432,
        database: conn?.database ?? '',
        username: conn?.username ?? '',
      };
    }

    const { connectionId: _, ...rest } = w;
    return { ...rest, connectionKey: key };
  });

  const file: DashboardExportFile = {
    version: 1,
    exportedAt: new Date().toISOString(),
    dashboard: { name: dashboard.name, icon: dashboard.icon, widgets },
    connections,
  };

  const safeName = dashboard.name.replace(/[^a-z0-9_-]/gi, '_').toLowerCase();
  const filePath = await saveDialog({
    defaultPath: `${safeName}.rowmance-dashboard`,
    filters: [{ name: 'Rowmance Dashboard', extensions: ['rowmance-dashboard'] }],
  });

  if (!filePath) return;
  await writeTextFile(filePath, JSON.stringify(file, null, 2));
}

// ── Import ────────────────────────────────────────────────────────────────────

export interface ImportedDashboard {
  name: string;
  icon: string;
  widgets: Omit<DashboardWidget, 'id'>[];
}

export async function importDashboard(
  allProfiles: ConnectionProfile[],
): Promise<ImportedDashboard | null> {
  const filePath = await openDialog({
    filters: [{ name: 'Rowmance Dashboard', extensions: ['rowmance-dashboard'] }],
    multiple: false,
  });

  if (!filePath || typeof filePath !== 'string') return null;

  const raw = await readTextFile(filePath);
  const file = JSON.parse(raw) as DashboardExportFile;

  if (file.version !== 1 || !file.dashboard) {
    throw new Error('Unrecognised dashboard file format.');
  }

  const fingerprintMap = new Map(allProfiles.map((p) => [makeFingerprint(p), p.id]));
  const idMap = new Map(allProfiles.map((p) => [p.id, p.id]));

  function resolveConnectionId(key: string): string {
    const exportConn = file.connections[key];
    if (!exportConn) return '';
    const byFingerprint = fingerprintMap.get(exportConn.fingerprint);
    if (byFingerprint) return byFingerprint;
    const byId = idMap.get(exportConn.originalId);
    if (byId) return byId;
    return '';
  }

  const widgets: Omit<DashboardWidget, 'id'>[] = file.dashboard.widgets.map((w) => {
    const { connectionKey, ...rest } = w;
    return { ...rest, connectionId: resolveConnectionId(connectionKey) };
  });

  return { name: file.dashboard.name, icon: file.dashboard.icon, widgets };
}
