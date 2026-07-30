<!--
  AiChatThread — the message thread + follow-up composer for a single AI
  conversation. Shared between the right-sidebar AiChatPanel and the
  standalone "ai_chat" center-panel tab, so both surfaces behave identically.
-->
<script lang="ts">
  import { tick, untrack } from 'svelte';
  import { useAiChat } from '$lib/stores/aiChat.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { useToast } from '$lib/stores/toast.svelte';
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
  import * as schemaApi from '$lib/tauri/schema';
  import SqlHighlight from '$lib/components/ui/SqlHighlight.svelte';
  import TableIcon from '$lib/components/icons/TableIcon.svelte';
  import TrashIcon from '$lib/components/icons/TrashIcon.svelte';
  import OpenInPanelIcon from '$lib/components/icons/OpenInPanelIcon.svelte';
  import Spinner from '$lib/components/ui/Spinner.svelte';
  import CopyButton from '$lib/components/ui/CopyButton.svelte';
  import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
  import Select from '$lib/components/ui/Select.svelte';
  import ChatCodeBlock from '$lib/components/ai/ChatCodeBlock.svelte';
  import ConnectionPickerDialog from '$lib/components/ai/ConnectionPickerDialog.svelte';

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
  const panels = usePanels();
  const settingsStore = useSettings();
  const connections = useConnections();
  const toast = useToast();

  // A chat opened via "New AI chat" gets a client-only `draft-` id and isn't
  // persisted (or shown in history) until its first message is actually sent.
  // conversationId can transiently be missing while the host panel is mid
  // re-render (e.g. right after opening another tab shifts focus elsewhere),
  // so guard rather than crash on `.startsWith`.
  const isDraft = $derived(!conversationId || conversationId.startsWith('draft-'));
  const draftConversation = $derived<AiConversation>({
    id: conversationId,
    mode: 'chat',
    contextKey: null,
    title: 'New Chat',
    connectionId: null,
    database: null,
    createdAt: '',
    updatedAt: '',
  });

  const config = $derived<AiConfig>({
    provider: settingsStore.settings.aiProvider,
    model: settingsStore.settings.aiModel,
    apiKey: settingsStore.settings.aiApiKey,
    baseUrl: settingsStore.settings.aiBaseUrl,
    contextLevel: settingsStore.settings.aiContextLevel,
    dataSampleRows: settingsStore.settings.aiDataSampleRows,
  });

  const conversation = $derived(
    !conversationId ? null : isDraft ? draftConversation : aiChat.getById(conversationId) ?? null,
  );
  const messages = $derived(isDraft ? [] : aiChat.getMessages(conversationId));

  let threadBodyEl = $state<HTMLDivElement | undefined>(undefined);

  async function scrollToBottom() {
    await tick();
    threadBodyEl?.scrollTo({ top: threadBodyEl.scrollHeight });
  }

  // ── Markdown + code block rendering ─────────────────────────────────────────

  type MessageSegment =
    | { type: 'text'; html: string }
    | { type: 'code'; code: string; lang?: string };

  // Pulls fenced code blocks out of a reply so they can be rendered by
  // ChatCodeBlock (syntax coloring, copy, "open in editor") instead of the
  // plain <pre><code> that marked() would otherwise produce for them.
  function parseMessageSegments(content: string): MessageSegment[] {
    const segments: MessageSegment[] = [];
    const fenceRe = /```(\w+)?\r?\n([\s\S]*?)```/g;
    let lastIndex = 0;
    let match: RegExpExecArray | null;
    while ((match = fenceRe.exec(content)) !== null) {
      const text = content.slice(lastIndex, match.index);
      if (text.trim()) segments.push({ type: 'text', html: marked(text) as string });
      segments.push({ type: 'code', code: match[2].replace(/\n$/, ''), lang: match[1]?.toLowerCase() });
      lastIndex = fenceRe.lastIndex;
    }
    const rest = content.slice(lastIndex);
    if (rest.trim()) segments.push({ type: 'text', html: marked(rest) as string });
    return segments;
  }

  // Best-effort connection to target when opening a code block's SQL in a new
  // query editor: the chat's fixed context, then whatever the user last picked
  // in the ad-hoc schema selector, then whichever connection the focused panel
  // is already scoped to.
  function resolveEditorConnection(): { connectionId: string; database?: string } | null {
    if (conversation?.connectionId) {
      return { connectionId: conversation.connectionId, database: conversation.database ?? undefined };
    }
    if (adHocConnectionId) return { connectionId: adHocConnectionId, database: adHocDatabase || undefined };
    if (lastEditorConnection) return lastEditorConnection;
    const focused = panels.focusedPanel.content;
    if ('connectionId' in focused && focused.connectionId) {
      return { connectionId: focused.connectionId, database: 'database' in focused ? focused.database : undefined };
    }
    return null;
  }

  // Set when openInQueryEditor can't resolve a connection on its own, so the
  // user can pick one; the pending SQL waits here until they confirm or cancel.
  let pendingEditorSql = $state<string | null>(null);

  function openInQueryEditor(sql: string) {
    const target = resolveEditorConnection();
    if (!target) {
      pendingEditorSql = sql;
      return;
    }
    panels.openInFocused({
      kind: 'query_editor',
      connectionId: target.connectionId,
      database: target.database,
      initialSql: sql,
    });
  }

  async function confirmEditorConnection(
    connectionId: string,
    database: string | undefined,
    remember: boolean,
  ) {
    if (pendingEditorSql === null) return;
    // Snapshot before opening the editor: openInFocused switches the split's
    // focused tab away from this chat, which unmounts this very component (a
    // split only keeps its currently-focused item's component alive) — so any
    // $derived read (conversation, isDraft) made after that call reflects a
    // torn-down instance, not the chat we were just looking at.
    const targetConversation = conversation;
    const wasDraft = isDraft;
    panels.openInFocused({ kind: 'query_editor', connectionId, database, initialSql: pendingEditorSql });
    pendingEditorSql = null;
    if (!remember) return;
    // Set immediately so later clicks in this session don't re-prompt even if
    // the persist call below fails (e.g. the app backend hasn't picked up a
    // newly added command yet).
    lastEditorConnection = { connectionId, database: database ?? '' };
    if (!targetConversation || wasDraft) return;
    try {
      const updated = await aiChat.setConnection(targetConversation.id, connectionId, database ?? null);
      console.log('[AI] Remembered connection for chat', targetConversation.id, updated);
    } catch (err) {
      // Not folded into followUpError: that banner gets cleared by the very
      // next send, and this can fail well before the user sends again.
      console.error('[AI] Failed to remember connection for chat', targetConversation.id, err);
      toast.addToast(`Couldn't remember this connection for the chat: ${errorMessage(err)}`, 'error', 0);
    }
  }

  // ── Follow-up ──────────────────────────────────────────────────────────────

  let followUpText = $state('');
  let followUpLoading = $state(false);
  let followUpError = $state<string | null>(null);
  let pendingQuestion = $state<string | null>(null);
  let includeSchema = $state(false);
  // Set right before sending, so the UI can confirm what (if anything) was
  // actually attached to the last message — useful for verifying the toggle worked.
  let lastSchemaAttachment = $state<{ database: string; chars: number } | null>(null);

  // When the conversation isn't already scoped to one connection+database
  // (e.g. general chat mode), the user picks one ad hoc to attach structure from.
  let adHocConnectionId = $state('');
  let adHocDatabase = $state('');
  let adHocDatabases = $state<string[]>([]);
  let adHocDbLoading = $state(false);

  // The connection/database a message was actually sent with (fixed context or
  // whatever was ad hoc-picked at send time). adHocConnectionId/adHocDatabase get
  // reset right after sending so the picker is blank for the next message, so this
  // is what "open in query editor" falls back on for general chat mode.
  let lastEditorConnection = $state<{ connectionId: string; database: string } | null>(null);

  const hasFixedContext = $derived(!!conversation?.connectionId && !!conversation?.database);

  const connectionOptions = $derived([
    { value: '', label: 'Select connection…' },
    ...connections.profiles
      .filter((p) => connections.isActive(p.id))
      .map((p) => ({ value: p.id, label: p.name })),
  ]);

  const adHocDatabaseOptions = $derived([
    { value: '', label: adHocDbLoading ? 'Loading…' : 'Select database…' },
    ...adHocDatabases.map((db) => ({ value: db, label: db })),
  ]);

  async function onAdHocConnectionChange(id: string) {
    adHocConnectionId = id;
    adHocDatabase = '';
    adHocDatabases = [];
    if (!id) return;
    adHocDbLoading = true;
    try {
      adHocDatabases = await schemaApi.listDatabases(id);
      if (adHocDatabases.length === 1) adHocDatabase = adHocDatabases[0];
    } catch {
      adHocDatabases = [];
    } finally {
      adHocDbLoading = false;
    }
  }

  // Reset per-thread UI state and (re)load messages whenever the conversation changes.
  // Restores any unsent draft for the new conversation, and persists whatever was
  // typed here back to the store when navigating away (cleanup), so it isn't lost.
  // Everything inside is untracked so this effect depends ONLY on conversationId —
  // otherwise reading/writing the draft store (a $state map) would make the effect
  // re-run whenever the draft changes, immediately clobbering what was just typed.
  $effect(() => {
    const id = conversationId;
    const draft = isDraft;
    untrack(() => {
      followUpText = draft ? '' : aiChat.getDraft(id);
      followUpLoading = false;
      followUpError = null;
      pendingQuestion = null;
      includeSchema = false;
      adHocConnectionId = '';
      adHocDatabase = '';
      adHocDatabases = [];
      lastSchemaAttachment = null;
      lastEditorConnection = null;
      if (draft) return;
      if (!aiChat.loaded) aiChat.loadConversations();
      void aiChat.ensureMessagesLoaded(id).then(scrollToBottom);
    });

    return () => {
      if (draft) return;
      untrack(() => aiChat.setDraft(id, followUpText));
    };
  });

  async function buildFollowUpSystemPrompt(
    conv: AiConversation,
    chatSchemaContext?: string,
  ): Promise<string> {
    const dialectInfo =
      connections.getById(conv.connectionId ?? '')?.dialectInfo ?? defaultDialectInfo;
    return buildFollowUpSystemPromptForMode(
      conv.mode,
      config,
      conv.connectionId ?? '',
      conv.database ?? '',
      dialectInfo,
      chatSchemaContext,
    );
  }

  // Only 'generate'/'explain'/'describe' fold full schema into the system prompt
  // automatically when schema context is enabled in settings (see
  // buildFollowUpSystemPrompt) — 'summarise' and 'chat' never do, regardless of
  // that setting — so the manual toggle only needs to fetch anything when that
  // automatic path doesn't apply.
  const schemaAlreadyAutomatic = $derived(
    !!conversation &&
      conversation.mode !== 'summarise' &&
      conversation.mode !== 'chat' &&
      config.contextLevel !== 'none',
  );

  async function sendFollowUp() {
    if (!followUpText.trim() || !conversation || followUpLoading) return;
    if (includeSchema && !schemaAlreadyAutomatic && !hasFixedContext && (!adHocConnectionId || !adHocDatabase)) {
      followUpError = 'Select a connection and database to include table structures, or turn off "Table structures".';
      return;
    }
    followUpLoading = true;
    followUpError = null;
    const text = followUpText.trim();
    followUpText = '';
    pendingQuestion = text;
    const attachSchema = includeSchema;
    includeSchema = false;
    await scrollToBottom();
    const schemaConnectionId = conversation.connectionId ?? adHocConnectionId;
    const schemaDatabase = conversation.database ?? adHocDatabase;
    if (schemaConnectionId) lastEditorConnection = { connectionId: schemaConnectionId, database: schemaDatabase };
    adHocConnectionId = '';
    adHocDatabase = '';
    adHocDatabases = [];
    try {
      let schemaContext: string | undefined;
      if (attachSchema && !schemaAlreadyAutomatic && schemaConnectionId && schemaDatabase) {
        const dialectInfo =
          connections.getById(schemaConnectionId)?.dialectInfo ?? defaultDialectInfo;
        schemaContext = await buildSchemaContext(
          schemaConnectionId,
          schemaDatabase,
          'structure',
          0,
          dialectInfo,
        );
      }
      lastSchemaAttachment = schemaContext
        ? { database: schemaDatabase, chars: schemaContext.length }
        : null;
      // Attaching structure implies the chat is now about this connection —
      // link it so later actions (e.g. "open in query editor") don't need to
      // ask again.
      const linkConnectionId = schemaContext ? schemaConnectionId : null;
      const linkDatabase = schemaContext ? schemaDatabase : null;
      // For 'chat' mode, fold the schema + strict "use only these names" rules into
      // the system prompt (stronger adherence than appending it to the user message).
      const systemPrompt =
        conversation.mode === 'chat'
          ? await buildFollowUpSystemPrompt(conversation, schemaContext)
          : await buildFollowUpSystemPrompt(conversation);
      // Already folded into the system prompt above for chat mode — don't send it
      // again in the user message, which would just double token usage.
      const userMessageSchemaContext = conversation.mode === 'chat' ? undefined : schemaContext;
      if (isDraft) {
        const title = text.length > 60 ? `${text.slice(0, 60)}…` : text;
        const { conversation: created } = await aiChat.startConversation(
          { mode: 'chat', contextKey: null, connectionId: linkConnectionId, database: linkDatabase, title },
          config,
          systemPrompt,
          text,
          userMessageSchemaContext,
        );
        panels.updateAiChatConversationId(conversationId, created.id);
      } else {
        await aiChat.sendFollowUp(
          conversation.id,
          config,
          systemPrompt,
          text,
          userMessageSchemaContext,
        );
        if (
          linkConnectionId &&
          (conversation.connectionId !== linkConnectionId || conversation.database !== linkDatabase)
        ) {
          // Isolated from the outer catch: the message already sent
          // successfully, so a failure here shouldn't look like a failed send.
          try {
            await aiChat.setConnection(conversation.id, linkConnectionId, linkDatabase);
          } catch (err) {
            console.error('[AI] Failed to link connection after attaching structure', conversation.id, err);
            toast.addToast(`Couldn't link this chat to its connection: ${errorMessage(err)}`, 'error', 0);
          }
        }
      }
    } catch (err) {
      followUpError = errorMessage(err);
      followUpText = text;
      includeSchema = attachSchema;
      if (!conversation.connectionId && !conversation.database) {
        adHocConnectionId = schemaConnectionId;
        adHocDatabase = schemaDatabase;
        if (schemaConnectionId) {
          adHocDbLoading = true;
          schemaApi
            .listDatabases(schemaConnectionId)
            .then((dbs) => (adHocDatabases = dbs))
            .finally(() => (adHocDbLoading = false));
        }
      }
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
    if (!isDraft) await aiChat.remove(conversationId);
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
            <div class="thread-result-toolbar">
              <button
                class="chat-code-toolbar-btn"
                onclick={() => openInQueryEditor(stripSqlCodeFence(message.content))}
                title="Open in new query editor"
                aria-label="Open in new query editor"
                type="button"
              >
                <OpenInPanelIcon width={12} height={12} />
              </button>
            </div>
            <CopyButton text={stripSqlCodeFence(message.content)} />
          {:else}
            <div class="thread-answer markdown-body">
              {#each parseMessageSegments(message.content) as segment}
                {#if segment.type === 'code'}
                  <ChatCodeBlock code={segment.code} lang={segment.lang} onOpenInEditor={openInQueryEditor} />
                {:else}
                  {@html segment.html}
                {/if}
              {/each}
            </div>
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
    {#if lastSchemaAttachment}
      <div class="schema-attachment-note">
        <TableIcon size={11} />
        Table structures for "{lastSchemaAttachment.database}" attached to last message ({lastSchemaAttachment.chars.toLocaleString()}
        chars)
      </div>
    {/if}
    {#if !schemaAlreadyAutomatic}
      <div class="followup-options">
        <button
          class="schema-toggle"
          class:active={includeSchema}
          onclick={() => (includeSchema = !includeSchema)}
          disabled={followUpLoading}
          title="Include table structures with this message"
          type="button"
        >
          <TableIcon size={12} />
          Table structures
        </button>
        {#if includeSchema && !hasFixedContext}
          <Select
            aria-label="Connection"
            bind:value={adHocConnectionId}
            options={connectionOptions}
            onchange={onAdHocConnectionChange}
            disabled={followUpLoading}
            size="sm"
            searchable
          />
          <Select
            aria-label="Database"
            bind:value={adHocDatabase}
            options={adHocDatabaseOptions}
            disabled={followUpLoading || !adHocConnectionId || adHocDbLoading}
            size="sm"
            searchable
          />
        {/if}
      </div>
    {:else}
      <div class="followup-options">
        <span class="schema-auto-note" title="AI schema context is enabled in Settings">
          <TableIcon size={12} />
          Table structures included
        </span>
      </div>
    {/if}
    <div class="followup-input-row">
      <textarea
        class="followup-textarea"
        bind:value={followUpText}
        placeholder={messages.length > 0 ? 'Continue this conversation…' : 'Ask me anything…'}
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

{#if pendingEditorSql !== null}
  <ConnectionPickerDialog
    title="Open in query editor"
    message="This chat isn't tied to a connection. Choose one to open the query in."
    rememberLabel="Always use this connection for this chat"
    onconfirm={confirmEditorConnection}
    oncancel={() => (pendingEditorSql = null)}
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

  .thread-result-toolbar {
    position: absolute;
    top: var(--spacing-2);
    right: calc(var(--spacing-2) + 24px);
  }

  .chat-code-toolbar-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 4px;
    color: var(--color-text-muted);
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    cursor: pointer;
    opacity: 0.8;
    transition:
      opacity var(--transition-fast),
      background var(--transition-fast);
  }

  .chat-code-toolbar-btn:hover {
    opacity: 1;
    background: var(--color-bg-hover);
  }

  .thread-result {
    margin: 0;
    padding: var(--spacing-3) calc(var(--spacing-5) + 36px) var(--spacing-3) var(--spacing-3);
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
    align-items: center;
    gap: var(--spacing-2);
    flex-wrap: wrap;
  }

  .schema-attachment-note {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
  }

  .schema-auto-note {
    display: inline-flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: 2px var(--spacing-2);
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
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
