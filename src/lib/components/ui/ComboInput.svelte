<!--
  ComboInput — free-text input with a filtered suggestion dropdown.
  Styled identically to the Select component; allows values not in the suggestion list.
-->
<script lang="ts">
  import { portal } from '$lib/actions/portal';
  import SmallChevronIcon from '$lib/components/icons/SmallChevronIcon.svelte';

  interface Props {
    value?: string;
    suggestions: string[];
    id?: string;
    placeholder?: string;
    disabled?: boolean;
    onchange?: (_value: string) => void;
  }

  let {
    value = $bindable(''),
    suggestions,
    id,
    placeholder = '',
    disabled = false,
    onchange,
  }: Props = $props();

  const dropdownId = `combo-dropdown-${Math.random().toString(36).slice(2)}`;

  let open = $state(false);
  let focusedIndex = $state(-1);
  let inputEl = $state<HTMLInputElement | null>(null);
  let dropdownEl = $state<HTMLDivElement | null>(null);
  let dropTop = $state(0);
  let dropLeft = $state(0);
  let dropWidth = $state(0);
  let openUp = $state(false);

  const filtered = $derived(
    value.trim()
      ? suggestions.filter((s) => s.toLowerCase().includes(value.toLowerCase()))
      : suggestions,
  );

  function positionDropdown() {
    if (!inputEl || !dropdownEl) return;
    const rect = inputEl.getBoundingClientRect();
    const dropH = Math.min(dropdownEl.scrollHeight, 260);
    const spaceBelow = window.innerHeight - rect.bottom - 6;
    const spaceAbove = rect.top - 6;
    openUp = spaceBelow < dropH && spaceAbove > spaceBelow;
    dropTop = openUp ? rect.top - dropH - 4 : rect.bottom + 4;
    dropLeft = rect.left;
    dropWidth = rect.width;
  }

  function openDropdown() {
    if (disabled || open) return;
    open = true;
    focusedIndex = -1;
  }

  function close() {
    open = false;
    focusedIndex = -1;
  }

  function pick(val: string) {
    value = val;
    onchange?.(val);
    close();
    inputEl?.focus();
  }

  function handleInput(e: Event) {
    value = (e.currentTarget as HTMLInputElement).value;
    onchange?.(value);
    if (!open) open = true;
    focusedIndex = -1;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (!open) {
        open = true;
        return;
      }
      focusedIndex = Math.min(focusedIndex + 1, filtered.length - 1);
      scrollFocused();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      focusedIndex = Math.max(focusedIndex - 1, -1);
      scrollFocused();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (open && focusedIndex >= 0 && filtered[focusedIndex] !== undefined) {
        pick(filtered[focusedIndex]);
      }
    } else if (e.key === 'Escape') {
      close();
    } else if (e.key === 'Tab') {
      close();
    }
  }

  function scrollFocused() {
    requestAnimationFrame(() => {
      const el = dropdownEl?.querySelector<HTMLElement>('[data-focused="true"]');
      el?.scrollIntoView({ block: 'nearest' });
    });
  }

  $effect(() => {
    if (!open) return;

    requestAnimationFrame(positionDropdown);

    function onMousedown(e: MouseEvent) {
      const t = e.target as Node;
      if (!inputEl?.contains(t) && !dropdownEl?.contains(t)) close();
    }
    function onScroll() {
      positionDropdown();
    }
    function onResize() {
      positionDropdown();
    }

    document.addEventListener('mousedown', onMousedown, true);
    window.addEventListener('scroll', onScroll, true);
    window.addEventListener('resize', onResize);

    return () => {
      document.removeEventListener('mousedown', onMousedown, true);
      window.removeEventListener('scroll', onScroll, true);
      window.removeEventListener('resize', onResize);
    };
  });
</script>

<div class="combo-wrap" class:combo-wrap--open={open} class:combo-wrap--disabled={disabled}>
  <input
    bind:this={inputEl}
    {id}
    type="text"
    role="combobox"
    aria-autocomplete="list"
    aria-expanded={open}
    aria-controls={dropdownId}
    {placeholder}
    {disabled}
    {value}
    class="combo-input"
    oninput={handleInput}
    onfocus={openDropdown}
    onkeydown={handleKeydown}
  />
  <span
    class="combo-chevron{open ? ' combo-chevron--open' : ''}"
    onmousedown={(e) => {
      e.preventDefault();
      if (open) {
        close();
        inputEl?.focus();
      } else {
        inputEl?.focus();
        openDropdown();
      }
    }}
    aria-hidden="true"
  >
    <SmallChevronIcon />
  </span>
