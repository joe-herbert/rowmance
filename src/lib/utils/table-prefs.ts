import { invoke } from '@tauri-apps/api/core';

interface ColPrefs {
  hiddenColumns: string[];
  colWidths: Record<string, number>;
  columnOrder: string[];
}

const PREFIX = 'col_prefs:';

function prefsKey(connectionId: string, database: string, table: string): string {
  return `${PREFIX}${connectionId}:${database}:${table}`;
}

export async function loadColPrefs(
  connectionId: string,
  database: string,
  table: string,
): Promise<ColPrefs | null> {
  try {
    return await invoke<ColPrefs | null>('settings_get', {
      key: prefsKey(connectionId, database, table),
    });
  } catch {
    return null;
  }
}

export async function saveColPrefs(
  connectionId: string,
  database: string,
  table: string,
  prefs: Partial<ColPrefs>,
): Promise<void> {
  try {
    const key = prefsKey(connectionId, database, table);
    const existing = (await loadColPrefs(connectionId, database, table)) ?? {
      hiddenColumns: [],
      colWidths: {},
      columnOrder: [],
    };
    await invoke<void>('settings_set', { key, value: { ...existing, ...prefs } });
  } catch {
    // intentionally empty
  }
}
