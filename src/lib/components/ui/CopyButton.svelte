<!-- CopyButton — small icon button that copies `text` to the clipboard. -->
<!-- Position it inside a `position: relative` container; it anchors itself top-right. -->
<script lang="ts">
  import CopyIcon from '$lib/components/icons/CopyIcon.svelte';
  import CheckIcon from '$lib/components/icons/CheckIcon.svelte';

  interface Props {
    text: string;
  }

  const { text }: Props = $props();

  let copied = $state(false);
  let resetTimeout: ReturnType<typeof setTimeout> | undefined;

  function copy() {
    navigator.clipboard.writeText(text).then(() => {
      copied = true;
      clearTimeout(resetTimeout);
      resetTimeout = setTimeout(() => (copied = false), 1500);
    });
  }
</script>

<button
  class="copy-btn"
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

<style>
  .copy-btn {
    position: absolute;
    top: var(--spacing-2);
    right: var(--spacing-2);
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

  .copy-btn:hover {
    opacity: 1;
    background: var(--color-bg-hover);
  }
</style>
