<script lang="ts">
  import CopyIcon from '$lib/components/icons/CopyIcon.svelte';
  import CheckIcon from '$lib/components/icons/CheckIcon.svelte';

  interface Props {
    message: string;
  }

  let { message }: Props = $props();

  let copied = $state(false);

  function copy() {
    navigator.clipboard.writeText(message).then(() => {
      copied = true;
      setTimeout(() => (copied = false), 1500);
    });
  }
</script>

<div class="error-message">
  <span class="error-message__text">{message}</span>
  <button
    class="error-message__copy"
    onclick={copy}
    title={copied ? 'Copied' : 'Copy error message'}
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
  .error-message {
    display: flex;
    align-items: flex-start;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-3);
    background: var(--color-danger-subtle);
    border: 1px solid rgba(239, 68, 68, 0.3);
    border-radius: var(--radius-md);
    font-size: var(--font-size-sm);
    color: var(--color-danger);
  }

  .error-message__text {
    flex: 1;
    user-select: text;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .error-message__copy {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    padding: 3px;
    color: var(--color-danger);
    background: transparent;
    border: 1px solid rgba(239, 68, 68, 0.3);
    border-radius: var(--radius-sm);
    cursor: pointer;
    opacity: 0.8;
    transition: opacity var(--transition-fast), background var(--transition-fast);
  }

  .error-message__copy:hover {
    opacity: 1;
    background: rgba(239, 68, 68, 0.15);
  }
</style>
