<!--
  AiModal — modal for AI-powered query generation and explanation.
  Generate mode: user types a prompt, AI returns SQL to insert at a chosen line.
  Explain mode: automatically explains the provided SQL on open.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import Modal from '$lib/components/Modal.svelte';
  import SqlHighlight from '$lib/components/ui/SqlHighlight.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import {
    generateQuery,
    explainQuery,
    describeTable,
    summariseResult,
    buildFollowUpSystemPrompt as buildFollowUpSystemPromptForMode,
    type AiConfig,
  } from '$lib/ai/service';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { useAiChat } from '$lib/stores/aiChat.svelte';
  import type { AiConversation, AiMessage } from '$lib/types';
  import { defaultDialectInfo } from '$lib/utils/dialect';
  import { errorMessage } from '$lib/utils/errors';
  import { fnv1aHash } from '$lib/utils/hash';
  import { looksLikeSql, stripSqlCodeFence } from '$lib/utils/sql';
  import { marked } from 'marked';
  import CloseIcon from '$lib/components/icons/CloseIcon.svelte';
  import Spinner from '$lib/components/ui/Spinner.svelte';
  import CopyButton from '$lib/components/ui/CopyButton.svelte';

  interface GenerateProps {
    mode: 'generate';
    connectionId: string;
    database: string;
    oninsert: (_sql: string) => void;
    onclose: () => void;
  }
  interface ExplainProps {
    mode: 'explain';
    sql: string;
    connectionId: string;
    database: string;
    onclose: () => void;
  }
  interface DescribeProps {
    mode: 'describe';
    tableName: string;
    ddl: string;
    connectionId: string;
    database: string;
    onclose: () => void;
  }
  interface SummariseProps {
    mode: 'summarise';
    sql: string;
    columns: string[];
    rows: (string | number | boolean | null)[][];
    connectionId: string;
    database: string;
    onclose: () => void;
  }
  type Props = GenerateProps | ExplainProps | DescribeProps | SummariseProps;

  // eslint-disable-next-line svelte/valid-compile
  let props: Props = $props();

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

  const dialectInfo = $derived(connections.getById(props.connectionId)?.dialectInfo);

  const aiChat = useAiChat();

  let prompt = $state('');
  let result = $state('');
  let isLoading = $state(false);
  let error = $state<string | null>(null);
  let promptEl = $state<HTMLTextAreaElement | undefined>(undefined);
  let hasRun = $state(false);

  // Persisted conversation this modal is showing/continuing (null until the first
  // exchange completes and is saved, or an existing thread for this source is resumed).
  let conversation = $state<AiConversation | null>(null);
  let messages = $state<AiMessage[]>([]);
  let resumedExisting = $state(false);

  let followUpText = $state('');
  let followUpLoading = $state(false);
  let followUpError = $state<string | null>(null);
  let pendingQuestion = $state<string | null>(null);
  let bodyEl = $state<HTMLDivElement | undefined>(undefined);

  async function scrollToBottom() {
    await tick();
    bodyEl?.scrollTo({ top: bodyEl.scrollHeight });
  }

  // Follow-up exchanges beyond the first user/assistant pair (which `result` already shows).
  const followUpPairs = $derived.by(() => {
    const rest = messages.slice(2);
    const pairs: { question: string; answer: string }[] = [];
    for (let i = 0; i < rest.length; i += 2) {
      pairs.push({ question: rest[i]?.content ?? '', answer: rest[i + 1]?.content ?? '' });
    }
    return pairs;
  });

  const latestAssistantContent = $derived(
    followUpPairs.length > 0 ? followUpPairs[followUpPairs.length - 1].answer : result,
  );

  function computeContextKey(): string | null {
    if (props.mode === 'describe') {
      return `${props.connectionId}:${props.database}:${props.tableName}`;
    }
    if (props.mode === 'explain' || props.mode === 'summarise') {
      return `${props.connectionId}:${props.database}:${fnv1aHash(props.sql)}`;
    }
    return null;
  }

  function computeTitle(): string {
    if (props.mode === 'describe') return `Describe: ${props.tableName}`;
    if (props.mode === 'explain') return `Explain: ${props.sql.slice(0, 60)}`;
    if (props.mode === 'summarise') return `Summarise: ${props.sql.slice(0, 60)}`;
    return `Generate: ${prompt.slice(0, 60)}`;
  }

  async function buildFollowUpSystemPrompt(): Promise<string> {
    return buildFollowUpSystemPromptForMode(
      props.mode,
      config,
      props.connectionId,
      props.database,
      dialectInfo ?? defaultDialectInfo,
    );
  }

  async function persistFirstExchange(firstUserMessage: string) {
    if (props.mode === 'generate' && conversation) return; // regenerate: don't duplicate
    const { conversation: conv, messages: msgs } = await aiChat.create({
      mode: props.mode,
      contextKey: computeContextKey(),
      title: computeTitle(),
      connectionId: props.connectionId,
      database: props.database,
      firstUserMessage,
      firstAssistantMessage: result,
    });
    conversation = conv;
    messages = msgs;
  }

  async function initConversation(forceNew = false) {
    if (!forceNew) {
      const existing = await aiChat.find(
        props.mode,
        computeContextKey(),
        props.connectionId,
        props.database,
      );
      if (existing) {
        conversation = existing;
        messages = await aiChat.ensureMessagesLoaded(existing.id);
        result = messages[1]?.content ?? '';
        resumedExisting = true;
        return;
      }
    }
    resumedExisting = false;
    conversation = null;
    messages = [];
    if (props.mode === 'explain') await runExplain();
    else if (props.mode === 'describe') await runDescribe();
    else if (props.mode === 'summarise') await runSummarise();
  }

  async function sendFollowUp() {
    if (!followUpText.trim() || !conversation || followUpLoading) return;
    followUpLoading = true;
    followUpError = null;
    const text = followUpText.trim();
    followUpText = '';
    pendingQuestion = text;
    await scrollToBottom();
    try {
      const systemPrompt = await buildFollowUpSystemPrompt();
      await aiChat.sendFollowUp(conversation.id, config, systemPrompt, text);
      messages = aiChat.getMessages(conversation.id);
    } catch (err) {
      followUpError = errorMessage(err);
      followUpText = text;
    } finally {
      followUpLoading = false;
      pendingQuestion = null;
      await scrollToBottom();
    }
  }

  $effect(() => {
    if (props.mode !== 'generate' && !hasRun) {
      hasRun = true;
      initConversation();
    }
  });

  $effect(() => {
    if (props.mode === 'generate') {
      promptEl?.focus();
    }
  });

  async function runGenerate() {
    if (!prompt.trim() || isLoading) return;
    isLoading = true;
    error = null;
    result = '';
    try {
      result = await generateQuery(
        config,
        prompt,
        props.connectionId,
        props.database,
        dialectInfo ?? defaultDialectInfo,
      );
      await persistFirstExchange(prompt);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      isLoading = false;
    }
  }

  async function runExplain() {
    if (props.mode !== 'explain') return;
    isLoading = true;
    error = null;
    result = '';
    try {
      result = await explainQuery(
        config,
        props.sql,
        props.connectionId,
        props.database,
        dialectInfo ?? defaultDialectInfo,
      );
      await persistFirstExchange(`Explain this SQL:\n\n${props.sql}`);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      isLoading = false;
    }
  }

  async function runSummarise() {
    if (props.mode !== 'summarise') return;
    isLoading = true;
    error = null;
    result = '';
    try {
      result = await summariseResult(config, props.sql, props.columns, props.rows);
      await persistFirstExchange(`Summarise the results of:\n\n${props.sql}`);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      isLoading = false;
    }
  }

  async function runDescribe() {
    if (props.mode !== 'describe') return;
    isLoading = true;
    error = null;
    result = '';
    try {
      result = await describeTable(
        config,
        props.tableName,
        props.ddl,
        props.connectionId,
        props.database,
        dialectInfo ?? defaultDialectInfo,
      );
      await persistFirstExchange(`Describe the table "${props.tableName}".`);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      isLoading = false;
    }
  }
