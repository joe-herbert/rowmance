// Live-view polling manager for the table browser and query editor.
//
// Poll timers live here at module scope (not inside a component) because
// Panel.svelte fully unmounts/destroys TableBrowser/QueryEditor when a tab
// loses focus or is switched away from ({#key ...} blocks in Panel.svelte).
// A per-component interval would stop the moment the tab is backgrounded;
// keeping it here lets Live mode keep polling and toasting in the background,
// writing results into the same tableDataCache/queryEditorCache the
// components already read from on mount.
import { fetchTableSnapshot } from '$lib/utils/tableSnapshot';
import { executeMultiQuery } from '$lib/tauri/query';
import { stripLineComments } from '$lib/utils/sql';
import { queryEditorCache } from '$lib/stores/queryEditorState';
import { useToast } from '$lib/stores/toast.svelte';
import { useSettings } from '$lib/stores/settings.svelte';
import type { DialectInfo, QueryResult, ForeignKeyInfo } from '$lib/types';
import type { FilterEditorState } from '$lib/components/table/FilterEditor.svelte';

export interface TableLiveParams {
  connectionId: string;
  database: string;
  table: string;
  instanceDb?: string | null;
  getDialect: () => DialectInfo | undefined;
  getFilterState: () => FilterEditorState;
  getSearchTerm: () => string;
  getPage: () => number;
  getPageSize: () => number;
}

export interface QueryLiveParams {
  connectionId: string;
  database: string | null;
  instanceDb?: string | null;
  sql: string;
  /** Saved query name, if any — shown in the change toast as "on <name>". */
  queryName?: string;
}

interface TableLiveEntry {
  timerId: ReturnType<typeof setInterval>;
  signature: string | null;
  rowCount: number | null;
  rows: unknown[][] | null;
  params: TableLiveParams;
}

interface QueryLiveEntry {
  timerId: ReturnType<typeof setInterval>;
  signature: string | null;
  rowCount: number | null;
  rows: unknown[][] | null;
  sql: string;
  connectionId: string;
  database: string | null;
  instanceDb?: string | null;
  queryName?: string;
}

// Plain reactive flags so toggle buttons re-render across component remounts.
const tableLiveFlags = $state<Record<string, boolean>>({});
const queryLiveFlags = $state<Record<string, boolean>>({});

const tableTimers = new Map<string, TableLiveEntry>();
const queryTimers = new Map<string, QueryLiveEntry>();

export interface TableSnapshotUpdate {
  result: QueryResult;
  unfilteredTotal: number | null;
  foreignKeys: ForeignKeyInfo[];
}

type TableListener = (snapshot: TableSnapshotUpdate) => void;
type QueryListener = (results: QueryResult[]) => void;

// Pushes fresh poll results to any currently-mounted component watching this
// key, since a mounted TableBrowser/QueryEditor only reads the shared caches
// once at mount — it has no way to notice them changing underneath it.
const tableListeners = new Map<string, Set<TableListener>>();
const queryListeners = new Map<string, Set<QueryListener>>();

function addListener<T>(registry: Map<string, Set<T>>, key: string, fn: T): () => void {
  let set = registry.get(key);
  if (!set) {
    set = new Set();
    registry.set(key, set);
  }
  set.add(fn);
  return () => {
    set!.delete(fn);
    if (set!.size === 0) registry.delete(key);
  };
}

function rowsSignature(rows: unknown[][]): string {
  return JSON.stringify(rows);
}

/** Counts rows that differ at the same index between two same-length pages. */
function countRowDiffs(prevRows: unknown[][] | null, newRows: unknown[][]): number {
  if (!prevRows || prevRows.length !== newRows.length) return 0;
  let count = 0;
  for (let i = 0; i < newRows.length; i++) {
    if (JSON.stringify(prevRows[i]) !== JSON.stringify(newRows[i])) count++;
  }
  return count;
}

type ChangeKind = 'added' | 'deleted' | 'updated' | 'changed';

function describeTableChange(kind: ChangeKind, count: number, table: string): string {
  const noun = count === 1 ? 'row' : 'rows';
  switch (kind) {
    case 'added':
      return `${count} ${noun} added to ${table}`;
    case 'deleted':
      return `${count} ${noun} deleted from ${table}`;
    case 'updated':
      return `${count} ${noun} updated in ${table}`;
    case 'changed':
      return `${table} updated`;
  }
}

function describeQueryChange(kind: ChangeKind, count: number, queryName?: string): string {
  const noun = count === 1 ? 'row' : 'rows';
  const target = queryName ?? 'query';
  switch (kind) {
    case 'added':
      return `${count} ${noun} added on ${target}`;
    case 'deleted':
      return `${count} ${noun} deleted on ${target}`;
    case 'updated':
      return `${count} ${noun} updated on ${target}`;
    case 'changed':
      return `Updated on ${target}`;
  }
}

