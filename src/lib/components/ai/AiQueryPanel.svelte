<!--
  AiQueryPanel — inline AI panel for query generation and explanation.
  Mode 'generate': user types a prompt, AI returns SQL inserted into editor.
  Mode 'explain': shows AI explanation of the current SQL.
-->
<script lang="ts">
  import { useSettings } from '$lib/stores/settings.svelte';
  import { generateQuery, explainQuery, type AiConfig } from '$lib/ai/service';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { defaultDialectInfo } from '$lib/utils/dialect';
  import { errorMessage } from '$lib/utils/errors';
  import SqlHighlight from '$lib/components/ui/SqlHighlight.svelte';
  import CloseIcon from '$lib/components/icons/CloseIcon.svelte';
  import Spinner from '$lib/components/ui/Spinner.svelte';

  interface Props {
    mode: 'generate' | 'explain';
    connectionId: string;
    database: string;
    currentSql?: string;
    ongenerated?: (_sql: string) => void;
    onclose?: () => void;
  }

  let { mode, connectionId, database, currentSql = '', ongenerated, onclose }: Props = $props();

  const settingsStore = useSettings();
  const connections = useConnections();

  const config = $derived<AiConfig>({
    provider: settingsStore.settings.aiProvider,
    model: settingsStore.settings.aiModel,
    apiKey: settingsStore.settings.aiApiKey,
    baseUrl: settingsStore.settings.aiBaseUrl,
    contextLevel: settingsStore.settings.aiContextLevel,
    dataSampleRows: settingsStore.settings.aiDataSampleRows,
  });

  const dialectInfo = $derived(connections.getById(connectionId)?.dialectInfo);

  let prompt = $state('');
  let result = $state('');
  let isLoading = $state(false);
  let error = $state<string | null>(null);
  let promptEl = $state<HTMLTextAreaElement | undefined>(undefined);
  let didExplain = $state(false);

  $effect(() => {
    if (mode === 'explain' && !didExplain && currentSql.trim()) {
      didExplain = true;
      runExplain();
    }
  });

  $effect(() => {
    if (mode === 'generate') {
      promptEl?.focus();
    }
  });

  async function runGenerate() {
    if (!prompt.trim() || isLoading) return;
    isLoading = true;
    error = null;
    result = '';
    try {
      const sql = await generateQuery(
        config,
        prompt,
        connectionId,
        database,
        dialectInfo ?? defaultDialectInfo,
      );
      result = sql;
    } catch (err) {
      error = errorMessage(err);
    } finally {
      isLoading = false;
    }
  }

  async function runExplain() {
    if (!currentSql.trim() || isLoading) return;
    isLoading = true;
    error = null;
    result = '';
    try {
      result = await explainQuery(
        config,
        currentSql,
        connectionId,
        database,
        dialectInfo ?? defaultDialectInfo,
      );
    } catch (err) {
      error = errorMessage(err);
    } finally {
      isLoading = false;
    }
  }

  function insertSql() {
    if (result) ongenerated?.(result);
  }
</script>

