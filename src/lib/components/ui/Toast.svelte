<!--
  Toast — renders a stack of ephemeral notifications in the bottom-right corner.
  Reads from the toast store; each toast auto-dismisses after its duration.
-->
<script lang="ts">
  import { useToast } from '$lib/stores/toast.svelte';
  import CheckIcon from '$lib/components/icons/CheckIcon.svelte';
  import CloseIcon from '$lib/components/icons/CloseIcon.svelte';
  import WarningIcon from '$lib/components/icons/WarningIcon.svelte';
  import InfoCircleIcon from '$lib/components/icons/InfoCircleIcon.svelte';
  import CopyIcon from '$lib/components/icons/CopyIcon.svelte';

  const toastStore = useToast();
</script>

<div class="toast-stack" aria-live="polite" aria-atomic="false">
  {#each toastStore.toasts as toast (toast.id)}
    <div class="toast toast--{toast.type}" role="status" aria-label="{toast.type}: {toast.message}">
      <span class="toast-icon" aria-hidden="true">
        {#if toast.type === 'success'}
          <CheckIcon width={13} height={13} strokeWidth={2.5} />
        {:else if toast.type === 'error'}
          <CloseIcon width={13} height={13} strokeWidth={2.5} />
        {:else if toast.type === 'warning'}
          <WarningIcon width={13} height={13} />
        {:else}
          <InfoCircleIcon width={13} height={13} />
        {/if}
      </span>
      <span class="toast-message">{toast.message}</span>
      {#if toast.type === 'error'}
        <button
          class="toast-copy"
          onclick={() => navigator.clipboard.writeText(toast.message)}
          aria-label="Copy error message"
          title="Copy"
        >
          <CopyIcon width={11} height={11} />
        </button>
      {/if}
      <button
        class="toast-dismiss"
        onclick={() => toastStore.dismiss(toast.id)}
        aria-label="Dismiss notification"
        title="Dismiss"
        ><CloseIcon width={10} height={10} strokeWidth={2.5} /></button
      >
    </div>
  {/each}
</div>

<style>
  .toast-stack {
    position: fixed;
    bottom: var(--spacing-4);
    right: var(--spacing-4);
    z-index: 1000;
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
    pointer-events: none;
    max-width: 360px;
    width: calc(100vw - var(--spacing-8));
  }

  .toast {
    display: flex;
    align-items: flex-start;
    gap: var(--spacing-2);
    padding: var(--spacing-3) var(--spacing-3);
    border-radius: var(--radius-md);
    border: 1px solid var(--color-border);
    box-shadow: var(--shadow-overlay);
    background: var(--color-bg-overlay);
    pointer-events: auto;
    animation: toast-in 150ms ease forwards;
  }

  @keyframes toast-in {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .toast--info {
    border-left: 3px solid var(--color-accent);
  }

  .toast--success {
    border-left: 3px solid var(--color-success);
  }

  .toast--error {
    border-left: 3px solid var(--color-danger);
  }

  .toast--warning {
    border-left: 3px solid var(--color-warning);
  }

  .toast-icon {
    flex-shrink: 0;
    font-size: var(--font-size-sm);
    line-height: 1.4;
  }

  .toast--info .toast-icon {
    color: var(--color-accent);
  }
  .toast--success .toast-icon {
    color: var(--color-success);
  }
  .toast--error .toast-icon {
    color: var(--color-danger);
  }
  .toast--warning .toast-icon {
    color: var(--color-warning);
  }

  .toast-message {
    flex: 1;
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    color: var(--color-text-primary);
    line-height: var(--line-height-normal);
    word-break: break-word;
  }

  .toast-copy {
    flex-shrink: 0;
    width: 18px;
    height: 18px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-muted);
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    cursor: pointer;
    padding: 0;
    opacity: 0.7;
    transition:
      color var(--transition-fast),
      background var(--transition-fast),
      opacity var(--transition-fast);
  }

  .toast-copy:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
    opacity: 1;
  }

  .toast-dismiss {
    flex-shrink: 0;
    width: 18px;
    height: 18px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    cursor: pointer;
    line-height: 1;
    padding: 0;
    transition:
      color var(--transition-fast),
      background var(--transition-fast);
  }

  .toast-dismiss:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }
</style>
