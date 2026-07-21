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

  export type FilterValueKind = 'text' | 'raw' | 'column';

  export interface FilterRule {
    id: string;
    /** When set, this rule is displayed/stored as raw SQL rather than column/operator/value. */
    rawSql?: string;
    column: string;
    operator: FilterOperator;
    value: string;
    /** How `value` should be interpreted when building SQL. Defaults to 'text'. */
    valueKind?: FilterValueKind;
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
    return (
      r.column !== '' &&
      (r.operator === 'IS NULL' || r.operator === 'IS NOT NULL' || r.value.trim() !== '')
    );
  }

  export function filterStateIsActive(state: FilterEditorState): boolean {
    if (state.mode === 'sql') return state.sql.trim().length > 0;
    return state.groups.some((g) => g.rules.some(isActiveRule));
  }

  export function activeRuleCount(state: FilterEditorState): number {
    if (state.mode === 'sql') return parseSqlConditions(state.sql).length;
    return state.groups.reduce((sum, g) => sum + g.rules.filter(isActiveRule).length, 0);
  }

  function buildRuleValueSql(r: FilterRule, quoteIdentifier: (_n: string) => string): string {
    const kind = r.valueKind ?? 'text';
    if (kind === 'raw') return r.value;
    if (kind === 'column') return quoteIdentifier(r.value);
    return `'${r.value.replaceAll("'", "''")}'`;
  }

  function buildRuleSql(r: FilterRule, quoteIdentifier: (_n: string) => string): string {
    if (r.rawSql !== undefined) return r.rawSql.trim();
    const col = quoteIdentifier(r.column);
    if (r.operator === 'IS NULL') return `${col} IS NULL`;
    if (r.operator === 'IS NOT NULL') return `${col} IS NOT NULL`;
    if (r.operator === 'IN') {
      const values = r.value
        .split(',')
        .map((v) => `'${v.trim().replaceAll("'", "''")}'`)
        .join(', ');
      return `${col} IN (${values})`;
    }
    return `${col} ${r.operator} ${buildRuleValueSql(r, quoteIdentifier)}`;
  }

  export function buildWhereClause(
    state: FilterEditorState,
    quoteIdentifier: (_n: string) => string,
  ): string {
    if (state.mode === 'sql') return state.sql.trim();

    const activeGroups = state.groups
      .map((g) => ({
        conjunction: g.conjunction,
        parts: g.rules.filter(isActiveRule).map((r) => buildRuleSql(r, quoteIdentifier)),
      }))
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
          if (trimmed[i + 1] === stringChar) {
            current += trimmed[i + 1];
            i += 2;
            continue;
          }
          inString = false;
        }
        i++;
        continue;
      }

      if (ch === "'" || ch === '"' || ch === '`') {
        inString = true;
        stringChar = ch;
        current += ch;
        i++;
        continue;
      }
      if (ch === '(') {
        depth++;
        current += ch;
        i++;
        continue;
      }
      if (ch === ')') {
        depth--;
        current += ch;
        i++;
        continue;
      }

      if (depth === 0) {
        const upper = trimmed.slice(i).toUpperCase();
        if (/^AND[\s(]/.test(upper)) {
          if (current.trim()) parts.push({ keyword: pendingKeyword, condition: current.trim() });
          current = '';
          pendingKeyword = 'AND';
          i += 3;
          while (i < trimmed.length && /\s/.test(trimmed[i])) i++;
          continue;
        }
        if (/^OR[\s(]/.test(upper)) {
          if (current.trim()) parts.push({ keyword: pendingKeyword, condition: current.trim() });
          current = '';
          pendingKeyword = 'OR';
          i += 2;
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
          if (t[i + 1] === stringChar) {
            i++;
            continue;
          }
          inString = false;
        }
        continue;
      }
      if (ch === "'" || ch === '"' || ch === '`') {
        inString = true;
        stringChar = ch;
        continue;
      }
      if (ch === '(') depth++;
      else if (ch === ')') depth--;
      if (depth === 0) return null;
    }
    return t.slice(1, -1).trim();
  }

  function unquoteColumn(s: string): string {
    if ((s.startsWith('`') && s.endsWith('`')) || (s.startsWith('"') && s.endsWith('"')))
      return s.slice(1, -1);
    return s;
  }

  function unquoteValue(s: string): string | null {
    const t = s.trim();
    if (t.startsWith("'") && t.endsWith("'") && t.length >= 2)
      return t.slice(1, -1).replaceAll("''", "'");
    if ((t.startsWith('"') && t.endsWith('"')) || (t.startsWith('`') && t.endsWith('`')))
      return t.slice(1, -1);
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
    if (notLikeMatch) {
      const val = unquoteValue(notLikeMatch[1]);
      if (val !== null) return { column, operator: 'NOT LIKE', value: val };
    }

    const likeMatch = after.match(/^LIKE\s+(.+)$/i);
    if (likeMatch) {
      const val = unquoteValue(likeMatch[1]);
      if (val !== null) return { column, operator: 'LIKE', value: val };
    }

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
    return {
      id: crypto.randomUUID(),
      rawSql: condition,
      column: '',
      operator: '=' as FilterOperator,
      value: '',
    };
  }

  /** Converts a SQL WHERE string into groups of builder rules. */
  export function parseSqlToGroups(sql: string): {
    groupJunction: 'AND' | 'OR';
    groups: FilterGroup[];
  } {
    const topConditions = parseSqlConditions(sql);
    const topKeywords = topConditions.map((c) => c.keyword).filter((k) => k !== 'WHERE');
    const groupJunction: 'AND' | 'OR' = topKeywords[0] === 'OR' ? 'OR' : 'AND';

    const groups: FilterGroup[] = topConditions.map(({ condition }) => {
      const inner = stripOuterParens(condition);
      if (inner !== null) {
        const innerConds = parseSqlConditions(inner);
        const innerKeywords = innerConds.map((c) => c.keyword).filter((k) => k !== 'WHERE');
        const conjunction: 'AND' | 'OR' = innerKeywords[0] === 'OR' ? 'OR' : 'AND';
        return {
          id: crypto.randomUUID(),
          conjunction,
          rules: innerConds.map((c) => conditionToRule(c.condition)),
        };
      }
      return {
        id: crypto.randomUUID(),
        conjunction: 'AND' as const,
        rules: [conditionToRule(condition)],
      };
    });

    return { groupJunction, groups };
  }