</div>

{#if open && filtered.length > 0}
  <div
    bind:this={dropdownEl}
    id={dropdownId}
    class="combo-dropdown"
    class:combo-dropdown--up={openUp}
    style="top:{dropTop}px;left:{dropLeft}px;min-width:{dropWidth}px"
    role="listbox"
    tabindex="-1"
    use:portal
  >
    {#each filtered as option, idx}
      <button
        type="button"
        role="option"
        aria-selected={option === value}
        data-focused={idx === focusedIndex ? 'true' : 'false'}
        class="combo-option"
        class:combo-option--selected={option === value}
        class:combo-option--focused={idx === focusedIndex}
        onmouseenter={() => {
          focusedIndex = idx;
        }}
        onmousedown={(e) => {
          e.preventDefault();
          pick(option);
        }}
      >
        <span
          class="combo-option-indicator"
          class:combo-option-indicator--visible={option === value}
        ></span>
        <span class="combo-option-label">{option}</span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .combo-wrap {
    position: relative;
    display: flex;
    align-items: center;
    width: 100%;
    height: 32px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg-secondary);
    box-sizing: border-box;
    transition:
      border-color var(--transition-fast),
      background var(--transition-fast),
      box-shadow var(--transition-fast);
    -webkit-backdrop-filter: blur(8px);
    backdrop-filter: blur(8px);
  }

  .combo-wrap:hover:not(.combo-wrap--disabled) {
    border-color: var(--color-border-strong);
    background: var(--color-bg-hover);
  }

  .combo-wrap--open {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 2px var(--color-accent-subtle);
    background: var(--color-bg-hover);
  }

  .combo-wrap--disabled {
    opacity: 0.48;
    cursor: not-allowed;
  }

  .combo-input {
    flex: 1;
    height: 100%;
    padding: 0 28px 0 8px;
    border: none;
    background: transparent;
    color: var(--color-text-primary);
    font-size: var(--font-size-md);
    font-family: var(--font-family-ui);
    outline: none;
    min-width: 0;
  }

  .combo-input::placeholder {
    color: var(--color-text-muted);
  }

  .combo-input:disabled {
    cursor: not-allowed;
  }

  .combo-chevron {
    position: absolute;
    right: 7px;
    top: 50%;
    transform: translateY(-50%) rotate(0deg);
    color: var(--color-text-muted);
    transition:
      transform var(--transition-md),
      color var(--transition-fast);
    pointer-events: auto;
    cursor: pointer;
    flex-shrink: 0;
  }

  .combo-chevron--open {
    transform: translateY(-50%) rotate(180deg);
    color: var(--color-accent);
  }

  /* ── Dropdown ─────────────────────────────────────────────────────────────── */

  .combo-dropdown {
    position: fixed;
    z-index: 9999;
    background: var(--color-bg-overlay);
    -webkit-backdrop-filter: blur(20px) saturate(160%);
    backdrop-filter: blur(20px) saturate(160%);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-overlay);
    overflow-y: auto;
    max-height: 260px;
    padding: 3px;
    font-size: var(--font-size-md);
    animation: combo-in var(--transition-md) both;
    transform-origin: top center;
    outline: none;
  }

  .combo-dropdown--up {
    animation: combo-in-up var(--transition-md) both;
    transform-origin: bottom center;
  }

  @keyframes combo-in {
    from {
      opacity: 0;
      transform: scaleY(0.92) translateY(-4px);
    }
    to {
      opacity: 1;
      transform: scaleY(1) translateY(0);
    }
  }

  @keyframes combo-in-up {
    from {
      opacity: 0;
      transform: scaleY(0.92) translateY(4px);
    }
    to {
      opacity: 1;
      transform: scaleY(1) translateY(0);
    }
  }

  /* ── Option ───────────────────────────────────────────────────────────────── */

  .combo-option {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    min-height: 30px;
    padding: 0 8px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text-primary);
    font-family: var(--font-family-ui);
    font-size: inherit;
    text-align: left;
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
    white-space: nowrap;
  }

  .combo-option--focused,
  .combo-option:hover {
    background: var(--color-accent-subtle);
  }

  .combo-option--selected {
    color: var(--color-accent);
    font-weight: var(--font-weight-medium);
  }

  .combo-option-indicator {
    flex-shrink: 0;
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--color-accent);
    opacity: 0;
    transition: opacity var(--transition-fast);
  }

  .combo-option-indicator--visible {
    opacity: 1;
  }

  .combo-option-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