</script>

<Modal
  zindex={200}
  label={props.mode === 'generate'
    ? 'Generate query'
    : props.mode === 'explain'
      ? 'Explain query'
      : props.mode === 'describe'
        ? 'Describe table'
        : 'Summarise results'}
  onbackdropclick={props.onclose}
>
  <div class="ai-modal">
    <header class="ai-modal-header">
      <h2 class="ai-modal-title">
        {props.mode === 'generate'
          ? 'Generate Query'
          : props.mode === 'explain'
            ? 'Explain Query'
            : props.mode === 'describe'
              ? 'Describe Table'
              : 'Summarise Results'}
      </h2>
      <button class="ai-modal-close" onclick={props.onclose} aria-label="Close">
        <CloseIcon width={12} height={12} strokeWidth={2.5} />
      </button>
    </header>

    <div class="ai-modal-body" bind:this={bodyEl}>
      {#if props.mode === 'generate' && config.contextLevel === 'none'}
        <div class="ai-context-warning">
          AI has no schema context — results may be inaccurate. Enable schema access in Settings →
          AI.
        </div>
      {/if}
      {#if props.mode === 'generate'}
        <textarea
          bind:this={promptEl}
          class="ai-prompt"
          bind:value={prompt}
          placeholder="Describe the query you want…"
          rows="3"
          onkeydown={(e) => {
            if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
              e.preventDefault();
              runGenerate();
            }
            if (e.key === 'Escape') props.onclose();
          }}
        ></textarea>
      {:else if props.mode === 'explain'}
        <div class="ai-source-sql">
          <pre class="ai-source-sql-pre"><SqlHighlight sql={props.sql} /></pre>
        </div>
      {:else if props.mode === 'describe'}
        <div class="ai-source-sql">
          <pre class="ai-source-sql-pre"><SqlHighlight sql={props.ddl} /></pre>
        </div>
      {:else}
        <div class="ai-source-sql">
          <pre class="ai-source-sql-pre"><SqlHighlight sql={props.sql} /></pre>
        </div>
      {/if}

      {#if error}
        <div class="ai-error">{error}</div>
      {:else if isLoading}
        <div class="ai-loading">
          <Spinner size={14} label="Loading" />
          {props.mode === 'generate'
            ? 'Generating…'
            : props.mode === 'explain'
              ? 'Explaining…'
              : props.mode === 'describe'
                ? 'Describing…'
                : 'Summarising…'}
        </div>
      {:else if result}
        <div class="ai-response">
          {#if props.mode === 'generate' && looksLikeSql(result)}
            <pre class="ai-result"><SqlHighlight sql={stripSqlCodeFence(result)} /></pre>
            <CopyButton text={stripSqlCodeFence(result)} />
          {:else}
            <div class="ai-explain-result markdown-body">{@html marked(result)}</div>
            <CopyButton text={result} />
          {/if}
        </div>

        {#if resumedExisting}
          <div class="ai-resumed-note">Continuing a previous conversation about this.</div>
        {/if}

        {#each followUpPairs as pair, i (i)}
          <div class="ai-followup-question">{pair.question}</div>
          <div class="ai-response">
            {#if props.mode === 'generate' && looksLikeSql(pair.answer)}
              <pre class="ai-result"><SqlHighlight sql={stripSqlCodeFence(pair.answer)} /></pre>
              <CopyButton text={stripSqlCodeFence(pair.answer)} />
            {:else}
              <div class="ai-explain-result markdown-body">{@html marked(pair.answer)}</div>
              <CopyButton text={pair.answer} />
            {/if}
          </div>
        {/each}

        {#if pendingQuestion}
          <div class="ai-followup-question">{pendingQuestion}</div>
          <div class="ai-loading">
            <Spinner size={14} label="Loading" />
            Thinking…
          </div>
        {/if}

        {#if followUpError}
          <div class="ai-error">{followUpError}</div>
        {/if}

        {#if conversation}
          <div class="ai-followup-input">
            <textarea
              class="ai-followup-textarea"
              bind:value={followUpText}
              placeholder="Ask a follow-up…"
              rows="2"
              disabled={followUpLoading}
              onkeydown={(e) => {
                if (e.key === 'Enter' && !e.shiftKey) {
                  e.preventDefault();
                  sendFollowUp();
                }
              }}
            ></textarea>
            <button
              class="ai-btn ai-btn--primary"
              onclick={sendFollowUp}
              disabled={followUpLoading || !followUpText.trim()}
            >
              {#if followUpLoading}
                <Spinner size={12} label="Sending" />
              {:else}
                Send
              {/if}
            </button>
          </div>
        {/if}
      {/if}
    </div>

    <footer class="ai-modal-footer">
      <button class="ai-btn" onclick={props.onclose}>
        {props.mode === 'generate' ? 'Cancel' : 'Close'}
      </button>
      {#if props.mode === 'generate'}
        {#if result}
          <button class="ai-btn" onclick={runGenerate} disabled={isLoading || !prompt.trim()}>
            Regenerate
          </button>
          <button
            class="ai-btn ai-btn--primary"
            onclick={() => {
              if (props.mode === 'generate') props.oninsert(stripSqlCodeFence(latestAssistantContent));
            }}
            disabled={!looksLikeSql(latestAssistantContent)}
            title={looksLikeSql(latestAssistantContent)
              ? undefined
              : 'The latest reply is not a SQL query'}
          >
            Insert into editor
          </button>
        {:else}
          <button
            class="ai-btn ai-btn--primary"
            onclick={runGenerate}
            disabled={isLoading || !prompt.trim()}
          >
            {#if isLoading}
              <Spinner size={12} label="Generating" />
            {:else}
              Generate
            {/if}
          </button>
        {/if}
      {:else if result || error}
        <button
          class="ai-btn ai-btn--primary"
          onclick={() => initConversation(true)}
          disabled={isLoading}
        >
          New conversation
        </button>
      {/if}
    </footer>
  </div>
</Modal>

<style>
  .ai-modal {
    background: var(--color-bg-overlay);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    width: 600px;
    max-width: calc(100vw - var(--spacing-8));
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .ai-modal-header {
    display: flex;
    align-items: center;
    padding: var(--spacing-4) var(--spacing-5);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .ai-modal-title {
    flex: 1;
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
  }

  .ai-modal-close {
    color: var(--color-text-muted);
    padding: var(--spacing-1);
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    transition:
      color var(--transition-fast),
      background var(--transition-fast);
  }

  .ai-context-warning {
    padding: var(--spacing-2) var(--spacing-3);
    background: var(--color-warning-subtle);
    border: 1px solid var(--color-warning);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    color: var(--color-warning);
  }

  .ai-modal-close:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .ai-modal-body {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-4) var(--spacing-5);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-3);
    min-height: 0;
  }

  .ai-prompt {
    width: 100%;
    box-sizing: border-box;
    resize: vertical;
    min-height: 72px;
    padding: var(--spacing-2) var(--spacing-3);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-primary);
    color: var(--color-text-primary);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    outline: none;
    line-height: 1.5;
    transition: border-color var(--transition-fast);
  }

  .ai-prompt:focus {
    border-color: var(--color-accent);
  }

  .ai-source-sql {
    background: var(--color-editor-bg);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    overflow: auto;
    max-height: 130px;
    flex-shrink: 0;
  }

  .ai-source-sql-pre {
    margin: 0;
    padding: var(--spacing-3);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-mono);
    white-space: pre-wrap;
    word-break: break-word;
    -webkit-user-select: text;
    user-select: text;
  }

  .ai-error {
    padding: var(--spacing-2) var(--spacing-3);
    background: var(--color-danger-subtle);
    border: 1px solid rgba(239, 68, 68, 0.3);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-sm);
    color: var(--color-danger);
  }

  .ai-loading {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    font-style: italic;
  }

  .ai-result {
    margin: 0;
    padding: var(--spacing-3);
    background: var(--color-editor-bg);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-mono);
    white-space: pre-wrap;
    word-break: break-word;
    -webkit-user-select: text;
    user-select: text;
    overflow: auto;
    flex-shrink: 0;
  }

  .ai-explain-result,
  .ai-explain-result :global(*) {
    -webkit-user-select: text;
    user-select: text;
  }

  .ai-resumed-note {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
  }

  .ai-response {
    position: relative;
    flex-shrink: 0;
  }

  .ai-response .ai-result,
  .ai-response .ai-explain-result.markdown-body {
    padding-right: calc(var(--spacing-5) + 12px);
  }

  .ai-followup-question {
    align-self: flex-end;
    max-width: 85%;
    padding: var(--spacing-2) var(--spacing-3);
    border-radius: var(--radius-sm);
    background: var(--color-accent-subtle, var(--color-bg-secondary));
    color: var(--color-text-primary);
    font-size: var(--font-size-sm);
    white-space: pre-wrap;
    word-break: break-word;
    -webkit-user-select: text;
    user-select: text;
  }

  .ai-followup-input {
    display: flex;
    gap: var(--spacing-2);
    align-items: flex-end;
    flex-shrink: 0;
  }

  .ai-followup-textarea {
    flex: 1;
    box-sizing: border-box;
    resize: vertical;
    min-height: 44px;
    padding: var(--spacing-2) var(--spacing-3);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-primary);
    color: var(--color-text-primary);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    outline: none;
    line-height: 1.5;
    transition: border-color var(--transition-fast);
  }

  .ai-followup-textarea:focus {
    border-color: var(--color-accent);
  }

  :global(.ai-explain-result.markdown-body) {
    font-size: var(--font-size-sm);
    line-height: 1.7;
    color: var(--color-text-primary);
  }

  :global(.ai-explain-result.markdown-body h1),
  :global(.ai-explain-result.markdown-body h2),
  :global(.ai-explain-result.markdown-body h3),
  :global(.ai-explain-result.markdown-body h4) {
    font-weight: var(--font-weight-semibold);
    line-height: 1.25;
    margin-top: 1.2rem;
    margin-bottom: 0.4rem;
    color: var(--color-text-primary);
  }

  :global(.ai-explain-result.markdown-body h1) {
    font-size: 1.2rem;
  }
  :global(.ai-explain-result.markdown-body h2) {
    font-size: 1.1rem;
  }
  :global(.ai-explain-result.markdown-body h3) {
    font-size: 1rem;
  }

  :global(.ai-explain-result.markdown-body p) {
    margin: 0 0 0.75rem;
  }

  :global(.ai-explain-result.markdown-body p:last-child) {
    margin-bottom: 0;
  }

  :global(.ai-explain-result.markdown-body ul),
  :global(.ai-explain-result.markdown-body ol) {
    padding-left: 1.5rem;
    margin: 0 0 0.75rem;
  }

  :global(.ai-explain-result.markdown-body ul) {
    list-style: disc;
  }
  :global(.ai-explain-result.markdown-body ol) {
    list-style: decimal;
  }

  :global(.ai-explain-result.markdown-body li) {
    margin-bottom: 0.2rem;
  }

  :global(.ai-explain-result.markdown-body code) {
    font-family: var(--font-family-mono);
    font-size: 0.85em;
    padding: 0.15em 0.35em;
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-xs);
  }

  :global(.ai-explain-result.markdown-body pre) {
    background: var(--color-editor-bg);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    padding: var(--spacing-3);
    overflow-x: auto;
    margin: 0 0 0.75rem;
  }

  :global(.ai-explain-result.markdown-body pre code) {
    background: none;
    border: none;
    padding: 0;
    font-size: var(--font-size-sm);
  }

  :global(.ai-explain-result.markdown-body strong) {
    font-weight: var(--font-weight-semibold);
  }

  :global(.ai-explain-result.markdown-body blockquote) {
    border-left: 3px solid var(--color-border-strong);
    padding-left: var(--spacing-3);
    color: var(--color-text-secondary);
    margin: 0 0 0.75rem;
  }

  .ai-modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: var(--spacing-2);
    padding: var(--spacing-4) var(--spacing-5);
    border-top: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .ai-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--spacing-2);
    height: 30px;
    padding: 0 var(--spacing-3);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-secondary);
    color: var(--color-text-secondary);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .ai-btn:hover:not(:disabled) {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .ai-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .ai-btn--primary {
    background: var(--color-accent);
    color: var(--color-text-on-accent);
    border-color: var(--color-accent);
  }

  .ai-btn--primary:hover:not(:disabled) {
    background: var(--color-accent-hover);
    border-color: var(--color-accent-hover);
  }
</style>
