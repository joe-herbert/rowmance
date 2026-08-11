<!--
  SchemaSyncPreview — editable CodeMirror preview of the generated sync SQL
  statements, with Apply (sequential execution via executeDdl, mirroring
  TableStructure.svelte's execSqls loop) and Copy to Clipboard actions.
  Rendered as a modal overlay on top of SchemaComparePanel.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { EditorView } from '@codemirror/view';
  import { EditorState, type Extension } from '@codemirror/state';
  import { sql as sqlLang } from '@codemirror/lang-sql';
  import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
  import { tags } from '@lezer/highlight';
  import Modal from '$lib/components/Modal.svelte';
  import CloseIcon from '$lib/components/icons/CloseIcon.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { executeDdl } from '$lib/tauri/schema';
  import { errorMessage } from '$lib/utils/errors';

  interface Props {
    sql: string[];
    targetConnectionId: string;
    /** Full human-readable description of the source (connection / database / table) this
     *  SQL was generated to copy from — shown alongside targetLabel so the copy direction
     *  is unambiguous, not just which side gets modified. */
    sourceLabel: string;
    /** Full human-readable description of the target (connection / database / table),
     *  shown prominently so it's unambiguous which side this SQL will run against. */
    targetLabel: string;
    onclose: () => void;
  }

  const { sql, targetConnectionId, sourceLabel, targetLabel, onclose }: Props = $props();

  const connections = useConnections();
  const targetProfile = $derived(connections.getById(targetConnectionId));
  const isReadOnly = $derived(targetProfile?.readOnly ?? false);

  type StatementStatus = 'pending' | 'running' | 'success' | 'error';
  interface StatementResult {
    sql: string;
    status: StatementStatus;
    error?: string;
  }

  let editorContainer = $state<HTMLDivElement | undefined>(undefined);
  let editorView = $state<EditorView | undefined>(undefined);
  let applying = $state(false);
  let results = $state<StatementResult[] | null>(null);
  let copied = $state(false);

  const initialDoc = $derived(sql.join(';\n\n') + (sql.length > 0 ? ';\n' : ''));

  function resolveCSSVar(name: string): string {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  }

  function buildHighlightStyle(): HighlightStyle {
    return HighlightStyle.define([
      { tag: tags.keyword, color: resolveCSSVar('--color-editor-keyword') },
      { tag: tags.string, color: resolveCSSVar('--color-editor-string') },
      { tag: tags.number, color: resolveCSSVar('--color-editor-number') },
      { tag: tags.comment, color: resolveCSSVar('--color-editor-comment') },
      { tag: tags.operator, color: resolveCSSVar('--color-editor-operator') },
      {
        tag: [tags.function(tags.variableName), tags.function(tags.propertyName)],
        color: resolveCSSVar('--color-editor-function'),
      },
      { tag: [tags.typeName, tags.className], color: resolveCSSVar('--color-editor-type') },
    ]);
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
      '.cm-gutters': {
        backgroundColor: resolveCSSVar('--color-editor-gutter-bg'),
        color: resolveCSSVar('--color-editor-gutter-text'),
        border: 'none',
        borderRight: `1px solid var(--color-border)`,
      },
      '.cm-scroller': {
        overflow: 'auto',
        fontFamily: 'var(--font-family-mono)',
      },
    });
  }

  onMount(() => {
    if (!editorContainer) return;
    const state = EditorState.create({
      doc: initialDoc,
      extensions: [buildTheme(), syntaxHighlighting(buildHighlightStyle()), sqlLang()],
    });
    editorView = new EditorView({ state, parent: editorContainer });

    return () => {
      editorView?.destroy();
      editorView = undefined;
    };
  });

  /** Split the editor's current text into individual statements on `;` followed by
   *  whitespace/newline (or end of text), trimming empties — a simple splitter since
   *  there is no shared multi-statement splitting utility elsewhere in this codebase. */
  function splitStatements(text: string): string[] {
    return text
      .split(/;\s*\n|;\s*$/)
      .map((s) => s.trim())
      .filter((s) => s.length > 0);
  }

  function currentText(): string {
    return editorView ? editorView.state.doc.toString() : initialDoc;
  }

  async function copyToClipboard() {
    await navigator.clipboard.writeText(currentText());
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  async function apply() {
    if (isReadOnly || applying) return;
    const statements = splitStatements(currentText());
    if (statements.length === 0) return;

    results = statements.map((s) => ({ sql: s, status: 'pending' as const }));
    applying = true;

    for (let i = 0; i < statements.length; i++) {
      results = results.map((r, idx) => (idx === i ? { ...r, status: 'running' } : r));
      try {
        await executeDdl(targetConnectionId, statements[i]);
        results = results.map((r, idx) => (idx === i ? { ...r, status: 'success' } : r));
      } catch (err) {
        results = results.map((r, idx) =>
          idx === i ? { ...r, status: 'error', error: errorMessage(err) } : r,
        );
        break;
      }
    }

    applying = false;
  }

  function handleKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape') {
      e.preventDefault();
      onclose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<Modal label="Sync SQL preview" onbackdropclick={onclose}>
  <div class="sync-preview">
    <div class="sync-header">
      <span class="sync-title">Sync SQL</span>
      <div class="sync-header-spacer"></div>
      <button class="icon-btn" onclick={onclose} title="Close" aria-label="Close">
        <CloseIcon width={16} height={16} strokeWidth={2} />
      </button>
    </div>

    <div class="target-banner">
      This will copy <strong class="mono">{sourceLabel}</strong> over
      <strong class="mono">{targetLabel}</strong> — {targetLabel} will be modified.
    </div>

    {#if isReadOnly}
      <div class="readonly-banner">
        This connection is read-only. Applying SQL is disabled — copy the statements and run
        them elsewhere if needed.
      </div>
    {/if}

    <div class="editor-container" bind:this={editorContainer}></div>

    {#if results}
      <div class="results-list">
        {#each results as r, i (i)}
          <div class="result-row result-{r.status}">
            <span class="result-status">
              {#if r.status === 'pending'}○{:else if r.status === 'running'}…{:else if r.status === 'success'}✓{:else}✕{/if}
            </span>
            <span class="result-sql mono">{r.sql}</span>
            {#if r.error}<span class="result-error">{r.error}</span>{/if}
          </div>
        {/each}
      </div>
    {/if}

    <div class="sync-footer">
      <button class="btn" onclick={copyToClipboard}>{copied ? 'Copied!' : 'Copy to Clipboard'}</button>
      <div class="footer-spacer"></div>
      <button class="btn" onclick={onclose}>Close</button>
      <button
        class="btn btn--primary"
        onclick={apply}
        disabled={isReadOnly || applying || sql.length === 0}
        title={isReadOnly ? 'This connection is read-only' : undefined}
      >
        {applying ? 'Applying…' : 'Apply'}
      </button>
    </div>
  </div>
</Modal>

<style>
  .sync-preview {
    display: flex;
    flex-direction: column;
    width: 720px;
    max-width: 92vw;
    height: 70vh;
    max-height: 640px;
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    overflow: hidden;
  }

  .sync-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .sync-title {
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
  }

  .target-banner {
    padding: var(--spacing-2) var(--spacing-3);
    background: var(--color-danger-subtle);
    color: var(--color-danger);
    font-size: var(--font-size-xs);
    line-height: var(--line-height-normal);
    flex-shrink: 0;
  }

  .target-banner strong {
    font-weight: var(--font-weight-semibold);
  }

  .mono {
    font-family: var(--font-family-mono);
  }

  .sync-header-spacer {
    flex: 1;
  }

  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    border: none;
    color: var(--color-text-muted);
    cursor: pointer;
    padding: 4px;
    border-radius: var(--radius-sm);
  }

  .icon-btn:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .readonly-banner {
    padding: var(--spacing-2) var(--spacing-3);
    background: var(--color-warning-subtle);
    color: var(--color-warning);
    font-size: var(--font-size-xs);
    line-height: var(--line-height-normal);
    flex-shrink: 0;
  }

  .editor-container {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .editor-container :global(.cm-editor) {
    height: 100%;
  }

  .editor-container :global(.cm-scroller) {
    overflow: auto;
  }

  .results-list {
    max-height: 160px;
    overflow: auto;
    border-top: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .result-row {
    display: flex;
    align-items: baseline;
    gap: var(--spacing-2);
    padding: 3px var(--spacing-3);
    font-size: var(--font-size-xs);
  }

  .result-status {
    flex-shrink: 0;
    width: 14px;
    text-align: center;
    color: var(--color-text-muted);
  }

  .result-success .result-status {
    color: var(--color-success);
  }

  .result-error .result-status {
    color: var(--color-danger);
  }

  .result-running .result-status {
    color: var(--color-accent);
  }

  .result-sql {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--color-text-secondary);
    flex: 1;
    min-width: 0;
  }

  .result-error {
    color: var(--color-danger);
    flex-shrink: 0;
  }

  .sync-footer {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-3);
    border-top: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .footer-spacer {
    flex: 1;
  }

  .btn {
    height: 28px;
    padding: 0 14px;
    border-radius: var(--radius-md);
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    cursor: pointer;
    border: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    color: var(--color-text-secondary);
    transition: all var(--transition-fast);
    white-space: nowrap;
    font-family: var(--font-family-ui);
  }

  .btn:hover:not(:disabled) {
    border-color: var(--color-border-strong);
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn--primary {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: white;
  }

  .btn--primary:hover:not(:disabled) {
    opacity: 0.88;
    border-color: var(--color-accent);
    background: var(--color-accent);
    color: white;
  }
</style>
