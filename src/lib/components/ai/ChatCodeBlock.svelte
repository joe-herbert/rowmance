<!--
  ChatCodeBlock — a fenced code block inside an AI chat reply. SQL-flavored
  blocks (```sql or unlabeled fences, which is how models usually reply) get
  syntax coloring via SqlHighlight and an "open in new query editor" action;
  other languages render as plain monospace text.
-->
<script lang="ts">
  import SqlHighlight from '$lib/components/ui/SqlHighlight.svelte';
  import CopyIcon from '$lib/components/icons/CopyIcon.svelte';
  import CheckIcon from '$lib/components/icons/CheckIcon.svelte';
  import OpenInPanelIcon from '$lib/components/icons/OpenInPanelIcon.svelte';

  interface Props {
    code: string;
    lang?: string;
    onOpenInEditor?: (code: string) => void;
  }

  const { code, lang, onOpenInEditor }: Props = $props();

  const isSql = $derived(!lang || lang.toLowerCase() === 'sql');

  let copied = $state(false);
  let resetTimeout: ReturnType<typeof setTimeout> | undefined;

  function copy() {
    navigator.clipboard.writeText(code).then(() => {
      copied = true;
      clearTimeout(resetTimeout);
      resetTimeout = setTimeout(() => (copied = false), 1500);
    });
  }
</script>

<div class="chat-code-block">
  <div class="chat-code-toolbar">
    {#if isSql && onOpenInEditor}
      <button
        class="chat-code-toolbar-btn"
        onclick={() => onOpenInEditor(code)}
        title="Open in new query editor"
        aria-label="Open in new query editor"
        type="button"
      >
        <OpenInPanelIcon width={12} height={12} />
      </button>
    {/if}
    <button
      class="chat-code-toolbar-btn"
      onclick={copy}
      title={copied ? 'Copied' : 'Copy to clipboard'}
      aria-label={copied ? 'Copied' : 'Copy to clipboard'}
      type="button"
    >
      {#if copied}
        <CheckIcon width={12} height={12} strokeWidth={2.5} />
      {:else}
        <CopyIcon width={12} height={12} />
      {/if}
    </button>
  </div>
  <pre class="chat-code-pre">{#if isSql}<SqlHighlight sql={code} />{:else}<code>{code}</code>{/if}</pre>
</div>

<style>
  .chat-code-block {
    position: relative;
  }

  .chat-code-pre {
    margin: 0 0 0.6rem;
    padding: var(--spacing-3) calc(var(--spacing-5) + 24px) var(--spacing-3) var(--spacing-3);
    background: var(--color-editor-bg);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-mono);
    white-space: pre-wrap;
    word-break: break-word;
    overflow-x: auto;
    -webkit-user-select: text;
    user-select: text;
  }

  .chat-code-toolbar {
    position: absolute;
    top: var(--spacing-2);
    right: var(--spacing-2);
    display: flex;
    align-items: center;
    gap: 4px;
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
</style>
