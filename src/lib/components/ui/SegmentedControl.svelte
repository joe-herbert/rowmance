<!--
  SegmentedControl — a compact mode-switcher (Builder / SQL, etc.).
  Matches the style used in the filter editor header.
-->
<script lang="ts">
  interface Option {
    value: string;
    label: string;
  }

  interface Props {
    options: Option[];
    value: string;
    onchange?: (_value: string) => void;
  }

  const { options, value, onchange }: Props = $props();

  function handleKeydown(e: KeyboardEvent) {
    const btns = Array.from(
      (e.currentTarget as HTMLElement).querySelectorAll<HTMLButtonElement>('[role="tab"]'),
    );
    const idx = btns.indexOf(document.activeElement as HTMLButtonElement);
    if (idx === -1) return;
    let next = -1;
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') next = (idx + 1) % btns.length;
    else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') next = (idx - 1 + btns.length) % btns.length;
    else if (e.key === 'Home') next = 0;
    else if (e.key === 'End') next = btns.length - 1;
    if (next !== -1) {
      e.preventDefault();
      btns[next].focus();
      onchange?.(options[next].value);
    }
  }
</script>

<!-- svelte-ignore a11y_interactive_supports_focus -->
<div class="segmented" role="tablist" onkeydown={handleKeydown}>
  {#each options as opt (opt.value)}
    <button
      class="seg-btn"
      class:seg-btn--active={value === opt.value}
      role="tab"
      aria-selected={value === opt.value}
      tabindex={value === opt.value ? 0 : -1}
      onclick={() => value !== opt.value && onchange?.(opt.value)}>{opt.label}</button
    >
  {/each}
</div>

<style>
  .segmented {
    display: flex;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    overflow: hidden;
  }

  .seg-btn {
    padding: 2px var(--spacing-2);
    background: transparent;
    border: none;
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    color: var(--color-text-muted);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .seg-btn + .seg-btn {
    border-left: 1px solid var(--color-border);
  }

  .seg-btn--active {
    background: var(--color-accent-subtle);
    color: var(--color-accent);
    font-weight: var(--font-weight-medium);
  }
</style>
