interface RecordedStatement {
  sql: string;
  connectionId: string;
  database: string | null;
  executedAt: Date;
}

type RecordingState = 'idle' | 'recording' | 'paused';

let state = $state<RecordingState>('idle');
let connectionId = $state<string | null>(null);
let statements = $state<RecordedStatement[]>([]);
let reviewOpen = $state(false);

export function useRecording() {
  return {
    get state() {
      return state;
    },
    get isRecording() {
      return state === 'recording';
    },
    get isPaused() {
      return state === 'paused';
    },
    get isActive() {
      return state === 'recording' || state === 'paused';
    },
    get connectionId() {
      return connectionId;
    },
    get statements() {
      return statements;
    },
    get reviewOpen() {
      return reviewOpen;
    },

    isRecordingConnection(connId: string) {
      return connectionId === connId && (state === 'recording' || state === 'paused');
    },

    start(connId: string) {
      connectionId = connId;
      statements = [];
      state = 'recording';
    },

    pause() {
      if (state === 'recording') state = 'paused';
    },

    resume() {
      if (state === 'paused') state = 'recording';
    },

    stop() {
      state = 'idle';
      if (statements.length > 0) {
        reviewOpen = true;
      } else {
        connectionId = null;
      }
    },

    add(sql: string, connId: string, database: string | null) {
      if (state !== 'recording' || connectionId !== connId) return;
      statements = [...statements, { sql, connectionId: connId, database, executedAt: new Date() }];
    },

    closeReview() {
      reviewOpen = false;
      statements = [];
      connectionId = null;
    },
  };
}
