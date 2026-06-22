interface ColPrefs {
  hiddenColumns: string[];
  colWidths: Record<string, number>;
  columnOrder: string[];
}

const PREFIX = 'rowmance_col_prefs:';

function prefsKey(connectionId: string, database: string, table: string): string {
  return `${PREFIX}${connectionId}:${database}:${table}`;
}

export function loadColPrefs(connectionId: string, database: string, table: string): ColPrefs | null {
  try {
    const raw = localStorage.getItem(prefsKey(connectionId, database, table));
    return raw ? (JSON.parse(raw) as ColPrefs) : null;
  } catch {
    return null;
  }
}

export function saveColPrefs(
  connectionId: string,
  database: string,
  table: string,
  prefs: Partial<ColPrefs>,
): void {
  try {
    const key = prefsKey(connectionId, database, table);
    const existing: ColPrefs = loadColPrefs(connectionId, database, table) ?? {
      hiddenColumns: [],
      colWidths: {},
      columnOrder: [],
    };
    localStorage.setItem(key, JSON.stringify({ ...existing, ...prefs }));
  } catch {}
}