</script>

<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import type { ColumnMeta, DialectInfo } from '$lib/types';
  import { qi as dialectQi } from '$lib/utils/dialect';
  import Select from '$lib/components/ui/Select.svelte';
  import SegmentedControl from '$lib/components/ui/SegmentedControl.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import DatePicker from '$lib/components/ui/DatePicker.svelte';
  import TimePicker from '$lib/components/ui/TimePicker.svelte';
  import DateTimePicker from '$lib/components/ui/DateTimePicker.svelte';
  import { portal } from '$lib/actions/portal';
  import SqlHighlight from '$lib/components/ui/SqlHighlight.svelte';
  import PlusIcon from '$lib/components/icons/PlusIcon.svelte';
  import CloseIcon from '$lib/components/icons/CloseIcon.svelte';
  import CalendarIcon from '$lib/components/icons/CalendarIcon.svelte';

  interface Props {
    columns: ColumnMeta[];
    value: FilterEditorState;
    dialectInfo: DialectInfo;
    tableName?: string;
    schemaName?: string;
    onApply: (_state: FilterEditorState) => void;
    onClose: () => void;
  }

  let { columns, value, dialectInfo, tableName, schemaName, onApply, onClose }: Props = $props();

  const settings = useSettings();

  function quoteIdentifier(name: string): string {
    return dialectQi(name, dialectInfo);
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

  let draft = $state<FilterEditorState>(
    untrack(() => ({
      mode: value.mode,
      groupJunction: value.groupJunction,
      groups:
        value.groups.length > 0
          ? value.groups.map((g) => ({ ...g, rules: g.rules.map((r) => ({ ...r })) }))
          : [{ id: crypto.randomUUID(), conjunction: 'AND', rules: [newRule()] }],
      sql: value.sql,
    })),
  );
  let panelEl = $state<HTMLDivElement | null>(null);
  let openPickerRuleId = $state<string | null>(null);
  let pickerTriggerEl = $state<HTMLElement | null>(null);
  let pickerPopupEl = $state<HTMLDivElement | null>(null);
  let pickerTop = $state(0);
  let pickerLeft = $state(0);
  let pickerOpenUp = $state(false);

  const OPERATORS: FilterOperator[] = [
    '=',
    '!=',
    '>',
    '<',
    '>=',
    '<=',
    'LIKE',
    'NOT LIKE',
    'IS NULL',
    'IS NOT NULL',
    'IN',
  ];
  const VALUE_LESS_OPS: FilterOperator[] = ['IS NULL', 'IS NOT NULL'];
  const VALUE_KIND_INFO: Record<
    FilterValueKind,
    { label: string; next: FilterValueKind; desc: string }
  > = {
    text: { label: 'Text', next: 'raw', desc: 'Text — value is treated as a literal string' },
    raw: { label: 'Raw', next: 'column', desc: 'Raw — value is inserted into the SQL as-is' },
    column: {
      label: 'Col',
      next: 'text',
      desc: 'Column — value is treated as a reference to another column',
    },
  };

  function needsValue(op: FilterOperator): boolean {
    return !VALUE_LESS_OPS.includes(op);
  }

  function getDateCategory(columnName: string): 'datetime' | 'date' | 'time' | null {
    const col = columns.find((c) => c.name === columnName);
    if (!col) return null;
    const dt = col.dataType.toLowerCase();
    if (dt.includes('datetime') || dt.includes('timestamp')) return 'datetime';
    if (dt.startsWith('time')) return 'time';
    if (dt.startsWith('date')) return 'date';
    return null;
  }

  function positionPicker(): void {
    if (!pickerTriggerEl) return;
    const rect = pickerTriggerEl.getBoundingClientRect();
    const h = pickerPopupEl ? pickerPopupEl.offsetHeight : 300;
    const spaceBelow = window.innerHeight - rect.bottom - 8;
    pickerOpenUp = spaceBelow < h && rect.top > h;
    pickerTop = pickerOpenUp ? rect.top - h - 4 : rect.bottom + 4;
    pickerLeft = rect.left;
  }

  function openPicker(ruleId: string, triggerEl: HTMLElement): void {
    openPickerRuleId = ruleId;
    pickerTriggerEl = triggerEl;
    requestAnimationFrame(() => {
      positionPicker();
      requestAnimationFrame(positionPicker);
    });
  }

  function closePicker(): void {
    openPickerRuleId = null;
    pickerTriggerEl = null;
  }

  function isBooleanColumn(columnName: string): boolean {
    const col = columns.find((c) => c.name === columnName);
    if (!col) return false;
    const dt = col.dataType.toLowerCase();
    return /^bool/.test(dt) || dt === 'tinyint(1)';
  }

  function getEnumValues(columnName: string): string[] | null {
    const col = columns.find((c) => c.name === columnName);
    if (!col) return null;
    const match = col.dataType.match(/^enum\((.+)\)$/i);
    if (!match) return null;
    const raw = match[1];
    const values: string[] = [];
    let i = 0;
    while (i < raw.length) {
      if (raw[i] === "'") {
        let j = i + 1;
        while (j < raw.length && raw[j] !== "'") j++;
        values.push(raw.slice(i + 1, j));
        i = j + 2; // skip closing quote and comma
      } else {
        i++;
      }
    }
    return values.length > 0 ? values : null;
  }

  function booleanTrueLabel(): string {
    const fmt = settings.settings.booleanDisplay ?? 'tick-cross';
    if (fmt === 'true-false') return 'True';
    if (fmt === '1-0') return '1';
    if (fmt === 'as-saved') return 'true';
    return '✓';
  }

  function booleanFalseLabel(): string {
    const fmt = settings.settings.booleanDisplay ?? 'tick-cross';
    if (fmt === 'true-false') return 'False';
    if (fmt === '1-0') return '0';
    if (fmt === 'as-saved') return 'false';
    return '✗';
  }

  function newRule(): FilterRule {
    return { id: crypto.randomUUID(), column: columns[0]?.name ?? '', operator: '=', value: '' };
  }

  function addGroup(): void {
    draft.groups = [
      ...draft.groups,
      { id: crypto.randomUUID(), conjunction: 'AND', rules: [newRule()] },
    ];
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
      g.id === groupId
        ? { ...g, rules: g.rules.map((r) => (r.id === ruleId ? { ...r, column } : r)) }
        : g,
    );
  }

  function updateRuleOperator(groupId: string, ruleId: string, operator: FilterOperator): void {
    draft.groups = draft.groups.map((g) =>
      g.id === groupId
        ? {
            ...g,
            rules: g.rules.map((r) =>
              r.id === ruleId ? { ...r, operator, value: needsValue(operator) ? r.value : '' } : r,
            ),
          }
        : g,
    );
  }

  function updateRuleValue(groupId: string, ruleId: string, val: string): void {
    draft.groups = draft.groups.map((g) =>
      g.id === groupId
        ? { ...g, rules: g.rules.map((r) => (r.id === ruleId ? { ...r, value: val } : r)) }
        : g,
    );
  }

  function updateRuleValueKind(groupId: string, ruleId: string, valueKind: FilterValueKind): void {
    draft.groups = draft.groups.map((g) =>
      g.id === groupId
        ? {
            ...g,
            rules: g.rules.map((r) => (r.id === ruleId ? { ...r, valueKind, value: '' } : r)),
          }
        : g,
    );
  }

  function updateRuleRawSql(groupId: string, ruleId: string, rawSql: string): void {
    draft.groups = draft.groups.map((g) =>
      g.id === groupId
        ? { ...g, rules: g.rules.map((r) => (r.id === ruleId ? { ...r, rawSql } : r)) }
        : g,
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
    if (openPickerRuleId !== null) {
      if (!pickerTriggerEl?.contains(target) && !pickerPopupEl?.contains(target)) closePicker();
      return;
    }
    if (panelEl && !panelEl.contains(target)) onClose();
  }

  function handleDocumentKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape') {
      if (openPickerRuleId !== null) {
        closePicker();
        return;
      }
      onClose();
    }
  }

  $effect(() => {
    if (pickerPopupEl) requestAnimationFrame(positionPicker);
  });

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
      <button
        class="fe-add-group-btn"
        onclick={addGroup}
        title="Add group"
        aria-label="Add filter group"
      >
        <PlusIcon width={11} height={11} strokeWidth={2.5} />
        Add group
      </button>
    {/if}
    <SegmentedControl
      options={[
        { value: 'builder', label: 'Builder' },
        { value: 'sql', label: 'SQL' },
      ]}
      value={draft.mode}
      onchange={(v) => switchMode(v as 'builder' | 'sql')}
    />
    <button class="fe-close-btn" onclick={onClose} aria-label="Close filters"
      ><CloseIcon width={12} height={12} strokeWidth={2.5} /></button
    >
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
                title="Click to toggle AND / OR">{draft.groupJunction}</button
              >
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
                      aria-label="Toggle AND/OR">{group.conjunction}</button
                    >
                  {/if}

                  {#if rule.rawSql !== undefined}
                    <span class="raw-sql-tag">SQL</span>
                    <input
                      class="fe-value-input fe-value-input--raw"
                      type="text"
                      value={rule.rawSql}
                      oninput={(e) =>
                        updateRuleRawSql(group.id, rule.id, (e.target as HTMLInputElement).value)}
                      aria-label="Raw SQL condition"
                      autocomplete="off"
                      autocapitalize="off"
                      autocorrect="off"
                      spellcheck="false"
                    />
                  {:else}
                    <Select
                      value={rule.column}
                      options={columns.map((col) => ({ value: col.name, label: col.name }))}
                      onchange={(v) => updateRuleColumn(group.id, rule.id, v)}
                      aria-label="Column"
                      size="xs"
                      mono
                      style="flex:1; min-width:0; width:100%"
                      searchable
                    />

                    <Select
                      value={rule.operator}
                      options={OPERATORS.map((op) => ({ value: op, label: op }))}
                      onchange={(v) => updateRuleOperator(group.id, rule.id, v as FilterOperator)}
                      aria-label="Operator"
                      size="xs"
                      mono
                      style="width:100px; flex-shrink:0"
                      searchable
                    />

                    {#if needsValue(rule.operator)}
                      {@const valueKind = rule.valueKind ?? 'text'}
                      {@const enumValues = getEnumValues(rule.column)}
                      {#if valueKind === 'column' && rule.operator !== 'IN'}
                        <Select
                          value={rule.value || undefined}
                          options={columns
                            .filter((col) => col.name !== rule.column)
                            .map((col) => ({ value: col.name, label: col.name }))}
                          onchange={(v) => updateRuleValue(group.id, rule.id, v)}
                          aria-label="Filter value column"
                          size="xs"
                          mono
                          style="flex:1; min-width:0"
                          searchable
                        />
                      {:else if valueKind === 'raw' && rule.operator !== 'IN'}
                        <input
                          class="fe-value-input fe-value-input--raw"
                          type="text"
                          value={rule.value}
                          oninput={(e) =>
                            updateRuleValue(
                              group.id,
                              rule.id,
                              (e.target as HTMLInputElement).value,
                            )}
                          placeholder="expression"
                          aria-label="Filter value expression"
                          autocomplete="off"
                          autocapitalize="off"
                          autocorrect="off"
                          spellcheck="false"
                        />
                      {:else if isBooleanColumn(rule.column)}
                        <div class="fe-bool-picker" aria-label="Filter value">
                          <button
                            class="fe-bool-btn fe-bool-btn--true"
                            class:fe-bool-btn--selected={rule.value === 'true' ||
                              rule.value === '1'}
                            onclick={() =>
                              updateRuleValue(
                                group.id,
                                rule.id,
                                settings.settings.booleanDisplay === '1-0' ? '1' : 'true',
                              )}
                            type="button">{booleanTrueLabel()}</button
                          >
                          <button
                            class="fe-bool-btn fe-bool-btn--false"
                            class:fe-bool-btn--selected={rule.value === 'false' ||
                              rule.value === '0'}
                            onclick={() =>
                              updateRuleValue(
                                group.id,
                                rule.id,
                                settings.settings.booleanDisplay === '1-0' ? '0' : 'false',
                              )}
                            type="button">{booleanFalseLabel()}</button
                          >
                        </div>
                      {:else if enumValues !== null}
                        <Select
                          value={rule.value || undefined}
                          options={[
                            { value: '', label: '— pick value —' },
                            ...enumValues.map((v) => ({ value: v, label: v })),
                          ]}
                          onchange={(v) => updateRuleValue(group.id, rule.id, v)}
                          aria-label="Filter value"
                          size="xs"
                          mono
                          style="flex:1; min-width:0"
                          searchable
                        />
                      {:else}
                        {@const dateCategory = getDateCategory(rule.column)}
                        {#if dateCategory !== null}
                          <div class="fe-date-input-wrap">
                            <input
                              class="fe-value-input fe-date-input"
                              type="text"
                              value={rule.value}
                              placeholder={dateCategory === 'datetime'
                                ? 'YYYY-MM-DD HH:MM:SS'
                                : dateCategory === 'time'
                                  ? 'HH:MM:SS'
                                  : 'YYYY-MM-DD'}
                              oninput={(e) =>
                                updateRuleValue(
                                  group.id,
                                  rule.id,
                                  (e.target as HTMLInputElement).value,
                                )}
                              aria-label="Filter value"
                              autocomplete="off"
                              autocapitalize="off"
                              autocorrect="off"
                              spellcheck="false"
                            />
                            <button
                              class="fe-date-picker-btn"
                              type="button"
                              aria-label="Open date picker"
                              onclick={(e) =>
                                openPicker(
                                  rule.id,
                                  e.currentTarget.closest('.fe-date-input-wrap') as HTMLElement,
                                )}
                            >
                              <CalendarIcon width={11} height={11} />
                            </button>
                          </div>
                        {:else}
                          <input
                            class="fe-value-input"
                            type="text"
                            value={rule.value}
                            placeholder={rule.operator === 'LIKE' || rule.operator === 'NOT LIKE'
                              ? '%value%'
                              : rule.operator === 'IN'
                                ? "'a','b','c'"
                                : 'value'}
                            oninput={(e) =>
                              updateRuleValue(
                                group.id,
                                rule.id,
                                (e.target as HTMLInputElement).value,
                              )}
                            aria-label="Filter value"
                            autocomplete="off"
                            autocapitalize="off"
                            autocorrect="off"
                            spellcheck="false"
                          />
                        {/if}
                      {/if}

                      {#if rule.operator !== 'IN'}
                        <button
                          type="button"
                          class="fe-value-kind-btn"
                          onclick={() =>
                            updateRuleValueKind(group.id, rule.id, VALUE_KIND_INFO[valueKind].next)}
                          title={`Value type: ${VALUE_KIND_INFO[valueKind].desc}\nClick to cycle: Text → Raw → Column`}
                          aria-label="Cycle filter value type"
                          >{VALUE_KIND_INFO[valueKind].label}</button
                        >
                      {/if}
                    {:else}
                      <span class="fe-value-spacer"></span>
                    {/if}
                  {/if}

                  <button
                    class="rule-remove-btn"
                    onclick={() => removeRule(group.id, rule.id)}
                    aria-label="Remove filter"
                    title="Remove"><CloseIcon width={10} height={10} strokeWidth={2.5} /></button
                  >
                </div>
              {/each}
            </div>

            <div class="fe-add-row fe-group-footer">
              <button class="fe-add-btn" onclick={() => addRule(group.id)}>
                <PlusIcon width={11} height={11} strokeWidth={2.5} />
                Add filter
              </button>
              {#if draft.groups.length > 1}
                <button
                  class="group-remove-btn"
                  onclick={() => removeGroup(group.id)}
                  aria-label="Delete group">Delete group</button
                >
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
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
        rows="3"
        onkeydown={(e) => {
          if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            applyFilter();
          }
        }}
      ></textarea>
      {#if tableName}
        {@const tableRef = schemaName
          ? `${quoteIdentifier(schemaName)}.${quoteIdentifier(tableName)}`
          : quoteIdentifier(tableName)}
        <div class="sql-preview-hint">
          Executed as: <code class="sql-preview-code"
            ><SqlHighlight
              sql={`SELECT * FROM ${tableRef} WHERE ${draft.sql.trim() || '…'}`}
            /></code
          >
        </div>
      {/if}
    </div>
  {/if}

  <div class="fe-footer">
    <button class="fe-clear-btn" onclick={clearFilter}>Clear</button>
    <button class="fe-apply-btn" onclick={applyFilter}>Apply</button>
  </div>
</div>

{#if openPickerRuleId !== null}
  {@const activeRule = draft.groups.flatMap((g) => g.rules).find((r) => r.id === openPickerRuleId)}
  {@const activeGroup = draft.groups.find((g) => g.rules.some((r) => r.id === openPickerRuleId))}
  {@const dateCategory = activeRule ? getDateCategory(activeRule.column) : null}
  {#if activeRule && activeGroup && dateCategory}
    <div
      bind:this={pickerPopupEl}
      class="picker-popup"
      class:picker-popup--up={pickerOpenUp}
      style="top:{pickerTop}px; left:{pickerLeft}px"
      use:portal
      onkeydown={(e) => {
        if (e.key === 'Escape') closePicker();
      }}
      role="dialog"
      aria-label="Pick {dateCategory === 'datetime' ? 'date and time' : dateCategory}"
      tabindex="-1"
    >
      {#if dateCategory === 'date'}
        <DatePicker
          value={activeRule.value}
          onchange={(v) => updateRuleValue(activeGroup.id, activeRule.id, v)}
        />
      {:else if dateCategory === 'time'}
        <TimePicker
          value={activeRule.value}
          onchange={(v) => updateRuleValue(activeGroup.id, activeRule.id, v)}
        />
      {:else}
        <DateTimePicker
          value={activeRule.value}
          onchange={(v) => updateRuleValue(activeGroup.id, activeRule.id, v)}
        />
      {/if}
    </div>
  {/if}
{/if}

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
    transition:
      border-color var(--transition-fast),
      color var(--transition-fast),
      background var(--transition-fast);
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

  .fe-close-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

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

  .fe-body--sql {
    gap: var(--spacing-1);
  }

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
    transition:
      border-color var(--transition-fast),
      background var(--transition-fast),
      color var(--transition-fast);
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

  .conjunction-pill:hover {
    background: var(--color-accent);
    color: var(--color-text-on-accent);
  }

  .conjunction-spacer {
    flex-shrink: 0;
    width: 38px;
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    font-family: var(--font-family-mono);
    color: var(--color-text-muted);
    text-align: center;
    -webkit-user-select: none;
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

  .fe-value-input:focus {
    border-color: var(--color-accent);
  }
  .fe-value-input::placeholder {
    color: var(--color-text-muted);
    font-family: var(--font-family-ui);
  }
  .fe-value-input--raw {
    border-style: dashed;
  }
  .fe-value-spacer {
    flex: 1;
  }

  .fe-bool-picker {
    display: flex;
    flex: 1;
    gap: var(--spacing-1);
    min-width: 0;
  }

  .fe-bool-btn {
    flex: 1;
    padding: 3px var(--spacing-2);
    background: var(--color-bg-primary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    font-weight: var(--font-weight-medium);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      border-color var(--transition-fast);
  }

  .fe-bool-btn--true {
    color: var(--color-success);
  }
  .fe-bool-btn--false {
    color: var(--color-danger);
  }

  .fe-bool-btn:hover {
    background: var(--color-bg-hover);
    border-color: var(--color-border-strong);
  }

  .fe-bool-btn--true.fe-bool-btn--selected {
    background: var(--color-success-subtle);
    border-color: var(--color-success);
  }

  .fe-bool-btn--false.fe-bool-btn--selected {
    background: var(--color-danger-subtle);
    border-color: var(--color-danger);
  }

  .fe-date-input-wrap {
    flex: 1;
    min-width: 0;
    position: relative;
    display: flex;
    align-items: center;
  }

  .fe-date-input {
    padding-right: calc(var(--spacing-1) + 18px);
  }

  .fe-date-picker-btn {
    position: absolute;
    right: 3px;
    top: 50%;
    transform: translateY(-50%);
    display: flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    padding: 0;
    background: transparent;
    border: none;
    color: var(--color-text-muted);
    cursor: pointer;
    border-radius: var(--radius-sm);
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .fe-date-picker-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .fe-value-kind-btn {
    flex-shrink: 0;
    width: 34px;
    padding: 2px var(--spacing-1);
    background: var(--color-bg-tertiary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: 9px;
    font-weight: var(--font-weight-semibold);
    font-family: var(--font-family-mono);
    color: var(--color-text-muted);
    cursor: pointer;
    text-align: center;
    transition:
      border-color var(--transition-fast),
      color var(--transition-fast),
      background var(--transition-fast);
  }

  .fe-value-kind-btn:hover {
    border-color: var(--color-accent);
    color: var(--color-accent);
    background: var(--color-accent-subtle);
  }

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
    -webkit-user-select: none;
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
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
    line-height: 1;
  }

  .rule-remove-btn:hover {
    background: var(--color-danger-subtle);
    color: var(--color-danger);
  }

  .fe-add-row {
    padding-top: var(--spacing-1);
  }

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
    transition:
      border-color var(--transition-fast),
      color var(--transition-fast),
      background var(--transition-fast);
  }

  .fe-add-btn:hover {
    border-color: var(--color-accent);
    color: var(--color-accent);
    background: var(--color-accent-subtle);
  }

  /* ── SQL mode ───────────────────────────────────────────────────────────── */

  .sql-label {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-muted);
    font-family: var(--font-family-mono);
    -webkit-user-select: none;
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

  .sql-textarea:focus {
    border-color: var(--color-accent);
  }
  .sql-textarea::placeholder {
    color: var(--color-text-muted);
    font-family: var(--font-family-ui);
  }

  .sql-preview-hint {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-family: var(--font-family-ui);
    line-height: var(--line-height-normal);
    word-break: break-all;
  }

  .sql-preview-code {
    font-family: var(--font-family-mono);
    color: var(--color-text-secondary);
  }

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
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .fe-clear-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

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
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .fe-apply-btn:hover {
    background: var(--color-accent);
    color: var(--color-text-on-accent);
  }
</style>
