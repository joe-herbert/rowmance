<!--
  AiChatThread — the message thread + follow-up composer for a single AI
  conversation. Shared between the right-sidebar AiChatPanel and the
  standalone "ai_chat" center-panel tab, so both surfaces behave identically.
-->
<script lang="ts">
  import { tick, untrack } from 'svelte';
  import { useAiChat } from '$lib/stores/aiChat.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import {
    buildSchemaContext,
    buildFollowUpSystemPrompt as buildFollowUpSystemPromptForMode,
    type AiConfig,
  } from '$lib/ai/service';
  import type { AiConversation } from '$lib/types';
  import { defaultDialectInfo } from '$lib/utils/dialect';
  import { errorMessage } from '$lib/utils/errors';
  import { looksLikeSql, stripSqlCodeFence } from '$lib/utils/sql';
  import { marked } from 'marked';
  import SqlHighlight from '$lib/components/ui/SqlHighlight.svelte';
  import TableIcon from '$lib/components/icons/TableIcon.svelte';
  import TrashIcon from '$lib/components/icons/TrashIcon.svelte';
  import Spinner from '$lib/components/ui/Spinner.svelte';
  import CopyButton from '$lib/components/ui/CopyButton.svelte';
  import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';

  interface Props {
    conversationId: string;
    /** Called after the conversation is deleted, so the host can navigate away. */
    onDeleted?: () => void;
    /** Set false when the host renders its own delete button (e.g. next to
     *  "open in new tab" in the sidebar) and calls requestDelete() itself. */
    showDeleteButton?: boolean;
  }

  const { conversationId, onDeleted, showDeleteButton = true }: Props = $props();

  const aiChat = useAiChat();
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

  const conversation = $derived(aiChat.getById(conversationId) ?? null);
  const messages = $derived(aiChat.getMessages(conversationId));

  let threadBodyEl = $state<HTMLDivElement | undefined>(undefined);

  async function scrollToBottom() {
    await tick();
    threadBodyEl?.scrollTo({ top: threadBodyEl.scrollHeight });
  }

  // ── Follow-up ──────────────────────────────────────────────────────────────

  let followUpText = $state('');
  let followUpLoading = $state(false);
  let followUpError = $state<string | null>(null);
  let pendingQuestion = $state<string | null>(null);
  let includeSchema = $state(false);

  // Reset per-thread UI state and (re)load messages whenever the conversation changes.
  // Restores any unsent draft for the new conversation, and persists whatever was
  // typed here back to the store when navigating away (cleanup), so it isn't lost.
  // Everything inside is untracked so this effect depends ONLY on conversationId —
  // otherwise reading/writing the draft store (a $state map) would make the effect
  // re-run whenever the draft changes, immediately clobbering what was just typed.
  $effect(() => {
    const id = conversationId;
    untrack(() => {
      followUpText = aiChat.getDraft(id);
      followUpLoading = false;
      followUpError = null;
      pendingQuestion = null;
      includeSchema = false;
      if (!aiChat.loaded) aiChat.loadConversations();
      void aiChat.ensureMessagesLoaded(id).then(scrollToBottom);
    });

    return () => {
      untrack(() => aiChat.setDraft(id, followUpText));
    };
  });

  async function buildFollowUpSystemPrompt(conv: AiConversation): Promise<string> {
    const dialectInfo =
      connections.getById(conv.connectionId ?? '')?.dialectInfo ?? defaultDialectInfo;
    return buildFollowUpSystemPromptForMode(
      conv.mode,
      config,
      conv.connectionId ?? '',
      conv.database ?? '',
      dialectInfo,
    );
  }

  // Modes other than 'summarise' already fold full schema into the system prompt
  // whenever schema context is enabled in settings (see buildFollowUpSystemPrompt),
  // so the manual toggle only needs to fetch anything when that automatic path is off.
  const schemaAlreadyAutomatic = $derived(
    !!conversation && conversation.mode !== 'summarise' && config.contextLevel !== 'none',
  );

  async function sendFollowUp() {
    if (!followUpText.trim() || !conversation || followUpLoading) return;
    followUpLoading = true;
    followUpError = null;
    const text = followUpText.trim();
    followUpText = '';
    pendingQuestion = text;
    const attachSchema = includeSchema;
    includeSchema = false;
    await scrollToBottom();
    try {
      const systemPrompt = await buildFollowUpSystemPrompt(conversation);
      let schemaContext: string | undefined;
      if (
        attachSchema &&
        !schemaAlreadyAutomatic &&
        conversation.connectionId &&
        conversation.database
      ) {
        const dialectInfo =
          connections.getById(conversation.connectionId)?.dialectInfo ?? defaultDialectInfo;
        schemaContext = await buildSchemaContext(
          conversation.connectionId,
          conversation.database,
          'structure',
          0,
          dialectInfo,
        );
      }
      await aiChat.sendFollowUp(conversation.id, config, systemPrompt, text, schemaContext);
    } catch (err) {
      followUpError = errorMessage(err);
      followUpText = text;
      includeSchema = attachSchema;
    } finally {
      followUpLoading = false;
      pendingQuestion = null;
      await scrollToBottom();
    }
  }

  // ── Delete ───────────────────────────────────────────────────────────────────

  let confirmingDelete = $state(false);

  export function requestDelete() {
    confirmingDelete = true;
  }

  async function deleteConversation() {
    confirmingDelete = false;
    await aiChat.remove(conversationId);
    onDeleted?.();
  }
