<script lang="ts">
  import { onMount, onDestroy, untrack } from 'svelte';
  import {
    EditorView,
    keymap,
    lineNumbers,
    highlightActiveLine,
    highlightActiveLineGutter,
    drawSelection,
    dropCursor,
    rectangularSelection,
    crosshairCursor,
  } from '@codemirror/view';
  import { EditorState, type Extension } from '@codemirror/state';
  import { highlightSelectionMatches } from '@codemirror/search';
  import { defaultKeymap, indentWithTab } from '@codemirror/commands';
  import { sql as sqlLang } from '@codemirror/lang-sql';
  import {
    defaultHighlightStyle,
    syntaxHighlighting,
    bracketMatching,
    foldGutter,
  } from '@codemirror/language';
  import {
    closeBrackets,
    closeBracketsKeymap,
    autocompletion,
    completionKeymap,
    type CompletionContext,
    type CompletionResult,
    type CompletionSource,
  } from '@codemirror/autocomplete';
  import { format as sqlFormat } from 'sql-formatter';
  import type { QueryResult } from '$lib/types';
  import { executeMultiQuery, explainQuery } from '$lib/tauri/query';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import ResultsPanel from '$lib/components/editor/ResultsPanel.svelte';
  import * as schemaApi from '$lib/tauri/schema';
  import { splitStatements, statementAtCursor } from '$lib/utils/sql';
  import { errorMessage } from '$lib/utils/errors';
  import Select from '$lib/components/ui/Select.svelte';
  import { portal } from '$lib/actions/portal';

  interface Props {
    connectionId: string;
    database?: string;
    initialSql?: string;
    onExecute?: (_sql: string) => void;
  }

  let { connectionId, database: initialDatabase, initialSql = '', onExecute }: Props = $props();

  const connections = useConnections();
  const settingsStore = useSettings();
  const panelStore = usePanels();

  let editorContainer = $state<HTMLDivElement | undefined>(undefined);
  let editorView = $state<EditorView | undefined>(undefined);
  let sqlText = $state(untrack(() => initialSql));
  let results = $state<QueryResult[]>([]);
  let executedStatements = $state<string[]>([]);
  let isRunning = $state(false);
  let transactionActive = $state(false);

  let databases = $state<string[]>([]);
  let selectedDatabase = $state<string>(untrack(() => initialDatabase ?? connections.getById(connectionId)?.database ?? ''));

  let toolbarEl = $state<HTMLDivElement | undefined>(undefined);
  let toolbarWidth = $state(9999);
  let actionsMenuOpen = $state(false);
  let actionsMenuTriggerEl = $state<HTMLButtonElement | undefined>(undefined);
  let actionsMenuEl = $state<HTMLDivElement | undefined>(undefined);
  let actionsMenuTop = $state(0);
  let actionsMenuLeft = $state(0);

  const compact = $derived(toolbarWidth < 700);

  $effect(() => {
    if (!toolbarEl) return;
    const ro = new ResizeObserver(entries => {
      toolbarWidth = entries[0].contentRect.width;
    });
    ro.observe(toolbarEl);
    return () => ro.disconnect();
  });

  $effect(() => {
    if (!actionsMenuOpen) return;

    function positionMenu() {
      if (!actionsMenuTriggerEl || !actionsMenuEl) return;
      const rect = actionsMenuTriggerEl.getBoundingClientRect();
      actionsMenuTop = rect.bottom + 4;
      actionsMenuLeft = rect.right - actionsMenuEl.offsetWidth;
    }

    requestAnimationFrame(positionMenu);

    function onMousedown(e: MouseEvent) {
      const t = e.target as Node;
      if (!actionsMenuTriggerEl?.contains(t) && !actionsMenuEl?.contains(t)) {
        actionsMenuOpen = false;
      }
    }

    document.addEventListener('mousedown', onMousedown, true);
    return () => document.removeEventListener('mousedown', onMousedown, true);
  });

  let connectionName = $derived(connections.getById(connectionId)?.name ?? connectionId);
  let transactionMode = $derived(settingsStore.settings.transactionMode);
  let showTransactionToolbar = $derived(transactionMode || transactionActive);

  const DB_TYPE_DIALECT: Record<string, string> = {
    mysql: 'mysql',
    mariadb: 'mysql',
    postgres: 'postgresql',
  };

  let sqlDialect = $derived(() => {
    const profile = connections.getById(connectionId);
    return profile ? (DB_TYPE_DIALECT[profile.dbType] ?? 'sql') : 'sql';
  });

  // ── Schema-aware autocomplete ─────────────────────────────────────────────

  interface SchemaTable {
    database: string;
    name: string;
  }

  const schemaRef: {
    connectionId: string;
    tables: SchemaTable[];
    columns: Map<string, string[]>;
  } = { connectionId: '', tables: [], columns: new Map() };

  async function loadSchemaForCompletion(connId: string): Promise<void> {
    schemaRef.connectionId = connId;
    schemaRef.tables = [];
    schemaRef.columns = new Map();
    try {
      const dbs = await schemaApi.listDatabases(connId);
      databases = dbs;
      if (!selectedDatabase && dbs.length > 0) {
        selectedDatabase = dbs[0];
      }
      const tables: SchemaTable[] = [];
      for (const db of dbs) {
        const dbTables = await schemaApi.listTables(connId, db);
        for (const t of dbTables) {
          tables.push({ database: db, name: t.name });
        }
      }
      schemaRef.tables = tables;
    } catch {
      // Schema load failed — completions just won't show table/column names.
    }
  }

  $effect(() => {
    if (connections.isActive(connectionId)) {
      loadSchemaForCompletion(connectionId);
    }
  });

  function makeSchemaCompletionSource(): CompletionSource {
    return async (context: CompletionContext): Promise<CompletionResult | null> => {
      const dotMatch = context.matchBefore(/[\w"`]+\.[\w"`]*/);
      if (dotMatch) {
        const rawTable = dotMatch.text.split('.')[0].replace(/["`]/g, '');
        const schemaTable = schemaRef.tables.find(
          (t) => t.name.toLowerCase() === rawTable.toLowerCase(),
        );
        if (schemaTable) {
          const cacheKey = `${schemaTable.database}.${schemaTable.name}`;
          let cols = schemaRef.columns.get(cacheKey);
          if (!cols) {
            try {
              const colInfos = await schemaApi.listColumns(
                schemaRef.connectionId,
                schemaTable.database,
                schemaTable.name,
              );
              cols = colInfos.map((c) => c.name);
              schemaRef.columns.set(cacheKey, cols);
            } catch {
              cols = [];
            }
          }
          return {
            from: dotMatch.from + rawTable.length + 1,
            options: cols.map((col) => ({ label: col, type: 'property' })),
            validFor: /^[\w"`]*$/,
          };
        }
      }

      const word = context.matchBefore(/\w*/);
      if (!word || (word.from === word.to && !context.explicit)) return null;
      if (schemaRef.tables.length === 0) return null;

      return {
        from: word.from,
        options: schemaRef.tables.map((t) => ({
          label: t.name,
          detail: t.database,
          type: 'class',
        })),
        validFor: /^\w*$/,
      };
    };
  }

  // ── Theme ─────────────────────────────────────────────────────────────────

  function resolveCSSVar(name: string): string {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  }

  function buildTheme(): Extension {
    return EditorView.theme({
      '&': {
        backgroundColor: resolveCSSVar('--color-editor-bg'),
        color: resolveCSSVar('--color-editor-text'),
        height: '100%',
        fontFamily: 'var(--font-family-mono)',
        fontSize: 'var(--font-size-md)',
      },
      '.cm-content': {
        caretColor: resolveCSSVar('--color-editor-cursor'),
        padding: '0',
      },
      '.cm-cursor, .cm-dropCursor': {
        borderLeftColor: resolveCSSVar('--color-editor-cursor'),
      },
      '&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection': {
        backgroundColor: resolveCSSVar('--color-editor-selection'),
      },
      '.cm-gutters': {
        backgroundColor: resolveCSSVar('--color-editor-gutter-bg'),
        color: resolveCSSVar('--color-editor-gutter-text'),
        border: 'none',
        borderRight: `1px solid var(--color-border)`,
      },
      '.cm-lineNumbers .cm-gutterElement': {
        padding: '0 var(--spacing-2)',
        minWidth: '2.5ch',
      },
      '.cm-activeLine': {
        backgroundColor: resolveCSSVar('--color-editor-active-line'),
      },
      '.cm-activeLineGutter': {
        backgroundColor: resolveCSSVar('--color-editor-active-line'),
      },
      '.cm-matchingBracket': {
        backgroundColor: resolveCSSVar('--color-editor-bracket-match'),
        outline: 'none',
      },
      '.cm-scroller': {
        overflow: 'auto',
        fontFamily: 'var(--font-family-mono)',
      },
      '.cm-tooltip': {
        backgroundColor: 'var(--color-bg-overlay)',
        border: '1px solid var(--color-border)',
        boxShadow: 'var(--shadow-md)',
        borderRadius: 'var(--radius-sm)',
      },
      '.cm-tooltip-autocomplete ul li[aria-selected]': {
        backgroundColor: 'var(--color-bg-active)',
        color: 'var(--color-text-primary)',
      },
    });
  }

  // ── Query execution ───────────────────────────────────────────────────────

  async function runQuery(): Promise<void> {
    const query = sqlText.trim();
    if (!query || isRunning) return;

    isRunning = true;
    executedStatements = splitStatements(query);
    try {
      results = await executeMultiQuery(connectionId, query, selectedDatabase || null);
      onExecute?.(query);
    } catch (err) {
      results = [{
        queryId: '',
        columns: [],
        rows: [],
        totalRows: null,
        durationMs: 0,
        affectedRows: null,
        error: errorMessage(err),
      }];
    } finally {
      isRunning = false;
    }
  }

  async function runSelection(): Promise<void> {
    if (!editorView || isRunning) return;
    const state = editorView.state;
    const { from, to } = state.selection.main;
    const selected = from === to ? sqlText : state.sliceDoc(from, to);
    const query = selected.trim();
    if (!query) return;

    isRunning = true;
    executedStatements = splitStatements(query);
    try {
      results = await executeMultiQuery(connectionId, query, selectedDatabase || null);
      onExecute?.(query);
    } catch (err) {
      results = [{
        queryId: '',
        columns: [],
        rows: [],
        totalRows: null,
        durationMs: 0,
        affectedRows: null,
        error: errorMessage(err),
      }];
    } finally {
      isRunning = false;
    }
  }

  async function runUnderCursor(): Promise<void> {
    if (!editorView || isRunning) return;
    const pos = editorView.state.selection.main.head;
    const stmt = statementAtCursor(sqlText, pos);
    if (!stmt.trim()) return;

    isRunning = true;
    executedStatements = [stmt];
    try {
      results = await executeMultiQuery(connectionId, stmt, selectedDatabase || null);
      onExecute?.(stmt);
    } catch (err) {
      results = [{
        queryId: '',
        columns: [],
        rows: [],
        totalRows: null,
        durationMs: 0,
        affectedRows: null,
        error: errorMessage(err),
      }];
    } finally {
      isRunning = false;
    }
  }

  function formatQuery(): void {
    if (!editorView) return;
    const dialect = sqlDialect();
    try {
      const formatted = sqlFormat(sqlText, { language: dialect as NonNullable<Parameters<typeof sqlFormat>[1]>['language'] });
      editorView.dispatch({
        changes: { from: 0, to: editorView.state.doc.length, insert: formatted },
      });
    } catch {
      // Formatting failed silently — leave content unchanged.
    }
  }

  async function runExplain(): Promise<void> {
    const query = sqlText.trim();
    if (!query || isRunning) return;
    isRunning = true;
    try {
      const explainResult = await explainQuery(connectionId, query, selectedDatabase || null);
      panelStore.openInFocused({
        kind: 'explain',
        connectionId,
        sql: explainResult.rawJson,
        dialect: explainResult.dialect,
      });
    } catch (err) {
      results = [{
        queryId: '',
        columns: [],
        rows: [],
        totalRows: null,
        durationMs: 0,
        affectedRows: null,
        error: errorMessage(err),
      }];
    } finally {
      isRunning = false;
    }
  }

  // ── Transaction controls ──────────────────────────────────────────────────

  async function beginTransaction(): Promise<void> {
    const profile = connections.getById(connectionId);
    const sql = (profile?.dbType === 'mysql' || profile?.dbType === 'mariadb')
      ? 'START TRANSACTION'
      : 'BEGIN';
    isRunning = true;
    try {
      await executeMultiQuery(connectionId, sql);
      transactionActive = true;
    } catch (err) {
      results = [{
        queryId: '',
        columns: [],
        rows: [],
        totalRows: null,
        durationMs: 0,
        affectedRows: null,
        error: errorMessage(err),
      }];
    } finally {
      isRunning = false;
    }
  }

  async function commitTransaction(): Promise<void> {
    isRunning = true;
    try {
      await executeMultiQuery(connectionId, 'COMMIT');
      transactionActive = false;
    } catch (err) {
      results = [{
        queryId: '',
        columns: [],
        rows: [],
        totalRows: null,
        durationMs: 0,
        affectedRows: null,
        error: errorMessage(err),
      }];
    } finally {
      isRunning = false;
    }
  }

  async function rollbackTransaction(): Promise<void> {
    isRunning = true;
    try {
      await executeMultiQuery(connectionId, 'ROLLBACK');
      transactionActive = false;
    } catch (err) {
      results = [{
        queryId: '',
        columns: [],
        rows: [],
        totalRows: null,
        durationMs: 0,
        affectedRows: null,
        error: errorMessage(err),
      }];
    } finally {
      isRunning = false;
    }
  }

  // ── Shortcut action listener ──────────────────────────────────────────────

  function handleShortcutAction(e: Event) {
    const { action } = (e as CustomEvent<{ action: string }>).detail;
    switch (action) {
      case 'QUERY_RUN_ALL':
        runQuery();
        break;
      case 'QUERY_RUN_SELECTION':
        runSelection();
        break;
      case 'QUERY_RUN_UNDER_CURSOR':
        runUnderCursor();
        break;
      case 'QUERY_FORMAT':
        formatQuery();
        break;
      case 'QUERY_EXPLAIN':
        runExplain();
        break;
    }
  }

  // Cmd+Enter runs the query from within the editor.
  function runQueryCommand(): boolean {
    runQuery();
    return true;
  }

  let unlistenShortcut: (() => void) | null = null;

  onMount(() => {
    if (!editorContainer) return;

    document.addEventListener('shortcut-action', handleShortcutAction);
    unlistenShortcut = () => document.removeEventListener('shortcut-action', handleShortcutAction);

    const runKeymap = keymap.of([
      { key: 'Mod-Enter', run: runQueryCommand },
      { key: 'Mod-Shift-Enter', run: () => { runSelection(); return true; } },
      { key: 'Mod-Shift-r', run: () => { runUnderCursor(); return true; } },
      { key: 'Mod-Shift-f', run: () => { formatQuery(); return true; } },
      { key: 'Alt-e', run: () => { runExplain(); return true; } },
    ]);

    const state = EditorState.create({
      doc: sqlText,
      extensions: [
        buildTheme(),
        lineNumbers(),
        highlightActiveLine(),
        highlightActiveLineGutter(),
        foldGutter(),
        drawSelection(),
        dropCursor(),
        rectangularSelection(),
        crosshairCursor(),
        highlightSelectionMatches(),
        syntaxHighlighting(defaultHighlightStyle),
        bracketMatching(),
        closeBrackets(),
        autocompletion({ override: [makeSchemaCompletionSource()] }),
        sqlLang(),
        runKeymap,
        keymap.of([...defaultKeymap, ...completionKeymap, ...closeBracketsKeymap, indentWithTab]),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            sqlText = update.state.doc.toString();
          }
        }),
      ],
    });

    editorView = new EditorView({ state, parent: editorContainer });
    editorView.focus();

    return () => {
      editorView?.destroy();
      editorView = undefined;
    };
  });

  onDestroy(() => {
    unlistenShortcut?.();
  });
