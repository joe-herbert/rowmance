<script lang="ts">
  import { portal } from '$lib/actions/portal';
  import type { Snippet } from 'svelte';

  interface Props {
    x: number;
    y: number;
    open: boolean;
    onclose: () => void;
    children: Snippet;
    minWidth?: number;
    zIndex?: number;
  }

  let { x, y, open, onclose, children, minWidth = 160, zIndex = 500 }: Props = $props();

  let menuEl = $state<HTMLDivElement | undefined>(undefined);
  let left = $state(0);
  let top = $state(0);
  let maxHeight = $state<number | null>(null);

  $effect(() => {
    if (!open) return;
    const cx = x;
    const cy = y;
    left = cx;
    top = cy;
    maxHeight = null;

    const raf = requestAnimationFrame(() => {
      if (!menuEl) return;
      const { width, height } = menuEl.getBoundingClientRect();
      const margin = 8;
      const maxH = window.innerHeight - margin * 2;
      left = Math.max(margin, Math.min(cx, window.innerWidth - width - margin));
      if (height > maxH) {
        top = margin;
        maxHeight = maxH;
      } else {
        top = Math.max(margin, Math.min(cy, window.innerHeight - height - margin));
        maxHeight = null;
      }
      menuEl.querySelector<HTMLElement>('[role="menuitem"]:not(:disabled)')?.focus();
    });

    function onMousedown(e: MouseEvent) {
      const target = e.target as Element;
      if (!menuEl?.contains(target) && !target.closest?.('[data-ctx-submenu]')) onclose();
    }
    function onKeydown(e: KeyboardEvent) {
      if (e.key === 'Escape') {
        onclose();
        return;
      }
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        if (!menuEl) return;
        const items = Array.from(
          menuEl.querySelectorAll<HTMLElement>('[role="menuitem"]:not(:disabled)'),
        );
        if (!items.length) return;
        const idx = items.indexOf(document.activeElement as HTMLElement);
        const next =
          e.key === 'ArrowDown'
            ? idx === -1
              ? 0
              : (idx + 1) % items.length
            : idx === -1
              ? items.length - 1
              : (idx - 1 + items.length) % items.length;
        items[next]?.focus();
      }
    }
    document.addEventListener('mousedown', onMousedown, true);
    document.addEventListener('keydown', onKeydown, true);
    return () => {
      cancelAnimationFrame(raf);
      document.removeEventListener('mousedown', onMousedown, true);
      document.removeEventListener('keydown', onKeydown, true);
    };
  });
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    bind:this={menuEl}
    class="ctx-menu"
    role="menu"
    tabindex="-1"
    style:top="{top}px"
    style:left="{left}px"
    style:min-width="{minWidth}px"
    style:z-index={zIndex}
    style:max-height={maxHeight !== null ? `${maxHeight}px` : null}
    onclick={(e) => e.stopPropagation()}
    onmousedown={(e) => e.preventDefault()}
    use:portal
  >
    {@render children()}
  </div>
{/if}

<style>
  .ctx-menu {
    position: fixed;
    padding: var(--spacing-1) 0;
    background: var(--color-bg-overlay);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-md);
    overflow-y: auto;
  }
</style>
