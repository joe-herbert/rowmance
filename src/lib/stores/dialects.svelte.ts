/** Module-level reactive dialect registry. Loads once from the backend on first import. */
import { listDialects, type DialectEntry } from '$lib/tauri/connections';
import type { DialectInfo } from '$lib/types';

let _dialects = $state<Map<string, DialectInfo>>(new Map());

listDialects()
  .then((entries) => {
    _dialects = new Map(entries.map(({ dbType, dialect }) => [dbType, dialect]));
  })
  .catch(console.error);

export function getDialect(dbType: string): DialectInfo | undefined {
  return _dialects.get(dbType);
}

export function urlSchemeToDbType(scheme: string): string | undefined {
  for (const [dbType, dialect] of _dialects) {
    if (dialect.urlSchemes.includes(scheme)) return dbType;
  }
  return undefined;
}

export function getAllDialects(): DialectEntry[] {
  return [..._dialects.entries()].map(([dbType, dialect]) => ({ dbType, dialect }));
}

/** Returns the union of all system database names reported by loaded engine dialects. */
export function getAllSystemDatabases(): string[] {
  return [..._dialects.values()].flatMap((d) => d.systemDatabases ?? []);
}

/** Returns the union of all file extensions recognised by any loaded engine dialect. */
export function getAllFileExtensions(): Array<{ ext: string; dbType: string }> {
  return [..._dialects.entries()].flatMap(([dbType, d]) =>
    (d.fileExtensions ?? []).map((ext) => ({ ext, dbType })),
  );
}
