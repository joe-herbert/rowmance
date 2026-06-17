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
  import { executeQuery, executeSelection, explainQuery } from '$lib/tauri/query';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import ResultsPanel from '$lib/components/editor/ResultsPanel.svelte';
  import * as schemaApi from '$lib/tauri/schema';
  import { statementAtCursor } from '$lib/utils/sql';
  import { errorMessage } from '$lib/utils/errors';

  interface Props {
    connectionId: string;
    initialSql?: string;
    onExecute?: (_sql: string) => void;
  }

  let { connectionId, initialSql = '', onExecute }: Props = $props();

  const connections = useConnections();
  const settingsStore = useSettings();
  const panelStore = usePanels();

  let editorContainer = $state<HTMLDivElement | undefined>(undefined);
  let editorView = $state<EditorView | undefined>(undefined);
  let sqlText = $state(untrack(() => initialSql));
  let result = $state<QueryResult | null>(null);
  let isRunning = $state(false);
  let transactionActive = $state(false);

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
      const databases = await schemaApi.listDatabases(connId);
      const tables: SchemaTable[] = [];
      for (const db of databases) {
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
    try {
      result = await executeQuery(connectionId, query, 1, 50);
      onExecute?.(query);
    } catch (err) {
      result = {
        queryId: '',
        columns: [],
        rows: [],
        totalRows: null,
        durationMs: 0,
        affectedRows: null,
        error: errorMessage(err),
      };
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
    try {
      result = await executeSelection(connectionId, query);
      onExecute?.(query);
    } catch (err) {
      result = {
        queryId: '',
        columns: [],
        rows: [],
        totalRows: null,
        durationMs: 0,
        affectedRows: null,
        error: errorMessage(err),
      };
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
    try {
      result = await executeSelection(connectionId, stmt);
      onExecute?.(stmt);
    } catch (err) {
      result = {
        queryId: '',
        columns: [],
        rows: [],
        totalRows: null,
        durationMs: 0,
        affectedRows: null,
        error: errorMessage(err),
      };
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
      const explainResult = await explainQuery(connectionId, query);
      panelStore.openInFocused({
        kind: 'explain',
        connectionId,
        sql: explainResult.rawJson,
        dialect: explainResult.dialect,
      });
    } catch (err) {
      result = {
        queryId: '',
        columns: [],
        rows: [],
        totalRows: null,
        durationMs: 0,
        affectedRows: null,
        error: errorMessage(err),
      };
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
      await executeSelection(connectionId, sql);
      transactionActive = true;
    } catch (err) {
      result = {
        queryId: '',
        columns: [],
        rows: [],
        totalRows: null,
        durationMs: 0,
        affectedRows: null,
        error: errorMessage(err),
      };
    } finally {
      isRunning = false;
    }
  }

  async function commitTransaction(): Promise<void> {
    isRunning = true;
    try {
      await executeSelection(connectionId, 'COMMIT');
      transactionActive = false;
    } catch (err) {
      result = {
        queryId: '',
        columns: [],
        rows: [],
        totalRows: null,
        durationMs: 0,
        affectedRows: null,
        error: errorMessage(err),
      };
    } finally {
      isRunning = false;
    }
  }

  async function rollbackTransaction(): Promise<void> {
    isRunning = true;
    try {
      await executeSelection(connectionId, 'ROLLBACK');
      transactionActive = false;
    } catch (err) {
      result = {
        queryId: '',
        columns: [],
        rows: [],
        totalRows: null,
        durationMs: 0,
        affectedRows: null,
        error: errorMessage(err),
      };
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
        keymap.of([...defaultKeymap, ...completionKeymap, ...closeBracketsKeymap, indentWithTab]),
        runKeymap,
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
  <div class="toolbar">
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

    {#if transactionActive}
      <span class="tx-badge" title="Transaction in progress">TXN</span>
    {/if}

    <div class="toolbar-spacer"></div>

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
    <ResultsPanel {result} />
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
    flex: 0 0 var(--toolbar-height);
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: 0 var(--spacing-3);
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
    min-height: var(--toolbar-height);
    max-height: var(--toolbar-height);
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
    background: var(--color-bg-tertiary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-secondary);
    max-width: 160px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
