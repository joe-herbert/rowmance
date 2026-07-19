<!--
  Checkbox — polished custom checkbox replacing native input[type="checkbox"].
  Supports two size variants and full keyboard/accessibility support.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import CheckboxCheckIcon from '$lib/components/icons/CheckboxCheckIcon.svelte';

  interface Props {
    checked?: boolean;
    id?: string;
    'aria-label'?: string;
    disabled?: boolean;
    size?: 'sm' | 'md';
    onchange?: (_checked: boolean) => void;
    class?: string;
  }

  let {
    checked = $bindable(false),
    id,
    'aria-label': ariaLabel,
    disabled = false,
    size = 'md',
    onchange,
    class: className = '',
  }: Props = $props();

  const internalId = `checkbox-${Math.random().toString(36).slice(2)}`;
  const inputId = untrack(() => id ?? internalId);

  function handleChange(e: Event) {
    const c = (e.currentTarget as HTMLInputElement).checked;
    checked = c;
    onchange?.(c);
  }
</script>

<label
  class="checkbox-wrapper checkbox-wrapper--{size} {className}"
  class:checkbox-wrapper--disabled={disabled}
>
  <input
    id={inputId}
    type="checkbox"
    class="checkbox-input"
    {checked}
    {disabled}
    aria-label={ariaLabel}
    onchange={handleChange}
  />
  <span class="checkbox-box" class:checkbox-box--checked={checked} aria-hidden="true">
    <CheckboxCheckIcon class="checkbox-check" />
  </span>
</label>

<style>
  .checkbox-wrapper {
    display: inline-flex;
    align-items: center;
    cursor: pointer;
    flex-shrink: 0;
  }

  .checkbox-wrapper--disabled {
    opacity: 0.48;
    cursor: not-allowed;
    pointer-events: none;
  }

  .checkbox-input {
    position: absolute;
    opacity: 0;
    width: 0;
    height: 0;
    margin: 0;
    pointer-events: none;
  }

  .checkbox-box {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    border: 1.5px solid var(--color-border-strong);
    border-radius: var(--radius-sm);
    background: var(--color-bg-secondary);
    transition:
      border-color var(--transition-fast),
      background var(--transition-fast),
      box-shadow var(--transition-fast);
    flex-shrink: 0;
  }

  .checkbox-wrapper--sm .checkbox-box {
    width: 14px;
    height: 14px;
  }

  .checkbox-wrapper:not(.checkbox-wrapper--disabled):hover
    .checkbox-box:not(.checkbox-box--checked) {
    border-color: var(--color-accent);
    background: var(--color-accent-subtle);
  }

  .checkbox-input:focus-visible + .checkbox-box {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 2px var(--color-accent-subtle);
    outline: none;
  }

  .checkbox-box--checked {
    background: var(--color-accent);
    border-color: var(--color-accent);
  }

  .checkbox-wrapper:not(.checkbox-wrapper--disabled):hover .checkbox-box--checked {
    background: var(--color-accent-hover);
    border-color: var(--color-accent-hover);
  }

  .checkbox-check {
    width: 10px;
    height: 8px;
    color: white;
    opacity: 0;
    transform: scale(0.6);
    transition:
      opacity var(--transition-fast),
      transform var(--transition-fast);
  }

  .checkbox-wrapper--sm :global(.checkbox-check) {
    width: 8px;
    height: 6px;
  }

  .checkbox-box--checked :global(.checkbox-check) {
    opacity: 1;
    transform: scale(1);
  }
</style>
