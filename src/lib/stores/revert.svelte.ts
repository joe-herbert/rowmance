export interface RevertColumnChange {
  column: string;
  previousValue: unknown;
  newValue: unknown;
}

export interface RevertRowChange {
  operation: 'update' | 'insert' | 'delete';
  pkValues: Record<string, unknown>;
  columnChanges: RevertColumnChange[];
}

export interface RevertEntry {
  id: string;
  source: 'table' | 'query';
  connectionId: string;
  database: string;
  table: string;
  sql: string;
  revertSql: string;
  rows: RevertRowChange[];
  executedAt: Date;
  reverted: boolean;
}

type RevertState = 'idle' | 'active' | 'paused';

let state = $state<RevertState>('idle');
let connectionId = $state<string | null>(null);
let entries = $state<RevertEntry[]>([]);
let modalOpen = $state(false);

export function useRevert() {
  return {
    get state() {
      return state;
    },
    get isActive() {
      return state === 'active' || state === 'paused';
    },
    get isPaused() {
      return state === 'paused';
    },
    get isTracking() {
      return state === 'active';
    },
    get connectionId() {
      return connectionId;
    },
    get entries() {
      return entries;
    },
    get modalOpen() {
      return modalOpen;
    },

    isRevertingConnection(connId: string) {
      return connectionId === connId && (state === 'active' || state === 'paused');
    },

    start(connId: string) {
      connectionId = connId;
      entries = [];
      state = 'active';
    },

    pause() {
      if (state === 'active') state = 'paused';
    },

    resume() {
      if (state === 'paused') state = 'active';
    },

    stop() {
      state = 'idle';
      if (entries.length > 0) {
        modalOpen = true;
      } else {
        connectionId = null;
      }
    },

    add(entry: RevertEntry) {
      if (state !== 'active' || connectionId !== entry.connectionId) return;
      entries = [entry, ...entries];
    },

    openModal() {
      modalOpen = true;
    },

    closeModal() {
      modalOpen = false;
      if (state === 'idle') connectionId = null;
    },

    clear() {
      entries = [];
    },

    markReverted(id: string) {
      entries = entries.map((e) => (e.id === id ? { ...e, reverted: true } : e));
    },
  };
}
