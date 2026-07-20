<!--
  OnboardingTip — a dismissible banner shown once to first-time users
  when no database connections exist.
  Dismissed state is persisted in localStorage.
-->
<script lang="ts">
  import { useConnections } from '$lib/stores/connections.svelte';
  import HandIcon from '$lib/components/icons/HandIcon.svelte';
  import CloseIcon from '$lib/components/icons/CloseIcon.svelte';

  const connectionStore = useConnections();

  const STORAGE_KEY = 'onboarding-dismissed';

  let dismissed = $state(false);

  $effect(() => {
    dismissed = localStorage.getItem(STORAGE_KEY) === '1';
  });

  function dismiss() {
    localStorage.setItem(STORAGE_KEY, '1');
    dismissed = true;
  }

  const visible = $derived(!dismissed && connectionStore.profiles.length === 0);
</script>

{#if visible}
  <div class="onboarding-tip" role="complementary" aria-label="Getting started tip">
    <div class="tip-content">
      <span class="tip-icon" aria-hidden="true">
        <HandIcon width={20} height={20} />
      </span>
      <div class="tip-body">
        <strong class="tip-title">Welcome to Rowmance!</strong>
        <p class="tip-text">
          Click the <strong>+ Add Connection</strong> button in the sidebar to add your first database
          connection.
        </p>
      </div>
    </div>
    <button
      class="tip-dismiss"
      onclick={dismiss}
      aria-label="Dismiss getting started tip"
      title="Dismiss"><CloseIcon width={10} height={10} strokeWidth={2.5} /></button
    >
  </div>
{/if}

<style>
  .onboarding-tip {
    position: fixed;
    bottom: calc(var(--spacing-4) + 56px);
    right: var(--spacing-4);
    z-index: 900;
    display: flex;
    align-items: flex-start;
    gap: var(--spacing-2);
    max-width: 340px;
    padding: var(--spacing-3) var(--spacing-3);
    background: var(--color-accent);
    color: var(--color-text-on-accent);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    animation: tip-in 200ms ease forwards;
  }

  @keyframes tip-in {
    from {
      opacity: 0;
      transform: translateY(12px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .tip-content {
    display: flex;
    align-items: flex-start;
    gap: var(--spacing-2);
    flex: 1;
  }

  .tip-icon {
    font-size: var(--font-size-lg);
    flex-shrink: 0;
    line-height: 1;
    margin-top: 1px;
  }

  .tip-body {
    flex: 1;
  }

  .tip-title {
    display: block;
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    font-weight: var(--font-weight-semibold);
    margin-bottom: var(--spacing-1);
  }

  .tip-text {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    line-height: var(--line-height-normal);
    opacity: 0.92;
    margin: 0;
  }

  .tip-dismiss {
    flex-shrink: 0;
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: var(--font-size-xs);
    background: rgba(255, 255, 255, 0.2);
    color: var(--color-text-on-accent);
    border: none;
    border-radius: var(--radius-sm);
    cursor: pointer;
    line-height: 1;
    padding: 0;
    transition: background var(--transition-fast);
  }

  .tip-dismiss:hover {
    background: rgba(255, 255, 255, 0.35);
  }
</style>
