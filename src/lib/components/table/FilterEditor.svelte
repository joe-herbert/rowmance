<script lang="ts" module>
  export type FilterOperator =
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

  export interface FilterRule {
    id: string;
    /** When set, this rule is displayed/stored as raw SQL rather than column/operator/value. */
    rawSql?: string;
    column: string;
    operator: FilterOperator;
    value: string;
  }

  export interface FilterGroup {
    id: string;
    /** How rules within this group are joined. */
    conjunction: 'AND' | 'OR';
    rules: FilterRule[];
  }

  export interface FilterEditorState {
    mode: 'builder' | 'sql';
    /** How groups are joined to each other. */
    groupJunction: 'AND' | 'OR';
    groups: FilterGroup[];
    sql: string;
  }

  export function emptyFilterState(): FilterEditorState {
    return { mode: 'builder', groupJunction: 'AND', groups: [], sql: '' };
  }

  function isActiveRule(r: FilterRule): boolean {
    if (r.rawSql !== undefined) return r.rawSql.trim() !== '';
    return r.column !== '' && (r.operator === 'IS NULL' || r.operator === 'IS NOT NULL' || r.value.trim() !== '');
  }

  export function filterStateIsActive(state: FilterEditorState): boolean {
    if (state.mode === 'sql') return state.sql.trim().length > 0;
    return state.groups.some((g) => g.rules.some(isActiveRule));
  }

  export function activeRuleCount(state: FilterEditorState): number {
    if (state.mode === 'sql') return parseSqlConditions(state.sql).length;
    return state.groups.reduce((sum, g) => sum + g.rules.filter(isActiveRule).length, 0);
  }

  function buildRuleSql(r: FilterRule, quoteIdentifier: (_n: string) => string): string {
    if (r.rawSql !== undefined) return r.rawSql.trim();
    const col = quoteIdentifier(r.column);
    if (r.operator === 'IS NULL') return `${col} IS NULL`;
    if (r.operator === 'IS NOT NULL') return `${col} IS NOT NULL`;
    if (r.operator === 'IN') return `${col} IN (${r.value})`;
    const escaped = r.value.replaceAll("'", "''");
    return `${col} ${r.operator} '${escaped}'`;
  }

  export function buildWhereClause(
    state: FilterEditorState,
    quoteIdentifier: (_n: string) => string,
  ): string {
    if (state.mode === 'sql') return state.sql.trim();

    const activeGroups = state.groups
      .map((g) => ({ conjunction: g.conjunction, parts: g.rules.filter(isActiveRule).map((r) => buildRuleSql(r, quoteIdentifier)) }))
      .filter((g) => g.parts.length > 0);

    if (activeGroups.length === 0) return '';

    const groupStrings = activeGroups.map((g) => {
      const joined = g.parts.join(` ${g.conjunction} `);
      return activeGroups.length > 1 && g.parts.length > 1 ? `(${joined})` : joined;
    });

    return groupStrings.join(` ${state.groupJunction} `);
  }

  // ── SQL parser ───────────────────────────────────────────────────────────────

  /** Splits a WHERE clause on top-level AND/OR, respecting string literals and parens. */
  export function parseSqlConditions(sql: string): { keyword: string; condition: string }[] {
    const trimmed = sql.trim();
    if (!trimmed) return [];

    const parts: { keyword: string; condition: string }[] = [];
    let depth = 0;
    let inString = false;
    let stringChar = '';
    let current = '';
    let pendingKeyword = 'WHERE';
    let i = 0;

    while (i < trimmed.length) {
      const ch = trimmed[i];

      if (inString) {
        current += ch;
        if (ch === stringChar) {
          if (trimmed[i + 1] === stringChar) { current += trimmed[i + 1]; i += 2; continue; }
          inString = false;
        }
        i++;
        continue;
      }

      if (ch === "'" || ch === '"' || ch === '`') { inString = true; stringChar = ch; current += ch; i++; continue; }
      if (ch === '(') { depth++; current += ch; i++; continue; }
      if (ch === ')') { depth--; current += ch; i++; continue; }

      if (depth === 0) {
        const upper = trimmed.slice(i).toUpperCase();
        if (/^AND[\s(]/.test(upper)) {
          if (current.trim()) parts.push({ keyword: pendingKeyword, condition: current.trim() });
          current = ''; pendingKeyword = 'AND'; i += 3;
          while (i < trimmed.length && /\s/.test(trimmed[i])) i++;
          continue;
        }
        if (/^OR[\s(]/.test(upper)) {
          if (current.trim()) parts.push({ keyword: pendingKeyword, condition: current.trim() });
          current = ''; pendingKeyword = 'OR'; i += 2;
          while (i < trimmed.length && /\s/.test(trimmed[i])) i++;
          continue;
        }
      }

      current += ch;
      i++;
    }

    if (current.trim()) parts.push({ keyword: pendingKeyword, condition: current.trim() });
    return parts;
  }

  /** Returns the inner content if `s` is entirely wrapped in matching outer parens, else null. */
  function stripOuterParens(s: string): string | null {
    const t = s.trim();
    if (!t.startsWith('(') || !t.endsWith(')')) return null;
    let depth = 0;
    let inString = false;
    let stringChar = '';
    for (let i = 0; i < t.length - 1; i++) {
      const ch = t[i];
      if (inString) {
        if (ch === stringChar) {
          if (t[i + 1] === stringChar) { i++; continue; }
          inString = false;
        }
        continue;
      }
      if (ch === "'" || ch === '"' || ch === '`') { inString = true; stringChar = ch; continue; }
      if (ch === '(') depth++;
      else if (ch === ')') depth--;
      if (depth === 0) return null;
    }
    return t.slice(1, -1).trim();
  }

  function unquoteColumn(s: string): string {
    if ((s.startsWith('`') && s.endsWith('`')) || (s.startsWith('"') && s.endsWith('"'))) return s.slice(1, -1);
    return s;
  }

  function unquoteValue(s: string): string | null {
    const t = s.trim();
    if (t.startsWith("'") && t.endsWith("'") && t.length >= 2) return t.slice(1, -1).replaceAll("''", "'");
    if ((t.startsWith('"') && t.endsWith('"')) || (t.startsWith('`') && t.endsWith('`'))) return t.slice(1, -1);
    if (/^[\w.%-]+$/.test(t)) return t;
    return null;
  }

  function parseSqlCondition(sql: string): Omit<FilterRule, 'id'> | null {
    const t = sql.trim();
    const colMatch = t.match(/^(`(?:[^`]|``)*`|"(?:[^"]|"")*"|[\w]+)/);
    if (!colMatch) return null;
    const column = unquoteColumn(colMatch[1]);
    const after = t.slice(colMatch[0].length).trim();

    if (/^IS\s+NULL$/i.test(after)) return { column, operator: 'IS NULL', value: '' };
    if (/^IS\s+NOT\s+NULL$/i.test(after)) return { column, operator: 'IS NOT NULL', value: '' };

    const inMatch = after.match(/^IN\s*\((.+)\)$/is);
    if (inMatch) return { column, operator: 'IN', value: inMatch[1].trim() };

    const notLikeMatch = after.match(/^NOT\s+LIKE\s+(.+)$/i);
    if (notLikeMatch) { const val = unquoteValue(notLikeMatch[1]); if (val !== null) return { column, operator: 'NOT LIKE', value: val }; }

    const likeMatch = after.match(/^LIKE\s+(.+)$/i);
    if (likeMatch) { const val = unquoteValue(likeMatch[1]); if (val !== null) return { column, operator: 'LIKE', value: val }; }

    const opMatch = after.match(/^(!=|<>|>=|<=|>|<|=)\s*(.+)$/);
    if (opMatch) {
      let op = opMatch[1] as FilterOperator;
      if (op === ('<>' as FilterOperator)) op = '!=';
      const val = unquoteValue(opMatch[2]);
      if (val !== null) return { column, operator: op, value: val };
    }

    return null;
  }

  function conditionToRule(condition: string): FilterRule {
    const parsed = parseSqlCondition(condition);
    if (parsed) return { id: crypto.randomUUID(), ...parsed };
    return { id: crypto.randomUUID(), rawSql: condition, column: '', operator: '=' as FilterOperator, value: '' };
  }

  /** Converts a SQL WHERE string into groups of builder rules. */
  export function parseSqlToGroups(sql: string): { groupJunction: 'AND' | 'OR'; groups: FilterGroup[] } {
    const topConditions = parseSqlConditions(sql);
    const topKeywords = topConditions.map((c) => c.keyword).filter((k) => k !== 'WHERE');
    const groupJunction: 'AND' | 'OR' = topKeywords[0] === 'OR' ? 'OR' : 'AND';

    const groups: FilterGroup[] = topConditions.map(({ condition }) => {
      const inner = stripOuterParens(condition);
      if (inner !== null) {
        const innerConds = parseSqlConditions(inner);
        const innerKeywords = innerConds.map((c) => c.keyword).filter((k) => k !== 'WHERE');
        const conjunction: 'AND' | 'OR' = innerKeywords[0] === 'OR' ? 'OR' : 'AND';
        return { id: crypto.randomUUID(), conjunction, rules: innerConds.map((c) => conditionToRule(c.condition)) };
      }
      return { id: crypto.randomUUID(), conjunction: 'AND' as const, rules: [conditionToRule(condition)] };
    });

    return { groupJunction, groups };
  }
</script>

<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import type { ColumnMeta } from '$lib/types';
  import Select from '$lib/components/ui/Select.svelte';
  import SegmentedControl from '$lib/components/ui/SegmentedControl.svelte';

  interface Props {
    columns: ColumnMeta[];
    value: FilterEditorState;
    dbType: string;
    onApply: (_state: FilterEditorState) => void;
    onClose: () => void;
  }

  let { columns, value, dbType, onApply, onClose }: Props = $props();

  function quoteIdentifier(name: string): string {
    return dbType === 'postgres' ? `"${name}"` : `\`${name}\``;
  }

  function switchMode(newMode: 'builder' | 'sql'): void {
    if (newMode === draft.mode) return;
    if (newMode === 'sql') {
      draft.sql = buildWhereClause({ ...draft, mode: 'builder' }, quoteIdentifier);
    } else {
      if (draft.sql.trim()) {
        const { groupJunction, groups } = parseSqlToGroups(draft.sql);
        draft.groupJunction = groupJunction;
        draft.groups = groups;
      }
      if (draft.groups.length === 0) {
        draft.groups = [{ id: crypto.randomUUID(), conjunction: 'AND', rules: [newRule()] }];
      }
    }
    draft.mode = newMode;
  }

  let draft = $state<FilterEditorState>(untrack(() => ({
    mode: value.mode,
    groupJunction: value.groupJunction,
    groups: value.groups.length > 0
      ? value.groups.map((g) => ({ ...g, rules: g.rules.map((r) => ({ ...r })) }))
      : [{ id: crypto.randomUUID(), conjunction: 'AND', rules: [newRule()] }],
    sql: value.sql,
  })));
  let panelEl = $state<HTMLDivElement | null>(null);

  const OPERATORS: FilterOperator[] = ['=', '!=', '>', '<', '>=', '<=', 'LIKE', 'NOT LIKE', 'IS NULL', 'IS NOT NULL', 'IN'];
  const VALUE_LESS_OPS: FilterOperator[] = ['IS NULL', 'IS NOT NULL'];

  function needsValue(op: FilterOperator): boolean {
    return !VALUE_LESS_OPS.includes(op);
  }

  function getValueInputType(columnName: string): 'datetime-local' | 'date' | 'text' {
    const col = columns.find((c) => c.name === columnName);
    if (!col) return 'text';
    const dt = col.dataType.toLowerCase();
    if (dt.includes('datetime') || dt.includes('timestamp')) return 'datetime-local';
    if (dt.startsWith('date')) return 'date';
    return 'text';
  }

  function newRule(): FilterRule {
    return { id: crypto.randomUUID(), column: columns[0]?.name ?? '', operator: '=', value: '' };
  }

  function addGroup(): void {
    draft.groups = [...draft.groups, { id: crypto.randomUUID(), conjunction: 'AND', rules: [newRule()] }];
  }

  function removeGroup(groupId: string): void {
    draft.groups = draft.groups.filter((g) => g.id !== groupId);
  }

  function addRule(groupId: string): void {
    draft.groups = draft.groups.map((g) =>
      g.id === groupId ? { ...g, rules: [...g.rules, newRule()] } : g,
    );
  }

  function removeRule(groupId: string, ruleId: string): void {
    draft.groups = draft.groups.map((g) =>
      g.id === groupId ? { ...g, rules: g.rules.filter((r) => r.id !== ruleId) } : g,
    );
  }

  function toggleGroupConjunction(groupId: string): void {
    draft.groups = draft.groups.map((g) =>
      g.id === groupId ? { ...g, conjunction: g.conjunction === 'AND' ? 'OR' : 'AND' } : g,
    );
  }

  function updateRuleColumn(groupId: string, ruleId: string, column: string): void {
    draft.groups = draft.groups.map((g) =>
      g.id === groupId ? { ...g, rules: g.rules.map((r) => r.id === ruleId ? { ...r, column, value: '' } : r) } : g,
    );
  }

  function updateRuleOperator(groupId: string, ruleId: string, operator: FilterOperator): void {
    draft.groups = draft.groups.map((g) =>
      g.id === groupId
        ? { ...g, rules: g.rules.map((r) => r.id === ruleId ? { ...r, operator, value: needsValue(operator) ? r.value : '' } : r) }
        : g,
    );
  }

  function updateRuleValue(groupId: string, ruleId: string, val: string): void {
    draft.groups = draft.groups.map((g) =>
      g.id === groupId ? { ...g, rules: g.rules.map((r) => r.id === ruleId ? { ...r, value: val } : r) } : g,
    );
  }

  function updateRuleRawSql(groupId: string, ruleId: string, rawSql: string): void {
    draft.groups = draft.groups.map((g) =>
      g.id === groupId ? { ...g, rules: g.rules.map((r) => r.id === ruleId ? { ...r, rawSql } : r) } : g,
    );
  }

  function applyFilter(): void {
    onApply({
      mode: draft.mode,
      groupJunction: draft.groupJunction,
      groups: draft.groups.map((g) => ({ ...g, rules: g.rules.map((r) => ({ ...r })) })),
      sql: draft.sql,
    });
    onClose();
  }

  function clearFilter(): void {
    onApply(emptyFilterState());
    onClose();
  }

  function handleDocumentClick(e: MouseEvent): void {
    const target = e.target as Node;
    if (!target.isConnected) return;
    if (panelEl && !panelEl.contains(target)) onClose();
  }

  function handleDocumentKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape') onClose();
  }

  onMount(() => {
    const timer = setTimeout(() => {
      document.addEventListener('click', handleDocumentClick);
      document.addEventListener('keydown', handleDocumentKeydown);
    }, 0);
    return () => {
      clearTimeout(timer);
      document.removeEventListener('click', handleDocumentClick);
      document.removeEventListener('keydown', handleDocumentKeydown);
    };
  });
</script>

<div bind:this={panelEl} class="filter-editor" role="dialog" aria-label="Table filters">
  <div class="fe-header">
    <span class="fe-title">Filters</span>
    {#if draft.mode === 'builder'}
      <button class="fe-add-group-btn" onclick={addGroup} title="Add group" aria-label="Add filter group">
        <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        Add group
      </button>
    {/if}
    <SegmentedControl
      options={[{ value: 'builder', label: 'Builder' }, { value: 'sql', label: 'SQL' }]}
      value={draft.mode}
      onchange={(v) => switchMode(v as 'builder' | 'sql')}
    />
    <button class="fe-close-btn" onclick={onClose} aria-label="Close filters">✕</button>
  </div>

  {#if draft.mode === 'builder'}
    <div class="fe-body">
      {#if draft.groups.length === 0}
        <div class="fe-empty">No filters applied. Add a filter below.</div>
      {:else}
        {#each draft.groups as group, gi (group.id)}
          {#if gi > 0}
            <div class="group-junction-row">
              <button
                class="conjunction-pill"
                onclick={() => (draft.groupJunction = draft.groupJunction === 'AND' ? 'OR' : 'AND')}
                title="Click to toggle AND / OR"
              >{draft.groupJunction}</button>
            </div>
          {/if}

          <div class="filter-group" class:filter-group--multi={draft.groups.length > 1}>
            <div class="rule-list">
              {#each group.rules as rule, ri (rule.id)}
                <div class="rule-row">
                  {#if ri === 0}
                    <span class="conjunction-spacer">WHERE</span>
                  {:else}
                    <button
                      class="conjunction-pill"
                      onclick={() => toggleGroupConjunction(group.id)}
                      title="Click to toggle AND / OR"
                      aria-label="Toggle AND/OR"
                    >{group.conjunction}</button>
                  {/if}

                  {#if rule.rawSql !== undefined}
                    <span class="raw-sql-tag">SQL</span>
                    <input
                      class="fe-value-input fe-value-input--raw"
                      type="text"
                      value={rule.rawSql}
                      oninput={(e) => updateRuleRawSql(group.id, rule.id, (e.target as HTMLInputElement).value)}
                      aria-label="Raw SQL condition"
                      autocomplete="off" autocapitalize="off" autocorrect="off" spellcheck="false"
                    />
                  {:else}
                    <Select
                      value={rule.column}
                      options={columns.map(col => ({ value: col.name, label: col.name }))}
                      onchange={(v) => updateRuleColumn(group.id, rule.id, v)}
                      aria-label="Column"
                      size="xs"
                      mono
                      style="flex:1; min-width:0; width:100%"
                    />

                    <Select
                      value={rule.operator}
                      options={OPERATORS.map(op => ({ value: op, label: op }))}
                      onchange={(v) => updateRuleOperator(group.id, rule.id, v as FilterOperator)}
                      aria-label="Operator"
                      size="xs"
                      mono
                      style="width:100px; flex-shrink:0"
                    />

                    {#if needsValue(rule.operator)}
                      <input
                        class="fe-value-input"
                        type={getValueInputType(rule.column)}
                        value={rule.value}
                        placeholder={rule.operator === 'LIKE' || rule.operator === 'NOT LIKE' ? '%value%' : rule.operator === 'IN' ? "'a','b','c'" : 'value'}
                        oninput={(e) => updateRuleValue(group.id, rule.id, (e.target as HTMLInputElement).value)}
                        aria-label="Filter value"
                        autocomplete="off" autocapitalize="off" autocorrect="off" spellcheck="false"
                      />
                    {:else}
                      <span class="fe-value-spacer"></span>
                    {/if}
                  {/if}

                  <button class="rule-remove-btn" onclick={() => removeRule(group.id, rule.id)} aria-label="Remove filter" title="Remove">✕</button>
                </div>
              {/each}
            </div>

            <div class="fe-add-row fe-group-footer">
              <button class="fe-add-btn" onclick={() => addRule(group.id)}>
                <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
                Add filter
              </button>
              {#if draft.groups.length > 1}
                <button class="group-remove-btn" onclick={() => removeGroup(group.id)} aria-label="Delete group">Delete group</button>
              {/if}
            </div>
          </div>
        {/each}
      {/if}

    </div>
  {:else}
    <div class="fe-body fe-body--sql">
      <label class="sql-label" for="fe-sql-input">WHERE</label>
      <textarea
        id="fe-sql-input"
        class="sql-textarea"
        bind:value={draft.sql}
        placeholder="condition… (Shift+Enter for newline, Enter to apply)"
        autocomplete="off" autocapitalize="off" spellcheck="false"
        rows="3"
        onkeydown={(e) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); applyFilter(); } }}
      ></textarea>
    </div>
  {/if}

  <div class="fe-footer">
    <button class="fe-clear-btn" onclick={clearFilter}>Clear</button>
    <button class="fe-apply-btn" onclick={applyFilter}>Apply</button>
  </div>
</div>

<style>
  .filter-editor {
    position: absolute;
    z-index: 200;
    background: var(--color-bg-overlay);
    backdrop-filter: var(--glass-blur);
    -webkit-backdrop-filter: var(--glass-blur);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-overlay);
    min-width: 380px;
    max-width: 580px;
    display: flex;
    flex-direction: column;
    overflow: clip;
  }

  .fe-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    flex-shrink: 0;
  }

  .fe-title {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    flex: 1;
  }

  .fe-add-group-btn {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: 2px var(--spacing-2);
    background: transparent;
    border: 1px dashed var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    color: var(--color-text-muted);
    cursor: pointer;
    white-space: nowrap;
    transition: border-color var(--transition-fast), color var(--transition-fast), background var(--transition-fast);
  }

  .fe-add-group-btn:hover {
    border-color: var(--color-accent);
    color: var(--color-accent);
    background: var(--color-accent-subtle);
  }

  .fe-close-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    padding: 0;
    background: transparent;
    border: none;
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    cursor: pointer;
    border-radius: var(--radius-sm);
    transition: background var(--transition-fast);
    line-height: 1;
  }

  .fe-close-btn:hover { background: var(--color-bg-hover); color: var(--color-text-primary); }

  .fe-body {
    padding: var(--spacing-2) var(--spacing-3);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
    max-height: 400px;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: var(--color-scrollbar-thumb) var(--color-scrollbar-track);
  }

  .fe-body--sql { gap: var(--spacing-1); }

  .fe-empty {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    padding: var(--spacing-2) 0;
    font-family: var(--font-family-ui);
  }

  /* ── Groups ─────────────────────────────────────────────────────────────── */

  .group-junction-row {
    display: flex;
    align-items: center;
    padding: 0 var(--spacing-1);
  }

  .filter-group {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-1);
  }

  .filter-group--multi {
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    padding: var(--spacing-2);
    background: var(--color-bg-secondary);
  }

  .fe-group-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .group-remove-btn {
    padding: 3px var(--spacing-2);
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    color: var(--color-text-muted);
    cursor: pointer;
    transition: border-color var(--transition-fast), background var(--transition-fast), color var(--transition-fast);
  }

  .group-remove-btn:hover {
    border-color: var(--color-danger);
    background: var(--color-danger-subtle);
    color: var(--color-danger);
  }

  /* ── Rules ──────────────────────────────────────────────────────────────── */

  .rule-list {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-1);
  }

  .rule-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
  }

  .conjunction-pill {
    flex-shrink: 0;
    width: 38px;
    padding: 2px var(--spacing-1);
    background: var(--color-accent-subtle);
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-sm);
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    font-family: var(--font-family-mono);
    color: var(--color-accent);
    cursor: pointer;
    text-align: center;
    transition: background var(--transition-fast);
  }

  .conjunction-pill:hover { background: var(--color-accent); color: var(--color-text-on-accent); }

  .conjunction-spacer {
    flex-shrink: 0;
    width: 38px;
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    font-family: var(--font-family-mono);
    color: var(--color-text-muted);
    text-align: center;
    user-select: none;
  }

  .fe-value-input {
    flex: 1;
    min-width: 0;
    padding: 3px var(--spacing-1);
    background: var(--color-bg-primary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-primary);
    outline: none;
    transition: border-color var(--transition-fast);
  }

  .fe-value-input:focus { border-color: var(--color-accent); }
  .fe-value-input::placeholder { color: var(--color-text-muted); font-family: var(--font-family-ui); }
  .fe-value-input--raw { border-style: dashed; }
  .fe-value-spacer { flex: 1; }

  .raw-sql-tag {
    flex-shrink: 0;
    font-size: 9px;
    font-weight: var(--font-weight-semibold);
    font-family: var(--font-family-mono);
    padding: 1px var(--spacing-1);
    border-radius: var(--radius-sm);
    background: var(--color-bg-tertiary);
    border: 1px solid var(--color-border);
    color: var(--color-text-muted);
    user-select: none;
  }

  .rule-remove-btn {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    padding: 0;
    background: transparent;
    border: none;
    font-size: 10px;
    color: var(--color-text-muted);
    cursor: pointer;
    border-radius: var(--radius-sm);
    transition: background var(--transition-fast), color var(--transition-fast);
    line-height: 1;
  }

  .rule-remove-btn:hover { background: var(--color-danger-subtle); color: var(--color-danger); }

  .fe-add-row { padding-top: var(--spacing-1); }

  .fe-add-btn {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: 3px var(--spacing-2);
    background: transparent;
    border: 1px dashed var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    color: var(--color-text-muted);
    cursor: pointer;
    transition: border-color var(--transition-fast), color var(--transition-fast), background var(--transition-fast);
  }

  .fe-add-btn:hover { border-color: var(--color-accent); color: var(--color-accent); background: var(--color-accent-subtle); }

  /* ── SQL mode ───────────────────────────────────────────────────────────── */

  .sql-label {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-muted);
    font-family: var(--font-family-mono);
    user-select: none;
  }

  .sql-textarea {
    width: 100%;
    padding: var(--spacing-2);
    background: var(--color-bg-primary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-primary);
    outline: none;
    resize: vertical;
    min-height: 60px;
    transition: border-color var(--transition-fast);
    box-sizing: border-box;
  }

  .sql-textarea:focus { border-color: var(--color-accent); }
  .sql-textarea::placeholder { color: var(--color-text-muted); font-family: var(--font-family-ui); }

  /* ── Footer ─────────────────────────────────────────────────────────────── */

  .fe-footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-3);
    border-top: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    flex-shrink: 0;
  }

  .fe-clear-btn {
    padding: 3px var(--spacing-2);
    background: transparent;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    color: var(--color-text-secondary);
    cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast);
  }

  .fe-clear-btn:hover { background: var(--color-bg-hover); color: var(--color-text-primary); }

  .fe-apply-btn {
    padding: 3px var(--spacing-3);
    background: var(--color-accent-subtle);
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    font-family: var(--font-family-ui);
    color: var(--color-accent);
    cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast);
  }

  .fe-apply-btn:hover { background: var(--color-accent); color: var(--color-text-on-accent); }
</style>