</script>

{#if conversation}
  {#if showDeleteButton}
    <div class="thread-toolbar">
      <button
        class="icon-btn danger"
        onclick={requestDelete}
        aria-label="Delete conversation"
        title="Delete conversation"
        type="button"
      >
        <TrashIcon width={13} height={13} />
      </button>
    </div>
  {/if}

  <div class="thread-body" bind:this={threadBodyEl}>
    {#each messages as message (message.id)}
      {#if message.role === 'user'}
        <div class="thread-question">{message.content}</div>
      {:else}
        <div class="thread-response">
          {#if conversation.mode === 'generate' && looksLikeSql(message.content)}
            <pre class="thread-result"><SqlHighlight
                sql={stripSqlCodeFence(message.content)}
              /></pre>
            <CopyButton text={stripSqlCodeFence(message.content)} />
          {:else}
            <div class="thread-answer markdown-body">{@html marked(message.content)}</div>
            <CopyButton text={message.content} />
          {/if}
        </div>
      {/if}
    {/each}

    {#if pendingQuestion}
      <div class="thread-question">{pendingQuestion}</div>
      <div class="ai-loading">
        <Spinner size={14} label="Loading" />
        Thinking…
      </div>
    {/if}

    {#if followUpError}
      <div class="ai-error">{followUpError}</div>
    {/if}
  </div>

  <div class="thread-followup">
    {#if conversation.connectionId && conversation.database}
      <div class="followup-options">
        <button
          class="schema-toggle"
          class:active={includeSchema && !schemaAlreadyAutomatic}
          onclick={() => (includeSchema = !includeSchema)}
          disabled={followUpLoading || schemaAlreadyAutomatic}
          title={schemaAlreadyAutomatic
            ? 'Table structures are already included automatically (AI schema context is enabled in Settings)'
            : 'Include table structures with this message'}
          type="button"
        >
          <TableIcon size={12} />
          {schemaAlreadyAutomatic ? 'Table structures included' : 'Table structures'}
        </button>
      </div>
    {/if}
    <div class="followup-input-row">
      <textarea
        class="followup-textarea"
        bind:value={followUpText}
        placeholder="Continue this conversation…"
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
        class="action-btn action-btn--primary"
        onclick={sendFollowUp}
        disabled={followUpLoading || !followUpText.trim()}
        type="button"
      >
        {#if followUpLoading}
          <Spinner size={12} label="Sending" />
        {:else}
          Send
        {/if}
      </button>
    </div>
  </div>
{/if}

{#if confirmingDelete}
  <ConfirmDialog
    title="Delete conversation"
    message={`Delete "${conversation?.title}"? This cannot be undone.`}
    confirmText="Delete"
    cancelText="Cancel"
    danger
    onconfirm={deleteConversation}
    oncancel={() => (confirmingDelete = false)}
  />
{/if}

<style>
  .thread-toolbar {
    display: flex;
    justify-content: flex-end;
    padding: var(--spacing-1) var(--spacing-2) 0;
    flex-shrink: 0;
  }

  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--spacing-1);
    border-radius: var(--radius-sm);
    color: var(--color-text-muted);
  }

  .icon-btn:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .icon-btn.danger:hover {
    color: var(--color-danger);
    background: var(--color-danger-subtle);
  }

  .thread-body {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-3);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-3);
    min-height: 0;
  }

  .thread-question {
    align-self: flex-end;
    max-width: 90%;
    padding: var(--spacing-2) var(--spacing-3);
    border-radius: var(--radius-sm);
    background: var(--color-bg-secondary);
    color: var(--color-text-primary);
    font-size: var(--font-size-sm);
    white-space: pre-wrap;
    word-break: break-word;
    -webkit-user-select: text;
    user-select: text;
  }

  .thread-response {
    position: relative;
    flex-shrink: 0;
  }

  .thread-response .thread-answer {
    padding-right: calc(var(--spacing-5) + 12px);
  }

  .thread-result {
    margin: 0;
    padding: var(--spacing-3) calc(var(--spacing-5) + 12px) var(--spacing-3) var(--spacing-3);
    background: var(--color-editor-bg);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-mono);
    white-space: pre-wrap;
    word-break: break-word;
    overflow: auto;
    -webkit-user-select: text;
    user-select: text;
  }

  .thread-answer {
    font-size: var(--font-size-sm);
    line-height: 1.6;
    color: var(--color-text-primary);
  }

  .thread-answer,
  .thread-answer :global(*) {
    -webkit-user-select: text;
    user-select: text;
  }

  :global(.thread-answer p) {
    margin: 0 0 0.6rem;
  }
  :global(.thread-answer p:last-child) {
    margin-bottom: 0;
  }
  :global(.thread-answer ul),
  :global(.thread-answer ol) {
    padding-left: 1.4rem;
    margin: 0 0 0.6rem;
  }
  :global(.thread-answer code) {
    font-family: var(--font-family-mono);
    font-size: 0.85em;
    padding: 0.15em 0.35em;
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-xs);
  }
  :global(.thread-answer pre) {
    background: var(--color-editor-bg);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    padding: var(--spacing-3);
    overflow-x: auto;
    margin: 0 0 0.6rem;
  }
  :global(.thread-answer pre code) {
    background: none;
    border: none;
    padding: 0;
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

  .thread-followup {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
    padding: var(--spacing-2);
    border-top: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .followup-options {
    display: flex;
    gap: var(--spacing-2);
  }

  .schema-toggle {
    display: inline-flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: 2px var(--spacing-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-secondary);
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast),
      border-color var(--transition-fast);
  }

  .schema-toggle:hover:not(:disabled) {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .schema-toggle.active {
    background: var(--color-accent-subtle, var(--color-bg-secondary));
    border-color: var(--color-accent);
    color: var(--color-accent);
  }

  .schema-toggle:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .followup-input-row {
    display: flex;
    gap: var(--spacing-2);
    align-items: flex-end;
  }

  .followup-textarea {
    flex: 1;
    box-sizing: border-box;
    resize: vertical;
    min-height: 40px;
    padding: var(--spacing-2) var(--spacing-3);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-primary);
    color: var(--color-text-primary);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    outline: none;
    line-height: 1.4;
  }

  .followup-textarea:focus {
    border-color: var(--color-accent);
  }

  .action-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 30px;
    padding: 0 var(--spacing-3);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-secondary);
    color: var(--color-text-secondary);
    font-size: var(--font-size-sm);
    cursor: pointer;
  }

  .action-btn--primary {
    background: var(--color-accent);
    color: var(--color-text-on-accent);
    border-color: var(--color-accent);
  }

  .action-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
</style>
