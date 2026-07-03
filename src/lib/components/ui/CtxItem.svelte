<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    onclick?: (e: MouseEvent) => void;
    disabled?: boolean;
    danger?: boolean;
    children: Snippet;
  }

  let { onclick, disabled = false, danger = false, children }: Props = $props();
</script>

<button
  class="ctx-item"
  class:ctx-item--danger={danger}
  role="menuitem"
  {disabled}
  {onclick}
>
  {@render children()}
</button>

<style>
  .ctx-item {
    display: block;
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
  }

  .ctx-item:hover:not(:disabled) {
    background: var(--color-bg-active);
  }

  .ctx-item:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .ctx-item--danger {
    color: var(--color-danger);
  }

  .ctx-item--danger:hover:not(:disabled) {
    background: var(--color-danger-subtle);
  }
</style>
