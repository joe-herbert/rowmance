<!--
  AiChatPanel — right sidebar panel listing persisted AI chat conversations.
  List view shows all conversations; selecting one opens a thread view where
  the user can keep chatting via the same follow-up flow as AiModal (rendered
  by the shared AiChatThread component, also used by the "ai_chat" center tab).
-->
<script lang="ts">
  import { useAiChat } from '$lib/stores/aiChat.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import AiChatThread from '$lib/components/ai/AiChatThread.svelte';
  import ContextMenu from '$lib/components/ui/ContextMenu.svelte';
  import CtxItem from '$lib/components/ui/CtxItem.svelte';
  import CtxSep from '$lib/components/ui/CtxSep.svelte';
  import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
  import ChevronIcon from '$lib/components/icons/ChevronIcon.svelte';
  import OpenInPanelIcon from '$lib/components/icons/OpenInPanelIcon.svelte';
  import TrashIcon from '$lib/components/icons/TrashIcon.svelte';

  const aiChat = useAiChat();
  const panelStore = usePanels();

  const modeLabels: Record<string, string> = {
    generate: 'Generate',
    explain: 'Explain',
    describe: 'Describe',
    summarise: 'Summarise',
    chat: 'Chat',
  };

  $effect(() => {
    if (!aiChat.loaded) {
      aiChat.loadConversations();
    }
  });

  // ── List / thread navigation ──────────────────────────────────────────────

  let selectedId = $state<string | null>(null);
  const selectedConversation = $derived(selectedId ? (aiChat.getById(selectedId) ?? null) : null);
  let threadRef = $state<AiChatThread | undefined>(undefined);

  function openConversation(id: string) {
    selectedId = id;
  }

  function backToList() {
    selectedId = null;
  }

  function openInNewTab() {
    if (!selectedId) return;
    panelStore.openInFocused({ kind: 'ai_chat', conversationId: selectedId });
  }

  // ── Context menu / rename / delete ─────────────────────────────────────────

  interface CtxMenu {
    x: number;
    y: number;
    id: string;
    title: string;
  }
  let ctxMenu = $state<CtxMenu | null>(null);

  function showCtxMenu(e: MouseEvent, id: string, title: string) {
    e.preventDefault();
    ctxMenu = { x: e.clientX, y: e.clientY, id, title };
  }

  let renamingId = $state<string | null>(null);
  let renameValue = $state('');
  let renameInput = $state<HTMLInputElement | undefined>(undefined);

  function startRename(id: string, title: string) {
    ctxMenu = null;
    renamingId = id;
    renameValue = title;
    requestAnimationFrame(() => {
      renameInput?.focus();
      renameInput?.select();
    });
  }

  async function commitRename() {
    if (renamingId && renameValue.trim()) {
      await aiChat.rename(renamingId, renameValue.trim());
    }
    renamingId = null;
  }

  let confirmDeleteId = $state<string | null>(null);
  let confirmDeleteTitle = $state('');

  function startDelete(id: string, title: string) {
    ctxMenu = null;
    confirmDeleteId = id;
    confirmDeleteTitle = title;
  }

  async function confirmDelete() {
    if (!confirmDeleteId) return;
    const id = confirmDeleteId;
    confirmDeleteId = null;
    if (selectedId === id) selectedId = null;
    await aiChat.remove(id);
  }

  function timeAgo(iso: string): string {
    const diffMs = Date.now() - new Date(iso).getTime();
    const mins = Math.floor(diffMs / 60000);
    if (mins < 1) return 'just now';
    if (mins < 60) return `${mins}m ago`;
    const hours = Math.floor(mins / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    return `${days}d ago`;
  }
</script>

<div class="ai-chat-panel">
  {#if !selectedConversation}
    <div class="panel-toolbar">
      <span class="panel-title">AI Chats</span>
    </div>

    {#if aiChat.conversations.length === 0}
      <div class="empty-row">No AI conversations yet.</div>
    {:else}
      <ul class="conversation-list" role="list">
        {#each aiChat.conversations as conversation (conversation.id)}
          <li class="conversation-item" role="listitem">
            {#if renamingId === conversation.id}
              <input
                bind:this={renameInput}
                class="rename-input"
                type="text"
                bind:value={renameValue}
                onblur={commitRename}
                onkeydown={(e) => {
                  if (e.key === 'Enter') commitRename();
                  if (e.key === 'Escape') {
                    renamingId = null;
                  }
                }}
              />
            {:else}
              <button
                class="conversation-btn"
                onclick={() => openConversation(conversation.id)}
                oncontextmenu={(e) => showCtxMenu(e, conversation.id, conversation.title)}
                title={conversation.title}
                type="button"
              >
                <span class="conversation-title">{conversation.title}</span>
                <span class="conversation-meta">
                  <span class="mode-badge"
                    >{modeLabels[conversation.mode] ?? conversation.mode}</span
                  >
                  <span class="time-ago">{timeAgo(conversation.updatedAt)}</span>
                </span>
              </button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  {:else}
    <div class="panel-toolbar">
      <button
        class="icon-btn"
        onclick={backToList}
        aria-label="Back to conversations"
        type="button"
      >
        <ChevronIcon direction="left" width={14} height={14} />
      </button>
      <span class="panel-title thread-title">{selectedConversation.title}</span>
      <button
        class="icon-btn"
        onclick={openInNewTab}
        aria-label="Open in new tab"
        title="Open in new tab"
        type="button"
      >
        <OpenInPanelIcon width={14} height={14} />
      </button>
      <button
        class="icon-btn danger"
        onclick={() => threadRef?.requestDelete()}
        aria-label="Delete conversation"
        title="Delete conversation"
        type="button"
      >
        <TrashIcon width={14} height={14} />
      </button>
    </div>

    <AiChatThread
      bind:this={threadRef}
      conversationId={selectedConversation.id}
      onDeleted={backToList}
      showDeleteButton={false}
    />
  {/if}
</div>

<ContextMenu
  x={ctxMenu?.x ?? 0}
  y={ctxMenu?.y ?? 0}
  open={ctxMenu !== null}
  onclose={() => (ctxMenu = null)}
  minWidth={140}
  zIndex={1000}
>
  {#if ctxMenu}
    <CtxItem onclick={() => openConversation(ctxMenu!.id)}>Open</CtxItem>
    <CtxItem onclick={() => startRename(ctxMenu!.id, ctxMenu!.title)}>Rename</CtxItem>
    <CtxSep />
    <CtxItem danger onclick={() => startDelete(ctxMenu!.id, ctxMenu!.title)}>Delete</CtxItem>
  {/if}
</ContextMenu>

{#if confirmDeleteId}
  <ConfirmDialog
    title="Delete conversation"
    message={`Delete "${confirmDeleteTitle}"? This cannot be undone.`}
    confirmText="Delete"
    cancelText="Cancel"
    danger
    onconfirm={confirmDelete}
    oncancel={() => (confirmDeleteId = null)}
  />
{/if}

<style>
  .ai-chat-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .panel-toolbar {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: 6px var(--spacing-2);
    min-height: 34px;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .panel-title {
    font-size: 10.5px;
    font-weight: 700;
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .thread-title {
    flex: 1;
    text-transform: none;
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--spacing-1);
    border-radius: var(--radius-sm);
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .icon-btn:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .icon-btn.danger:hover {
    color: var(--color-danger);
    background: var(--color-danger-subtle);
  }

  .empty-row {
    padding: var(--spacing-4);
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    text-align: center;
  }

  .conversation-list {
    flex: 1;
    overflow-y: auto;
    list-style: none;
    margin: 0;
    padding: var(--spacing-1);
  }

  .conversation-item {
    margin: 0;
  }

  .conversation-btn {
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: var(--spacing-2) var(--spacing-2);
    border-radius: var(--radius-sm);
    text-align: left;
    color: var(--color-text-primary);
  }

  .conversation-btn:hover {
    background: var(--color-bg-hover);
  }

  .conversation-title {
    font-size: var(--font-size-sm);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }

  .conversation-meta {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  .mode-badge {
    padding: 1px 6px;
    border-radius: var(--radius-xs);
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
  }

  .rename-input {
    width: 100%;
    box-sizing: border-box;
    padding: var(--spacing-2);
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-sm);
    background: var(--color-bg-primary);
    color: var(--color-text-primary);
    font-size: var(--font-size-sm);
    outline: none;
  }
</style>
