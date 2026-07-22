<!-- CodeBlock — a <pre><code> block with a copy-to-clipboard button. -->
<script lang="ts">
  import CopyIcon from '$lib/components/icons/CopyIcon.svelte';
  import CheckIcon from '$lib/components/icons/CheckIcon.svelte';

  interface Props {
    code: string;
  }

  const { code }: Props = $props();

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

<div class="code-block">
  <pre><code>{code}</code></pre>
  <button
    class="code-block__copy"
    onclick={copy}
    title={copied ? 'Copied' : 'Copy to clipboard'}
    type="button"
  >
    {#if copied}
      <CheckIcon width={12} height={12} strokeWidth={2.5} />
    {:else}
      <CopyIcon width={12} height={12} />
    {/if}
  </button>
</div>

<style>
  .code-block {
    position: relative;
    margin: 0.5rem 0 1rem;
  }

  .code-block pre {
    margin: 0;
    padding: 0.75rem 2.25rem 0.75rem 1rem;
    overflow: auto;
    cursor: text;
    font-family:
      ui-monospace,
      SFMono-Regular,
      SF Mono,
      Menlo,
      Consolas,
      monospace;
    font-size: 0.9em;
    line-height: 1.5;
    background: var(--color-surface-raised, rgba(128, 128, 128, 0.1));
    border-radius: var(--radius-md);
  }

  .code-block__copy {
    position: absolute;
    top: 0.4rem;
    right: 0.4rem;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 4px;
    color: var(--color-text-muted, #888);
    background: var(--color-surface, rgba(128, 128, 128, 0.15));
    border: 1px solid var(--color-border, rgba(128, 128, 128, 0.3));
    border-radius: var(--radius-sm);
    cursor: pointer;
    opacity: 0.8;
    transition:
      opacity var(--transition-fast),
      background var(--transition-fast);
    transform: translateZ(0);
  }

  .code-block__copy:hover {
    opacity: 1;
    background: var(--color-surface-raised, rgba(128, 128, 128, 0.25));
  }
</style>