<div class="ai-panel">
  <div class="ai-panel-header">
    <span class="ai-panel-title">
      {#if mode === 'generate'}
        Generate Query
      {:else}
        Explain Query
      {/if}
    </span>
    <button class="ai-close-btn" onclick={onclose} aria-label="Close AI panel">
      <CloseIcon width={11} height={11} strokeWidth={2.5} />
    </button>
  </div>

  {#if settingsStore.settings.aiContextLevel === 'none'}
    <div class="ai-context-warning">
      AI cannot see your database schema — queries may be inaccurate. Enable schema access in
      <strong>Settings &rsaquo; AI &rsaquo; Context Level</strong>.
    </div>
  {/if}

  {#if mode === 'generate'}
    <div class="ai-input-row">
      <textarea
        bind:this={promptEl}
        class="ai-prompt"
        bind:value={prompt}
        placeholder="Describe the query you want to write…"
        rows="2"
        onkeydown={(e) => {
          if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
            e.preventDefault();
            runGenerate();
          }
          if (e.key === 'Escape') onclose?.();
        }}
      ></textarea>
      <button class="ai-run-btn" onclick={runGenerate} disabled={isLoading || !prompt.trim()}>
        {#if isLoading}
          <Spinner size={12} label="Generating" />
        {:else}
          Generate
        {/if}
      </button>
    </div>
  {:else if mode === 'explain' && isLoading}
    <div class="ai-loading"><Spinner size={12} label="Explaining" /> Explaining…</div>
  {/if}

  {#if error}
    <div class="ai-error">{error}</div>
  {/if}

  {#if result}
    <div class="ai-result">
      {#if mode === 'generate'}
        <pre class="ai-result-text"><SqlHighlight sql={result} /></pre>
      {:else}
        <p class="ai-result-text ai-result-text--plain">{result}</p>
      {/if}
      {#if mode === 'generate'}
        <div class="ai-result-actions">
          <button class="ai-action-btn ai-action-btn--primary" onclick={insertSql}
            >Insert into editor</button
          >
          <button class="ai-action-btn" onclick={runGenerate}>Regenerate</button>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .ai-panel {
    border-top: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    display: flex;
    flex-direction: column;
    gap: 0;
    flex-shrink: 0;
  }

  .ai-panel-header {
    display: flex;
    align-items: center;
    padding: var(--spacing-2) var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
    gap: var(--spacing-2);
  }

  .ai-panel-title {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    flex: 1;
  }

  .ai-close-btn {
    color: var(--color-text-muted);
    padding: 2px;
    border-radius: var(--radius-xs);
    display: flex;
    align-items: center;
    transition:
      color var(--transition-fast),
      background var(--transition-fast);
  }

  .ai-close-btn:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .ai-input-row {
    display: flex;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-3);
    align-items: flex-start;
  }

  .ai-prompt {
    flex: 1;
    resize: none;
    padding: var(--spacing-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-primary);
    color: var(--color-text-primary);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    outline: none;
    line-height: 1.4;
    transition: border-color var(--transition-fast);
  }

  .ai-prompt:focus {
    border-color: var(--color-accent);
  }

  .ai-run-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--spacing-2);
    height: 28px;
    padding: 0 var(--spacing-3);
    background: var(--color-accent);
    color: var(--color-text-on-accent);
    border: none;
    border-radius: var(--radius-sm);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    cursor: pointer;
    flex-shrink: 0;
    align-self: flex-end;
    transition: background var(--transition-fast);
  }

  .ai-run-btn:hover:not(:disabled) {
    background: var(--color-accent-hover);
  }

  .ai-run-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .ai-loading {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-3);
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    font-style: italic;
  }

  .ai-context-warning {
    margin: var(--spacing-2) var(--spacing-3) 0;
    padding: var(--spacing-2) var(--spacing-3);
    background: var(--color-warning-subtle, rgba(234, 179, 8, 0.1));
    border: 1px solid var(--color-warning, rgba(234, 179, 8, 0.4));
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
    line-height: 1.4;
  }

  .ai-context-warning strong {
    color: var(--color-text-primary);
    font-weight: var(--font-weight-medium);
  }

  .ai-error {
    margin: var(--spacing-2) var(--spacing-3);
    padding: var(--spacing-2) var(--spacing-3);
    background: var(--color-danger-subtle);
    border: 1px solid rgba(239, 68, 68, 0.3);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-sm);
    color: var(--color-danger);
  }

  .ai-result {
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--color-border);
    max-height: 200px;
    overflow-y: auto;
  }

  .ai-result-text {
    padding: var(--spacing-3);
    margin: 0;
    font-size: var(--font-size-sm);
    font-family: var(--font-family-mono);
    white-space: pre-wrap;
    word-break: break-word;
    color: var(--color-text-primary);
    line-height: 1.5;
    -webkit-user-select: text;
    user-select: text;
  }

  .ai-result-text--plain {
    font-family: var(--font-family-ui);
  }

  .ai-result-actions {
    display: flex;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-3);
    border-top: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    flex-shrink: 0;
  }

  .ai-action-btn {
    height: 26px;
    padding: 0 var(--spacing-3);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    color: var(--color-text-secondary);
    background: var(--color-bg-secondary);
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .ai-action-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .ai-action-btn--primary {
    background: var(--color-accent);
    color: var(--color-text-on-accent);
    border-color: var(--color-accent);
  }

  .ai-action-btn--primary:hover {
    background: var(--color-accent-hover);
    border-color: var(--color-accent-hover);
  }
</style>
