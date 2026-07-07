// Types mirrored from FilterEditor.svelte — kept in sync manually.
type FilterOperator =
  | '='
  | '!='
  | '>'
  | '<'
  | '>='
  | '<='
  | 'LIKE'
  | 'NOT LIKE'
  | 'IS NULL'
  | 'IS NOT NULL'
  | 'IN';

type FilterRule = {
  id: string;
  rawSql?: string;
  column: string;
  operator: FilterOperator;
  value: string;
};

type FilterGroup = {
  id: string;
  conjunction: 'AND' | 'OR';
  rules: FilterRule[];
};

export type FilterEditorState = {
  mode: 'builder' | 'sql';
  groupJunction: 'AND' | 'OR';
  groups: FilterGroup[];
  sql: string;
};

type SavedFilterState = {
  filterEditorState: FilterEditorState;
  searchTerm: string;
  page?: number;
};

export const tableBrowserFilterCache = new Map<string, SavedFilterState>();

export function clearTableFilterCache(key: string): void {
  tableBrowserFilterCache.delete(key);
}
