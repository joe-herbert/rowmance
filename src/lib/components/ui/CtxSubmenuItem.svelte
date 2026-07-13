<script lang="ts">
  import { portal } from '$lib/actions/portal';
  import type { Snippet } from 'svelte';

  interface Props {
    label: string;
    children: Snippet;
  }

  let { label, children }: Props = $props();

  let triggerEl = $state<HTMLButtonElement | null>(null);
  let submenuEl = $state<HTMLDivElement | null>(null);
  let open = $state(false);
  let left = $state(0);
  let top = $state(0);

  function openSubmenu(): void {
    if (!triggerEl) return;
    const rect = triggerEl.getBoundingClientRect();
    left = rect.right + 2;
    top = rect.top;
    open = true;
    requestAnimationFrame(() => {
      if (!submenuEl) return;
      const { width, height } = submenuEl.getBoundingClientRect();
      if (left + width > window.innerWidth - 8) {
        left = rect.left - width - 2;
      }
      if (top + height > window.innerHeight - 8) {
        top = Math.max(8, window.innerHeight - height - 8);
      }
    });
  }

  function handleTriggerMouseleave(e: MouseEvent): void {
    if (submenuEl?.contains(e.relatedTarget as Node)) return;
    open = false;
  }

  function handleSubmenuMouseleave(e: MouseEvent): void {
    if (triggerEl?.contains(e.relatedTarget as Node)) return;
    open = false;
  }
</script>

<button
  bind:this={triggerEl}
  class="ctx-submenu-trigger"
  role="menuitem"
  onmouseenter={openSubmenu}
  onmouseleave={handleTriggerMouseleave}
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
    onmouseenter={() => (open = true)}
    onmouseleave={handleSubmenuMouseleave}
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
