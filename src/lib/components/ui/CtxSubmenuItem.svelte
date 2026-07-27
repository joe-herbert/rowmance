<script lang="ts">
  import { portal } from '$lib/actions/portal';
  import { getContext, setContext } from 'svelte';
  import type { Snippet } from 'svelte';

  interface Props {
    label: string;
    children: Snippet;
  }

  let { label, children }: Props = $props();

  let triggerEl = $state<HTMLButtonElement | null>(null);
  let submenuEl = $state<HTMLDivElement | null>(null);
  let left = $state(0);
  let top = $state(0);

  // A submenu can itself contain another CtxSubmenuItem (nested folders, etc).
  // Because each level's popup is portaled to <body>, it's not a DOM descendant
  // of its parent's popup, so plain mouseenter/mouseleave containment checks
  // can't tell "the pointer moved into a deeper flyout" from "the pointer left
  // entirely" — the parent would close (and unmount the child) the moment the
  // pointer crosses into a nested popup. Instead, track two independent
  // signals and stay open if either is true: the pointer is directly over
  // this trigger/popup, or a descendant submenu (any depth) reports itself open.
  const CTX_KEY = 'ctx-submenu-child-notify';
  const parentNotify = getContext<{ notifyOpen: () => void; notifyClose: () => void } | undefined>(
    CTX_KEY,
  );
  setContext(CTX_KEY, {
    notifyOpen: () => {
      openDescendantCount += 1;
    },
    notifyClose: () => {
      openDescendantCount = Math.max(0, openDescendantCount - 1);
    },
  });

  let pointerInside = $state(false);
  let openDescendantCount = $state(0);
  const open = $derived(pointerInside || openDescendantCount > 0);

  let closeTimer: ReturnType<typeof setTimeout> | null = null;
  let wasOpen = false;

  $effect(() => {
    if (open !== wasOpen) {
      wasOpen = open;
      if (open) parentNotify?.notifyOpen();
      else parentNotify?.notifyClose();
    }
  });

  function positionSubmenu(): void {
    if (!triggerEl) return;
    const rect = triggerEl.getBoundingClientRect();
    left = rect.right;
    top = rect.top;
    requestAnimationFrame(() => {
      if (!submenuEl) return;
      const { width, height } = submenuEl.getBoundingClientRect();
      if (left + width > window.innerWidth - 8) {
        left = rect.left - width;
      }
      if (top + height > window.innerHeight - 8) {
        top = Math.max(8, window.innerHeight - height - 8);
      }
    });
  }

  function handleEnter(): void {
    if (closeTimer) {
      clearTimeout(closeTimer);
      closeTimer = null;
    }
    const wasInside = pointerInside;
    pointerInside = true;
    if (!wasInside) positionSubmenu();
  }

  function handleLeave(): void {
    if (closeTimer) clearTimeout(closeTimer);
    // Small grace period so a fast diagonal move from the trigger into the
    // popup (or between a popup and its own nested submenu) doesn't flicker.
    closeTimer = setTimeout(() => {
      pointerInside = false;
      closeTimer = null;
    }, 150);
  }
</script>

<button
  bind:this={triggerEl}
  class="ctx-submenu-trigger"
  role="menuitem"
  onmouseenter={handleEnter}
  onmouseleave={handleLeave}
>
  <span>{label}</span>
  <span class="arrow">›</span>
</button>

{#if open}
  <div
    bind:this={submenuEl}
    class="ctx-submenu"
    role="menu"
    tabindex="-1"
    data-ctx-submenu
    style:left="{left}px"
    style:top="{top}px"
    onmouseenter={handleEnter}
    onmouseleave={handleLeave}
    use:portal
  >
    {@render children()}
  </div>
{/if}

<style>
  .ctx-submenu-trigger {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: var(--spacing-1) var(--spacing-3);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    color: var(--color-text-primary);
    text-align: left;
    cursor: pointer;
    transition: background var(--transition-fast);
    background: transparent;
    border: none;
    gap: var(--spacing-4);
  }

  .ctx-submenu-trigger:hover {
    background: var(--color-bg-active);
  }

  .arrow {
    color: var(--color-text-muted);
    font-size: var(--font-size-md);
    line-height: 1;
  }

  .ctx-submenu {
    position: fixed;
    padding: var(--spacing-1) 0;
    background: var(--color-bg-overlay);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-md);
    min-width: 160px;
    z-index: 600;
  }
</style>
