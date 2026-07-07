import type { QueryResult } from '$lib/types';

export type SavedQueryEditorState = {
  sql: string;
  results: QueryResult[];
  executedStatements: string[];
  selectedDatabase: string;
  activeResultTab?: number;
};

const cache = new Map<string, SavedQueryEditorState>();

export const queryEditorCache = {
  get(editorId: string): SavedQueryEditorState | undefined {
    return cache.get(editorId);
  },
  save(editorId: string, state: SavedQueryEditorState) {
    cache.set(editorId, state);
  },
  delete(editorId: string) {
    cache.delete(editorId);
  },
};
