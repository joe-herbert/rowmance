<!--
  DdlViewer — read-only CodeMirror panel displaying a CREATE TABLE/VIEW DDL statement.
  Fetches the DDL from the backend on mount and shows loading/error states.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { EditorView } from '@codemirror/view';
  import { EditorState, type Extension } from '@codemirror/state';
  import { sql as sqlLang } from '@codemirror/lang-sql';
  import { defaultHighlightStyle, syntaxHighlighting } from '@codemirror/language';
  import { getDdl } from '$lib/tauri/schema';
  import { errorMessage } from '$lib/utils/errors';
  import Loader from '$lib/components/ui/Loader.svelte';

  interface Props {
    connectionId: string;
    database: string;
    objectName: string;
    objectType: 'table' | 'view';
  }

  const { connectionId, database, objectName, objectType }: Props = $props();

  let editorContainer = $state<HTMLDivElement | undefined>(undefined);
  let editorView = $state<EditorView | undefined>(undefined);
  let ddlText = $state('');
  let isLoading = $state(true);
  let loadError = $state<string | null>(null);

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

  async function loadDdl() {
    isLoading = true;
    loadError = null;
    try {
      ddlText = await getDdl(connectionId, database, objectName, objectType);
    } catch (err) {
      loadError = errorMessage(err);
    } finally {
      isLoading = false;
    }
  }

  async function copyDdl() {
    if (ddlText) {
      await navigator.clipboard.writeText(ddlText);
    }
  }

  onMount(() => {
    loadDdl().then(() => {
      if (!editorContainer) return;

      const state = EditorState.create({
        doc: ddlText,
        extensions: [
          buildTheme(),
          syntaxHighlighting(defaultHighlightStyle),
          sqlLang(),
          EditorState.readOnly.of(true),
        ],
      });

      editorView = new EditorView({ state, parent: editorContainer });
    });

    return () => {
      editorView?.destroy();
      editorView = undefined;
    };
  });

  // When ddlText changes after load, update the editor content.
  $effect(() => {
    if (!editorView) return;
    const current = editorView.state.doc.toString();
    if (current !== ddlText) {
      editorView.dispatch({
        changes: { from: 0, to: current.length, insert: ddlText },
      });
    }
  });
</script>

<div class="ddl-viewer">
  <div class="toolbar">
    <span class="object-label">
      <span class="object-type">{objectType}</span>
      <span class="object-path">{database}.{objectName}</span>
    </span>
    <div class="toolbar-spacer"></div>
    <button
      class="copy-btn"
      onclick={copyDdl}
      disabled={isLoading || !!loadError || !ddlText}
      title="Copy DDL to clipboard"
    >
      Copy DDL
    </button>
  </div>

  <div class="content">
    {#if isLoading}
      <div class="state-overlay">
        <Loader />
      </div>
    {:else if loadError}
      <div class="state-overlay state-overlay--error">
        <span class="error-icon" aria-hidden="true"><svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg></span>
        <span class="error-text">{loadError}</span>
        <button class="retry-btn" onclick={loadDdl}>Retry</button>
      </div>
    {:else}
      <div class="editor-container" bind:this={editorContainer}>
        <!-- CodeMirror mounts here -->
      </div>
    {/if}
  </div>
</div>

<style>
  .ddl-viewer {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    overflow: hidden;
  }

  .toolbar {
    flex: 0 0 var(--toolbar-height);
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: 0 var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
    min-height: var(--toolbar-height);
    max-height: var(--toolbar-height);
    flex-shrink: 0;
  }

  .object-label {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    overflow: hidden;
  }

  .object-type {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    flex-shrink: 0;
  }

  .object-path {
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
    font-family: var(--font-family-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .toolbar-spacer {
    flex: 1;
  }

  .copy-btn {
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
    flex-shrink: 0;
  }

  .copy-btn:hover:not(:disabled) {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .copy-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .content {
    flex: 1;
    overflow: hidden;
    position: relative;
    display: flex;
    flex-direction: column;
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

  .state-overlay {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--spacing-2);
    padding: var(--spacing-4);
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .state-overlay--error {
    color: var(--color-danger);
  }

  .error-icon {
    font-size: var(--font-size-lg);
  }

  .error-text {
    text-align: center;
    max-width: 300px;
    line-height: var(--line-height-normal);
    word-break: break-word;
    -webkit-user-select: text;
    user-select: text;
  }

  .retry-btn {
    margin-top: var(--spacing-1);
    padding: var(--spacing-1) var(--spacing-3);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
    background: var(--color-bg-primary);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .retry-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .loading-text {
    font-style: italic;
    animation: pulse 1s infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }
</style>
