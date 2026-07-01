<!--
  Select — polished custom dropdown replacing native <select> elements.
  Supports grouped options, three size variants, keyboard navigation, and portal positioning.
-->
<script lang="ts">
  import { portal } from '$lib/actions/portal';

  type Option = { value: string; label: string };
  type OptionGroup = { group: string; options: Option[] };
  type SelectOption = Option | OptionGroup;

  function isGroup(o: SelectOption): o is OptionGroup {
    return 'group' in o;
  }

  function flatOptions(opts: SelectOption[]): Option[] {
    return opts.flatMap((o) => (isGroup(o) ? o.options : [o]));
  }

  interface Props {
    value?: string;
    options: SelectOption[];
    id?: string;
    'aria-label'?: string;
    disabled?: boolean;
    size?: 'xs' | 'sm' | 'md';
    mono?: boolean;
    onchange?: (_value: string) => void;
    class?: string;
    style?: string;
  }

  let {
    value = $bindable(undefined),
    options,
    id,
    'aria-label': ariaLabel,
    disabled = false,
    size = 'sm',
    mono = false,
    onchange,
    class: className = '',
    style = '',
  }: Props = $props();

  const listboxId = `select-listbox-${Math.random().toString(36).slice(2)}`;

  let open = $state(false);
  let triggerEl = $state<HTMLButtonElement | null>(null);
  let dropdownEl = $state<HTMLDivElement | null>(null);
  let focusedIndex = $state(-1);
  let dropTop = $state(0);
  let dropLeft = $state(0);
  let dropWidth = $state(0);
  let openUp = $state(false);

  const flat = $derived(flatOptions(options));
  const selectedLabel = $derived(flat.find((o) => o.value === value)?.label ?? '');

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
    focusedIndex = Math.max(
      0,
      flat.findIndex((o) => o.value === value),
    );
  }

  function close() {
    open = false;
    focusedIndex = -1;
    triggerEl?.focus();
  }

  function selectOption(v: string) {
    value = v;
    onchange?.(v);
    close();
  }

  function handleTriggerKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' || e.key === ' ' || e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      if (!open) toggle();
      else if (e.key === 'ArrowDown') focusedIndex = (focusedIndex + 1) % flat.length;
      else if (e.key === 'ArrowUp') focusedIndex = (focusedIndex - 1 + flat.length) % flat.length;
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
      focusedIndex = (focusedIndex + 1) % flat.length;
      scrollFocusedIntoView();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      focusedIndex = (focusedIndex - 1 + flat.length) % flat.length;
      scrollFocusedIntoView();
    } else if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      if (focusedIndex >= 0 && focusedIndex < flat.length) {
        selectOption(flat[focusedIndex].value);
      }
    }
  }

  function scrollFocusedIntoView() {
    if (!dropdownEl) return;
    const focused = dropdownEl.querySelector<HTMLElement>('[data-focused="true"]');
    focused?.scrollIntoView({ block: 'nearest' });
  }

  $effect(() => {
    if (!open) return;

    requestAnimationFrame(positionDropdown);

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
  {style}
  onclick={toggle}
  onkeydown={handleTriggerKeydown}
>
  <span class="trigger-label">{selectedLabel}</span>
  <svg
    class="trigger-chevron"
    class:trigger-chevron--open={open}
    width="10"
    height="6"
    viewBox="0 0 10 6"
    fill="none"
    aria-hidden="true"
  >
    <path
      d="M1 1l4 4 4-4"
      stroke="currentColor"
      stroke-width="1.5"
      stroke-linecap="round"
      stroke-linejoin="round"
    />
  </svg>
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
    tabindex="-1"
    onkeydown={handleDropdownKeydown}
    use:portal
  >
    {#each options as item}
      {#if isGroup(item)}
        <div class="option-group-label">{item.group}</div>
        {#each item.options as opt}
          {@const idx = flat.indexOf(opt)}
          <button
            type="button"
            role="option"
            aria-selected={opt.value === value}
            data-focused={idx === focusedIndex ? 'true' : 'false'}
            class="option"
            class:option--selected={opt.value === value}
            class:option--focused={idx === focusedIndex}
            onmouseenter={() => {
              focusedIndex = idx;
            }}
            onmousedown={(e) => {
              e.preventDefault();
              selectOption(opt.value);
            }}
          >
            <span class="option-indicator" class:option-indicator--visible={opt.value === value}
            ></span>
            <span class="option-label">{opt.label}</span>
          </button>
        {/each}
      {:else}
        {@const idx = flat.indexOf(item)}
        <button
          type="button"
          role="option"
          aria-selected={item.value === value}
          data-focused={idx === focusedIndex ? 'true' : 'false'}
          class="option"
          class:option--selected={item.value === value}
          class:option--focused={idx === focusedIndex}
          onmouseenter={() => {
            focusedIndex = idx;
          }}
          onmousedown={(e) => {
            e.preventDefault();
            selectOption(item.value);
          }}
        >
          <span class="option-indicator" class:option-indicator--visible={item.value === value}
          ></span>
          <span class="option-label">{item.label}</span>
        </button>
      {/if}
    {/each}
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
  }

  .select-trigger--xs .trigger-chevron {
    right: 5px;
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

  /* ── Option group label ───────────────────────────────────────────────────── */

  .option-group-label {
    padding: 5px 10px 3px;
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    -webkit-user-select: none;
    user-select: none;
    font-family: var(--font-family-ui);
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
    color: var(--color-text-primary);
  }

  .option--selected {
    color: var(--color-accent);
    font-weight: var(--font-weight-medium);
  }

  /* ── Option indicator dot ─────────────────────────────────────────────────── */

  .option-indicator {
    flex-shrink: 0;
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--color-accent);
    opacity: 0;
    transition: opacity var(--transition-fast);
  }

  .option-indicator--visible {
    opacity: 1;
  }

  .option-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
