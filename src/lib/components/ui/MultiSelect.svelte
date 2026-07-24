<!--
  MultiSelect — polished custom multi-select dropdown with checkboxes.
  Same portal-positioning/keyboard pattern as Select.svelte, but supports
  selecting zero or more options and doesn't close on selection.
-->
<script lang="ts">
  import { portal } from '$lib/actions/portal';
  import Checkbox from '$lib/components/ui/Checkbox.svelte';
  import SmallChevronIcon from '$lib/components/icons/SmallChevronIcon.svelte';

  type Option = { value: string; label: string };

  interface Props {
    values?: string[];
    options: Option[];
    id?: string;
    'aria-label'?: string;
    disabled?: boolean;
    size?: 'xs' | 'sm' | 'md';
    mono?: boolean;
    searchable?: boolean;
    placeholder?: string;
    onchange?: (_values: string[]) => void;
    class?: string;
    style?: string;
  }

  let {
    values = $bindable([]),
    options,
    id,
    'aria-label': ariaLabel,
    disabled = false,
    size = 'sm',
    mono = false,
    searchable = false,
    placeholder = 'All',
    onchange,
    class: className = '',
    style = '',
  }: Props = $props();

  const listboxId = `multiselect-listbox-${Math.random().toString(36).slice(2)}`;

  let open = $state(false);
  let triggerEl = $state<HTMLButtonElement | null>(null);
  let dropdownEl = $state<HTMLDivElement | null>(null);
  let searchInputEl = $state<HTMLInputElement | null>(null);
  let searchQuery = $state('');
  let focusedIndex = $state(-1);
  let dropTop = $state(0);
  let dropLeft = $state(0);
  let dropWidth = $state(0);
  let openUp = $state(false);

  const triggerLabel = $derived.by(() => {
    if (values.length === 0) return placeholder;
    if (values.length === 1) {
      return options.find((o) => o.value === values[0])?.label ?? placeholder;
    }
    return `${values.length} selected`;
  });

  const filteredOptions = $derived(
    searchQuery.trim()
      ? options.filter((o) => o.label.toLowerCase().includes(searchQuery.trim().toLowerCase()))
      : options,
  );

  function positionDropdown() {
    if (!triggerEl || !dropdownEl) return;
    const rect = triggerEl.getBoundingClientRect();
    const dropH = Math.min(dropdownEl.scrollHeight, 260);
    const spaceBelow = window.innerHeight - rect.bottom - 6;
    const spaceAbove = rect.top - 6;

    openUp = spaceBelow < dropH && spaceAbove > spaceBelow;
    dropTop = openUp ? rect.top - dropH - 4 : rect.bottom + 4;
    dropLeft = rect.left;
    dropWidth = rect.width;
  }

  function toggle() {
    if (disabled) return;
    if (open) {
      close();
      return;
    }
    open = true;
    focusedIndex = 0;
  }

  function close() {
    open = false;
    focusedIndex = -1;
    searchQuery = '';
    triggerEl?.focus();
  }

  function toggleOption(v: string) {
    const next = values.includes(v) ? values.filter((x) => x !== v) : [...values, v];
    values = next;
    onchange?.(next);
  }

  function clearAll() {
    values = [];
    onchange?.([]);
  }

  function handleTriggerKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' || e.key === ' ' || e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      if (!open) toggle();
      else if (e.key === 'ArrowDown') focusedIndex = (focusedIndex + 1) % filteredOptions.length;
      else if (e.key === 'ArrowUp')
        focusedIndex = (focusedIndex - 1 + filteredOptions.length) % filteredOptions.length;
    } else if (e.key === 'Escape') {
      close();
    }
  }

  function handleDropdownKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' || e.key === 'Tab') {
      e.preventDefault();
      close();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      focusedIndex = (focusedIndex + 1) % filteredOptions.length;
      scrollFocusedIntoView();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      focusedIndex = (focusedIndex - 1 + filteredOptions.length) % filteredOptions.length;
      scrollFocusedIntoView();
    } else if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      if (focusedIndex >= 0 && focusedIndex < filteredOptions.length) {
        toggleOption(filteredOptions[focusedIndex].value);
      }
    }
  }

  function handleSearchKeydown(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === 'Escape') {
      e.preventDefault();
      close();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      focusedIndex = filteredOptions.length ? (focusedIndex + 1) % filteredOptions.length : -1;
      scrollFocusedIntoView();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      focusedIndex = filteredOptions.length
        ? (focusedIndex - 1 + filteredOptions.length) % filteredOptions.length
        : -1;
      scrollFocusedIntoView();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (focusedIndex >= 0 && focusedIndex < filteredOptions.length) {
        toggleOption(filteredOptions[focusedIndex].value);
      } else if (filteredOptions.length === 1) {
        toggleOption(filteredOptions[0].value);
      }
    } else if (e.key === 'Tab') {
      e.preventDefault();
      close();
    }
  }

  function scrollFocusedIntoView() {
    if (!dropdownEl) return;
    const focused = dropdownEl.querySelector<HTMLElement>('[data-focused="true"]');
    focused?.scrollIntoView({ block: 'nearest' });
  }

  $effect(() => {
    if (!open) return;

    requestAnimationFrame(() => {
      positionDropdown();
      if (searchable) searchInputEl?.focus();
    });

    function onMousedown(e: MouseEvent) {
      const t = e.target as Node;
      if (!triggerEl?.contains(t) && !dropdownEl?.contains(t)) close();
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

<button
  bind:this={triggerEl}
  {id}
  type="button"
  role="combobox"
  aria-haspopup="listbox"
  aria-expanded={open}
  aria-controls={listboxId}
  aria-label={ariaLabel}
  {disabled}
  class="select-trigger select-trigger--{size} {className}"
  class:select-trigger--mono={mono}
  class:select-trigger--open={open}
  class:select-trigger--disabled={disabled}
  class:select-trigger--placeholder={values.length === 0}
  {style}
  onclick={toggle}
  onkeydown={handleTriggerKeydown}
>
  <span class="trigger-label">{triggerLabel}</span>
  <span class="trigger-chevron" class:trigger-chevron--open={open}><SmallChevronIcon /></span>
</button>

{#if open}
  <div
    bind:this={dropdownEl}
    id={listboxId}
    class="select-dropdown select-dropdown--{size}"
    class:select-dropdown--mono={mono}
    class:select-dropdown--up={openUp}
    style="top:{dropTop}px;left:{dropLeft}px;min-width:{dropWidth}px"
    role="listbox"
    aria-label={ariaLabel}
    aria-multiselectable="true"
    tabindex="-1"
    onkeydown={handleDropdownKeydown}
    use:portal
  >
    {#if searchable}
      <div class="search-wrap">
        <input
          bind:this={searchInputEl}
          bind:value={searchQuery}
          type="text"
          class="search-input"
          placeholder="Search…"
          autocomplete="off"
          spellcheck="false"
          oninput={() => {
            focusedIndex = 0;
          }}
          onkeydown={handleSearchKeydown}
        />
      </div>
    {/if}
    {#if values.length > 0}
      <button
        type="button"
        class="clear-all"
        onmousedown={(e) => e.preventDefault()}
        onclick={clearAll}>Clear selection</button
      >
    {/if}
    {#each filteredOptions as opt, idx (opt.value)}
      {@const checked = values.includes(opt.value)}
      <button
        type="button"
        role="option"
        aria-selected={checked}
        data-focused={idx === focusedIndex ? 'true' : 'false'}
        class="option"
        class:option--selected={checked}
        class:option--focused={idx === focusedIndex}
        onmouseenter={() => {
          focusedIndex = idx;
        }}
        onmousedown={(e) => {
          e.preventDefault();
          toggleOption(opt.value);
        }}
      >
        <Checkbox size="sm" {checked} class="option-checkbox" onchange={() => {}} />
        <span class="option-label">{opt.label}</span>
      </button>
    {/each}
    {#if searchable && searchQuery.trim() && filteredOptions.length === 0}
      <div class="search-empty">No results</div>
    {/if}
  </div>
{/if}

<style>
  /* ── Trigger ──────────────────────────────────────────────────────────────── */

  .select-trigger {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 0;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg-secondary);
    color: var(--color-text-primary);
    font-family: var(--font-family-ui);
    cursor: pointer;
    white-space: nowrap;
    transition:
      border-color var(--transition-fast),
      background var(--transition-fast),
      box-shadow var(--transition-fast);
    outline: none;
    box-sizing: border-box;
    -webkit-backdrop-filter: blur(8px);
    backdrop-filter: blur(8px);
  }

  .select-trigger--xs {
    height: 22px;
    padding: 0 22px 0 6px;
    font-size: var(--font-size-xs);
    border-radius: var(--radius-sm);
  }

  .select-trigger--sm {
    height: 28px;
    padding: 0 24px 0 8px;
    font-size: var(--font-size-sm);
  }

  .select-trigger--md {
    height: 32px;
    padding: 0 28px 0 8px;
    font-size: var(--font-size-md);
    width: 100%;
    border-radius: var(--radius-md);
  }

  .select-trigger--mono {
    font-family: var(--font-family-mono);
  }

  .select-trigger:hover:not(.select-trigger--disabled) {
    border-color: var(--color-border-strong);
    background: var(--color-bg-hover);
  }

  .select-trigger--open {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 2px var(--color-accent-subtle);
    background: var(--color-bg-hover);
  }

  .select-trigger:focus-visible {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 2px var(--color-accent-subtle);
    outline: none;
  }

  .select-trigger--disabled {
    opacity: 0.48;
    cursor: not-allowed;
  }

  .select-trigger--placeholder {
    color: var(--color-text-muted);
  }

  .trigger-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    text-align: left;
  }

  .trigger-chevron {
    position: absolute;
    right: 7px;
    top: 50%;
    transform: translateY(-50%) rotate(0deg);
    color: var(--color-text-muted);
    transition:
      transform var(--transition-md),
      color var(--transition-fast);
    pointer-events: none;
    flex-shrink: 0;
    display: flex;
    align-items: center;
  }

  .select-trigger--xs .trigger-chevron {
    right: 5px;
  }

  .select-trigger--xs .trigger-chevron :global(svg) {
    width: 8px;
    height: 5px;
  }

  .trigger-chevron--open {
    transform: translateY(-50%) rotate(180deg);
    color: var(--color-accent);
  }

  /* ── Dropdown ─────────────────────────────────────────────────────────────── */

  .select-dropdown {
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
    animation: dropdown-in var(--transition-md) both;
    transform-origin: top center;
    outline: none;
  }

  .select-dropdown--up {
    animation: dropdown-in-up var(--transition-md) both;
    transform-origin: bottom center;
  }

  @keyframes dropdown-in {
    from {
      opacity: 0;
      transform: scaleY(0.92) translateY(-4px);
    }
    to {
      opacity: 1;
      transform: scaleY(1) translateY(0);
    }
  }

  @keyframes dropdown-in-up {
    from {
      opacity: 0;
      transform: scaleY(0.92) translateY(4px);
    }
    to {
      opacity: 1;
      transform: scaleY(1) translateY(0);
    }
  }

  .select-dropdown--xs {
    font-size: var(--font-size-xs);
    border-radius: var(--radius-sm);
  }

  .select-dropdown--sm {
    font-size: var(--font-size-sm);
  }

  .select-dropdown--md {
    font-size: var(--font-size-md);
  }

  .select-dropdown--mono {
    font-family: var(--font-family-mono);
  }

  /* ── Search ───────────────────────────────────────────────────────────────── */

  .search-wrap {
    position: sticky;
    top: 0;
    padding: 3px 3px 4px;
    background: var(--color-bg-overlay);
    border-bottom: 1px solid var(--color-border);
    margin-bottom: 3px;
  }

  .search-input {
    width: 100%;
    border: none;
    background: transparent;
    color: var(--color-text-primary);
    font-family: var(--font-family-ui);
    font-size: inherit;
    outline: none;
    padding: 1px 4px;
    box-sizing: border-box;
  }

  .search-input::placeholder {
    color: var(--color-text-muted);
  }

  .search-empty {
    padding: 6px 10px;
    color: var(--color-text-muted);
    font-size: inherit;
    -webkit-user-select: none;
    user-select: none;
  }

  .clear-all {
    display: block;
    width: 100%;
    padding: 5px 8px;
    border: none;
    border-bottom: 1px solid var(--color-border);
    border-radius: 0;
    margin-bottom: 2px;
    background: transparent;
    color: var(--color-accent);
    font-family: var(--font-family-ui);
    font-size: inherit;
    text-align: left;
    cursor: pointer;
  }

  .clear-all:hover {
    background: var(--color-accent-subtle);
  }

  /* ── Option ───────────────────────────────────────────────────────────────── */

  .option {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 0 8px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text-primary);
    font-family: inherit;
    font-size: inherit;
    text-align: left;
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
    white-space: nowrap;
    min-height: 26px;
  }

  .select-dropdown--xs .option {
    min-height: 22px;
    padding: 0 6px;
    gap: 5px;
  }

  .select-dropdown--md .option {
    min-height: 30px;
  }

  .option--focused,
  .option:hover {
    background: var(--color-accent-subtle);
  }

  .option--selected {
    color: var(--color-accent);
    font-weight: var(--font-weight-medium);
  }

  .option :global(.option-checkbox) {
    pointer-events: none;
  }

  .option-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