</script>

<div class="query-editor-panel">
  <div class="toolbar" bind:this={toolbarEl}>
    <button
      class="run-button"
      onclick={runQuery}
      disabled={isRunning}
      title="Run query (Cmd+Enter)"
      aria-label="Run query"
    >
      {isRunning ? '⏳' : '▶'}
    </button>

    <span class="connection-badge" title={connectionName}>
      {connectionName}
    </span>

    {#if databases.length > 0}
      <Select
        bind:value={selectedDatabase}
        options={databases.map(db => ({ value: db, label: db }))}
        size="xs"
        aria-label="Select database"
        mono
      />
    {:else if selectedDatabase}
      <span class="connection-badge" title="Database">{selectedDatabase}</span>
    {/if}

    {#if transactionActive}
      <span class="tx-badge" title="Transaction in progress">TXN</span>
    {/if}

    <div class="toolbar-spacer"></div>

    {#if compact}
      <button
        bind:this={actionsMenuTriggerEl}
        class="toolbar-btn toolbar-btn--icon"
        onclick={() => { actionsMenuOpen = !actionsMenuOpen; }}
        title="More actions"
        aria-label="More actions"
        aria-expanded={actionsMenuOpen}
        aria-haspopup="menu"
      >
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
          <circle cx="2" cy="7" r="1.25" fill="currentColor"/>
          <circle cx="7" cy="7" r="1.25" fill="currentColor"/>
          <circle cx="12" cy="7" r="1.25" fill="currentColor"/>
        </svg>
      </button>

      {#if actionsMenuOpen}
        <div
          bind:this={actionsMenuEl}
          class="actions-menu"
          role="menu"
          style="top:{actionsMenuTop}px;left:{actionsMenuLeft}px"
          use:portal
        >
          <button class="actions-menu-item" role="menuitem" onclick={() => { runSelection(); actionsMenuOpen = false; }} disabled={isRunning}>
            <span>Run Selection</span>
            <kbd>⇧↵</kbd>
          </button>
          <button class="actions-menu-item" role="menuitem" onclick={() => { runUnderCursor(); actionsMenuOpen = false; }} disabled={isRunning}>
            <span>Run Cursor</span>
            <kbd>⇧R</kbd>
          </button>
          <div class="actions-menu-sep" role="separator"></div>
          <button class="actions-menu-item" role="menuitem" onclick={() => { formatQuery(); actionsMenuOpen = false; }}>
            <span>Format SQL</span>
            <kbd>⇧F</kbd>
          </button>
          <button class="actions-menu-item" role="menuitem" onclick={() => { runExplain(); actionsMenuOpen = false; }} disabled={isRunning}>
            <span>Explain</span>
          </button>
        </div>
      {/if}
    {:else}
      <button
        class="toolbar-btn"
        onclick={runSelection}
        disabled={isRunning}
        title="Run selection (Cmd+Shift+Enter)"
      >
        Run Selection
      </button>

      <button
        class="toolbar-btn"
        onclick={runUnderCursor}
        disabled={isRunning}
        title="Run statement under cursor (Cmd+Shift+R)"
      >
        Run Cursor
      </button>

      <button
        class="toolbar-btn"
        onclick={formatQuery}
        title="Format SQL (Cmd+Shift+F)"
      >
        Format
      </button>

      <button
        class="toolbar-btn"
        onclick={runExplain}
        disabled={isRunning}
        title="Explain query"
      >
        Explain
      </button>
    {/if}
  </div>

  {#if showTransactionToolbar}
    <div class="tx-toolbar" class:tx-active={transactionActive}>
      <span class="tx-label">
        {transactionActive ? 'Transaction active' : 'Transaction mode'}
      </span>
      <div class="tx-actions">
        {#if !transactionActive}
          <button class="tx-btn" onclick={beginTransaction} disabled={isRunning}>Begin</button>
        {:else}
          <button class="tx-btn tx-btn--commit" onclick={commitTransaction} disabled={isRunning}>Commit</button>
          <button class="tx-btn tx-btn--rollback" onclick={rollbackTransaction} disabled={isRunning}>Rollback</button>
        {/if}
      </div>
    </div>
  {/if}

  <div class="editor-wrapper">
    <div class="editor-container" bind:this={editorContainer}></div>
  </div>

  <div class="results-wrapper">
    <ResultsPanel {results} statements={executedStatements} />
  </div>
</div>

<style>
  .query-editor-panel {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: var(--color-editor-bg);
  }

  .toolbar {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--spacing-2);
    padding: 0 var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
    min-height: var(--toolbar-height);
  }

  .run-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--spacing-6);
    height: var(--spacing-6);
    padding: 0;
    background: var(--color-accent);
    color: var(--color-text-on-accent);
    border: none;
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    cursor: pointer;
    transition: background var(--transition-fast);
    flex-shrink: 0;
  }

  .run-button:hover:not(:disabled) {
    background: var(--color-accent-hover);
  }

  .run-button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .connection-badge {
    display: inline-flex;
    align-items: center;
    padding: 0 var(--spacing-2);
    height: calc(var(--toolbar-height) - var(--spacing-3));
    color: var(--color-text-secondary);
    max-width: 300px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: default;
  }

  .tx-badge {
    display: inline-flex;
    align-items: center;
    padding: 0 var(--spacing-2);
    height: calc(var(--toolbar-height) - var(--spacing-3));
    background: var(--color-warning-subtle, #fff3cd);
    border: 1px solid var(--color-warning, #f59e0b);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-warning, #f59e0b);
    letter-spacing: 0.05em;
  }

  .toolbar-spacer {
    flex: 1;
  }

  .toolbar-btn {
    padding: 0 var(--spacing-2);
    height: calc(var(--toolbar-height) - var(--spacing-3));
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
    cursor: pointer;
    transition:
      color var(--transition-fast),
      background var(--transition-fast);
    white-space: nowrap;
    flex-shrink: 0;
  }

  .toolbar-btn:hover:not(:disabled) {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .toolbar-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .toolbar-btn--icon {
    width: calc(var(--toolbar-height) - var(--spacing-3));
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  /* Actions overflow menu */
  .actions-menu {
    position: fixed;
    z-index: 9999;
    background: var(--color-bg-overlay);
    -webkit-backdrop-filter: blur(20px) saturate(160%);
    backdrop-filter: blur(20px) saturate(160%);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-overlay);
    padding: 3px;
    min-width: 160px;
    animation: dropdown-in var(--transition-md) both;
    transform-origin: top right;
  }

  @keyframes dropdown-in {
    from { opacity: 0; transform: scaleY(0.92) translateY(-4px); }
    to   { opacity: 1; transform: scaleY(1)    translateY(0);    }
  }

  .actions-menu-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--spacing-4);
    width: 100%;
    padding: 0 var(--spacing-2);
    min-height: 26px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text-primary);
    font-family: var(--font-family-ui);
    font-size: var(--font-size-xs);
    text-align: left;
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .actions-menu-item:hover:not(:disabled) {
    background: var(--color-accent-subtle);
  }

  .actions-menu-item:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .actions-menu-item kbd {
    font-size: 10px;
    color: var(--color-text-muted);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    padding: 1px 5px;
    font-family: var(--font-family-ui);
  }

  .actions-menu-sep {
    height: 1px;
    background: var(--color-border);
    margin: 3px 0;
  }

  /* Transaction toolbar */
  .tx-toolbar {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--spacing-3);
    padding: 0 var(--spacing-3);
    height: 32px;
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
  }

  .tx-toolbar.tx-active {
    background: var(--color-warning-subtle, #fff3cd);
    border-bottom-color: var(--color-warning, #f59e0b);
  }

  .tx-label {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    flex: 1;
  }

  .tx-active .tx-label {
    color: var(--color-warning, #f59e0b);
    font-weight: var(--font-weight-medium);
  }

  .tx-actions {
    display: flex;
    gap: var(--spacing-1);
  }

  .tx-btn {
    padding: 0 var(--spacing-2);
    height: 22px;
    font-size: var(--font-size-xs);
    border-radius: var(--radius-sm);
    border: 1px solid var(--color-border);
    background: var(--color-bg-primary);
    color: var(--color-text-secondary);
    cursor: pointer;
    transition: background var(--transition-fast);
    white-space: nowrap;
  }

  .tx-btn:hover:not(:disabled) {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .tx-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .tx-btn--commit {
    border-color: var(--color-success);
    color: var(--color-success);
  }

  .tx-btn--commit:hover:not(:disabled) {
    background: var(--color-success-subtle);
  }

  .tx-btn--rollback {
    border-color: var(--color-danger);
    color: var(--color-danger);
  }

  .tx-btn--rollback:hover:not(:disabled) {
    background: var(--color-danger-subtle);
  }

  .editor-wrapper {
    flex: 0 0 40%;
    overflow: hidden;
    border-bottom: 1px solid var(--color-border);
  }

  .editor-container {
    height: 100%;
    overflow: hidden;
  }

  .editor-container :global(.cm-editor) {
    height: 100%;
  }

  .editor-container :global(.cm-scroller) {
    overflow: auto;
  }

  .results-wrapper {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
</style>
