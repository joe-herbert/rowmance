<script lang="ts">
  interface Props {
    value: boolean | null;
    displayFormat: 'tick-cross' | 'true-false' | '1-0';
    nullable?: boolean;
    onselect: (_v: boolean | null) => void;
  }

  let { value, displayFormat, nullable = true, onselect }: Props = $props();

  function trueLabel(): string {
    if (displayFormat === 'true-false') return 'True';
    if (displayFormat === '1-0') return '1';
    return '✓';
  }

  function falseLabel(): string {
    if (displayFormat === 'true-false') return 'False';
    if (displayFormat === '1-0') return '0';
    return '✗';
  }
</script>

<div class="boolean-picker" role="listbox" aria-label="Choose boolean value">
  <button
    class="bool-option"
    class:bool-option--selected={value === true}
    class:bool-option--true={true}
    role="option"
    aria-selected={value === true}
    onclick={() => onselect(true)}
  >
    {trueLabel()}
  </button>
  <button
    class="bool-option"
    class:bool-option--selected={value === false}
    class:bool-option--false={true}
    role="option"
    aria-selected={value === false}
    onclick={() => onselect(false)}
  >
    {falseLabel()}
  </button>
  {#if nullable}
    <button
      class="bool-option"
      class:bool-option--selected={value === null}
      class:bool-option--null={true}
      role="option"
      aria-selected={value === null}
      onclick={() => onselect(null)}
    >
      NULL
    </button>
  {/if}
</div>

<style>
  .boolean-picker {
    display: flex;
    flex-direction: column;
    padding: var(--spacing-2);
    gap: var(--spacing-1);
    min-width: 120px;
  }

  .bool-option {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--spacing-2) var(--spacing-4);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-primary);
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    cursor: pointer;
    transition: background var(--transition-fast), border-color var(--transition-fast);
    color: var(--color-text-secondary);
  }

  .bool-option:hover {
    background: var(--color-bg-hover);
    border-color: var(--color-border-strong);
  }

  .bool-option--true {
    color: var(--color-success);
  }

  .bool-option--false {
    color: var(--color-danger);
  }

  .bool-option--null {
    color: var(--color-null);
    font-style: italic;
  }

  .bool-option--selected {
    background: var(--color-accent-subtle);
    border-color: var(--color-accent);
  }

  .bool-option--true.bool-option--selected {
    background: var(--color-success-subtle);
    border-color: var(--color-success);
  }

  .bool-option--false.bool-option--selected {
    background: var(--color-danger-subtle);
    border-color: var(--color-danger);
  }
</style>