function notifyChange(
  prevSignature: string | null,
  prevCount: number | null,
  prevRows: unknown[][] | null,
  newSignature: string,
  newCount: number | null,
  newRows: unknown[][],
  formatMessage: (kind: ChangeKind, count: number) => string,
): void {
  if (prevSignature === null) return; // first poll after enabling — just record the baseline
  if (prevSignature === newSignature) return; // unchanged

  const toast = useToast();
  if (newCount !== null && prevCount !== null && newCount !== prevCount) {
    const delta = newCount - prevCount;
    if (delta > 0) {
      toast.addToast(formatMessage('added', delta), 'info');
    } else {
      toast.addToast(formatMessage('deleted', -delta), 'info');
    }
    return;
  }

  const updatedCount = countRowDiffs(prevRows, newRows);
  if (updatedCount > 0) {
    toast.addToast(formatMessage('updated', updatedCount), 'info');
  } else {
    toast.addToast(formatMessage('changed', 0), 'info');
  }
}

async function pollTable(key: string): Promise<void> {
  const entry = tableTimers.get(key);
  if (!entry) return;
  const { params } = entry;
  try {
    const snapshot = await fetchTableSnapshot({
      connectionId: params.connectionId,
      database: params.database,
      table: params.table,
      instanceDb: params.instanceDb,
      dialect: params.getDialect(),
      filterEditorState: params.getFilterState(),
      searchTerm: params.getSearchTerm(),
      page: params.getPage(),
      pageSize: params.getPageSize(),
      previousTotalRows: null,
      previousUnfilteredTotal: entry.rowCount,
      forceCount: true,
    });
    if (snapshot.error || !snapshot.result) return;

    const newRows = snapshot.result.rows;
    const newSignature = rowsSignature(newRows);
    const newCount = snapshot.unfilteredTotal;
    notifyChange(
      entry.signature,
      entry.rowCount,
      entry.rows,
      newSignature,
      newCount,
      newRows,
      (kind, count) => describeTableChange(kind, count, params.table),
    );
    entry.signature = newSignature;
    entry.rowCount = newCount;
    entry.rows = newRows;

    const listeners = tableListeners.get(key);
    if (listeners) {
      const update: TableSnapshotUpdate = {
        result: snapshot.result,
        unfilteredTotal: snapshot.unfilteredTotal,
        foreignKeys: snapshot.foreignKeys,
      };
      for (const fn of listeners) fn(update);
    }
  } catch {
    // Transient fetch errors are ignored — the next tick will retry.
  }
}

async function pollQuery(key: string): Promise<void> {
  const entry = queryTimers.get(key);
  if (!entry) return;
  try {
    const results = await executeMultiQuery(
      entry.connectionId,
      entry.sql,
      entry.database,
      entry.instanceDb,
    );
    const primary = results[0];
    if (!primary || primary.error) return;

    const newRows = primary.rows;
    const newSignature = JSON.stringify(results.map((r) => r.rows));
    const newCount = primary.totalRows ?? primary.affectedRows ?? null;
    notifyChange(
      entry.signature,
      entry.rowCount,
      entry.rows,
      newSignature,
      newCount,
      newRows,
      (kind, count) => describeQueryChange(kind, count, entry.queryName),
    );
    entry.signature = newSignature;
    entry.rowCount = newCount;
    entry.rows = newRows;

    const cached = queryEditorCache.get(key);
    if (cached) {
      queryEditorCache.save(key, { ...cached, results });
    }

    const listeners = queryListeners.get(key);
    if (listeners) for (const fn of listeners) fn(results);
  } catch {
    // Transient fetch errors are ignored — the next tick will retry.
  }
}

export function useLiveView() {
  return {
    isTableLive(key: string): boolean {
      return !!tableLiveFlags[key];
    },

    toggleTableLive(key: string, params: TableLiveParams): void {
      const existing = tableTimers.get(key);
      if (existing) {
        clearInterval(existing.timerId);
        tableTimers.delete(key);
        tableLiveFlags[key] = false;
        return;
      }
      const intervalMs = useSettings().settings.tableLivePollIntervalMs || 5000;
      const timerId = setInterval(() => pollTable(key), intervalMs);
      tableTimers.set(key, { timerId, signature: null, rowCount: null, rows: null, params });
      tableLiveFlags[key] = true;
      void pollTable(key);
    },

    isQueryLive(key: string): boolean {
      return !!queryLiveFlags[key];
    },

    toggleQueryLive(key: string, params: QueryLiveParams): void {
      const existing = queryTimers.get(key);
      if (existing) {
        clearInterval(existing.timerId);
        queryTimers.delete(key);
        queryLiveFlags[key] = false;
        return;
      }
      const intervalMs = useSettings().settings.queryLivePollIntervalMs || 5000;
      const timerId = setInterval(() => pollQuery(key), intervalMs);
      queryTimers.set(key, {
        timerId,
        signature: null,
        rowCount: null,
        rows: null,
        sql: stripLineComments(params.sql),
        connectionId: params.connectionId,
        database: params.database,
        instanceDb: params.instanceDb,
        queryName: params.queryName,
      });
      queryLiveFlags[key] = true;
      void pollQuery(key);
    },

    /** Keep a live query's polled statement in sync with manual re-runs/edits. */
    updateQuerySource(key: string, sql: string): void {
      const entry = queryTimers.get(key);
      if (entry) entry.sql = stripLineComments(sql);
    },

    /** Called by a mounted TableBrowser to receive fresh poll results directly. */
    subscribeTable(key: string, fn: TableListener): () => void {
      return addListener(tableListeners, key, fn);
    },

    /** Called by a mounted QueryEditor to receive fresh poll results directly. */
    subscribeQuery(key: string, fn: QueryListener): () => void {
      return addListener(queryListeners, key, fn);
    },
  };
}
