<!--
  OnboardingTip — a dismissible banner shown once to first-time users
  when no database connections exist.
  Dismissed state is persisted in localStorage.
-->
<script lang="ts">
  import { useConnections } from '$lib/stores/connections.svelte';

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

  const visible = $derived(
    !dismissed && connectionStore.profiles.length === 0,
  );
</script>

{#if visible}
  <div class="onboarding-tip" role="complementary" aria-label="Getting started tip">
    <div class="tip-content">
      <span class="tip-icon" aria-hidden="true">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M18 11V6a2 2 0 0 0-2-2v0a2 2 0 0 0-2 2v0"/><path d="M14 10V4a2 2 0 0 0-2-2v0a2 2 0 0 0-2 2v2"/><path d="M10 10.5V6a2 2 0 0 0-2-2v0a2 2 0 0 0-2 2v8"/><path d="M18 8a2 2 0 1 1 4 0v6a8 8 0 0 1-8 8h-2c-2.8 0-4.5-.86-5.99-2.34l-3.6-3.6a2 2 0 0 1 2.83-2.82L7 15"/></svg>
      </span>
      <div class="tip-body">
        <strong class="tip-title">Welcome to Rowmance!</strong>
        <p class="tip-text">
          Click the <strong>+</strong> button in the sidebar to add your first database connection.
        </p>
      </div>
    </div>
    <button
      class="tip-dismiss"
      onclick={dismiss}
      aria-label="Dismiss getting started tip"
      title="Dismiss"
    ><svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" aria-hidden="true"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg></button>
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
