<!--
  Toast — renders a stack of ephemeral notifications in the bottom-right corner.
  Reads from the toast store; each toast auto-dismisses after its duration.
-->
<script lang="ts">
  import { useToast } from '$lib/stores/toast.svelte';

  const toastStore = useToast();
</script>

<div class="toast-stack" aria-live="polite" aria-atomic="false">
  {#each toastStore.toasts as toast (toast.id)}
    <div
      class="toast toast--{toast.type}"
      role="status"
      aria-label="{toast.type}: {toast.message}"
    >
      <span class="toast-icon" aria-hidden="true">
        {#if toast.type === 'success'}
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
        {:else if toast.type === 'error'}
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
        {:else if toast.type === 'warning'}
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
        {:else}
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
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
          <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
        </button>
      {/if}
      <button
        class="toast-dismiss"
        onclick={() => toastStore.dismiss(toast.id)}
        aria-label="Dismiss notification"
        title="Dismiss"
      ><svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" aria-hidden="true"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg></button>
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

  .toast--info .toast-icon { color: var(--color-accent); }
  .toast--success .toast-icon { color: var(--color-success); }
  .toast--error .toast-icon { color: var(--color-danger); }
  .toast--warning .toast-icon { color: var(--color-warning); }

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
    transition: color var(--transition-fast), background var(--transition-fast), opacity var(--transition-fast);
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
    transition: color var(--transition-fast), background var(--transition-fast);
  }

  .toast-dismiss:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }
</style>
