<!--
  CellEditorModal — full-screen modal cell editor mounted via portal.
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import Modal from '$lib/components/Modal.svelte';
  import DatePicker from '$lib/components/ui/DatePicker.svelte';
  import TimePicker from '$lib/components/ui/TimePicker.svelte';
  import DateTimePicker from '$lib/components/ui/DateTimePicker.svelte';
  import BooleanPicker from '$lib/components/ui/BooleanPicker.svelte';
  import { executeQuery } from '$lib/tauri/query';

  function getInputType(dt: string): 'boolean' | 'datetime-local' | 'date' | 'time' | 'text' {
    const lower = dt.toLowerCase();
    if (lower.includes('bool') || lower === 'tinyint(1)') return 'boolean';
    if ((lower.includes('date') && lower.includes('time')) || lower.includes('timestamp'))
      return 'datetime-local';
    if (lower.includes('date')) return 'date';
    if (lower.includes('time') && !lower.includes('timestamp')) return 'time';
    return 'text';
  }

  type CellValue = string | number | boolean | null;

  interface Props {
    value: CellValue;
    originalValue: CellValue;
    colName: string;
    dataType: string;
    nullable: boolean;
    onConfirm: (_newValue: CellValue) => void;
    onCancel: () => void;
    connectionId?: string;
    database?: string | null;
  }

  let {
    value,
    originalValue,
    colName,
    dataType,
    nullable,
    onConfirm,
    onCancel,
    connectionId,
    database,
  }: Props = $props();

  const { settings } = useSettings();

  const inputType = $derived(getInputType(dataType));

  function toBoolState(v: typeof value): boolean | null {
    if (v === true || v === 1) return true;
    if (v === false || v === 0) return false;
    return null;
  }

  let boolState = $state<boolean | null>(untrack(() => toBoolState(value)));

  let textValue = $state<string>(untrack(() => (value === null ? '' : String(value))));
  let hasFormatted = $state(false);
  let showConfirmDropdown = $state(false);

  let inputEl = $state<HTMLInputElement | HTMLTextAreaElement | null>(null);
  let modalEl = $state<HTMLDivElement | null>(null);

  onMount(() => {
    if (inputEl) {
      inputEl.focus();
      if ('select' in inputEl) inputEl.select();
    }
  });

  $effect(() => {
    if (inputEl instanceof HTMLTextAreaElement) {
      void textValue;
      inputEl.style.height = 'auto';
      inputEl.style.height = `${inputEl.scrollHeight}px`;
    }
  });

  function isTinyInt1(): boolean {
    return dataType.toLowerCase() === 'tinyint(1)';
  }

  function boolToDbValue(v: boolean | null): boolean | number | null {
    if (v === null) return null;
    return isTinyInt1() ? (v ? 1 : 0) : v;
  }

  function confirmEdit(): void {
    if (inputType === 'boolean') {
      onConfirm(boolToDbValue(boolState));
    } else if (textValue === '') {
      onConfirm('');
    } else if (inputType === 'text') {
      const asNum = Number(textValue);
      if (typeof originalValue === 'number' && !isNaN(asNum) && textValue.trim() !== '') {
        onConfirm(asNum);
      } else {
        onConfirm(textValue);
      }
    } else {
      onConfirm(textValue);
    }
  }

  function handleKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      confirmEdit();
    }
  }

  function handleBackdropClick(): void {
    onCancel();
  }

  const showNow = $derived(
    inputType === 'date' || inputType === 'datetime-local' || inputType === 'time',
  );

  const isJsonType = $derived(
    dataType.toLowerCase() === 'json' || dataType.toLowerCase() === 'jsonb',
  );

  function contentLooksLikeJson(v: string): boolean {
    const t = v.trim();
    if (!t || (t[0] !== '{' && t[0] !== '[')) return false;
    try {
      const parsed = JSON.parse(t);
      return typeof parsed === 'object' && parsed !== null;
    } catch {
      return false;
    }
  }

  const initialLooksLikeJson = contentLooksLikeJson(value === null ? '' : String(value));

  const showFormatJson = $derived(inputType === 'text' && (isJsonType || contentLooksLikeJson(textValue)));

  const hasInvalidJson = $derived(
    (isJsonType || initialLooksLikeJson) &&
      inputType === 'text' &&
      textValue.trim() !== '' &&
      (() => {
        try {
          JSON.parse(textValue);
          return false;
        } catch {
          return true;
        }
      })(),
  );

  function formatJson(): void {
    try {
      textValue = JSON.stringify(JSON.parse(textValue), null, 2);
      hasFormatted = true;
    } catch {
      // not valid JSON, do nothing
    }
  }

  function confirmAndMinify(): void {
    try {
      textValue = JSON.stringify(JSON.parse(textValue));
    } catch {
      // not valid JSON, confirm as-is
    }
    confirmEdit();
  }

  function formatNow(d: Date, type: typeof inputType): string {
    const p = (n: number) => String(n).padStart(2, '0');
    const date = `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
    const time = `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
    if (type === 'date') return date;
    if (type === 'time') return time;
    return `${date} ${time}`;
  }

  function parseDbNow(raw: string, type: typeof inputType): string {
    const normalized = String(raw)
      .replace('T', ' ')
      .replace(/\.\d+/, '')
      .replace(/[+-]\d{2}:\d{2}$/, '')
      .trim();
    const [datePart = '', timePart = '00:00:00'] = normalized.split(' ');
    if (type === 'date') return datePart;
    if (type === 'time') return timePart;
    return `${datePart} ${timePart}`;
  }

  async function handleNow(): Promise<void> {
    if (settings.nowTimeSource === 'database' && connectionId) {
      try {
        const result = await executeQuery(connectionId, 'SELECT NOW()', 1, 1, database ?? null);
        if (!result.error && result.rows[0]?.[0] != null) {
          onConfirm(parseDbNow(String(result.rows[0][0]), inputType));
          return;
        }
      } catch {
        // fall through to user time on error
      }
    }
    onConfirm(formatNow(new Date(), inputType));
  }
</script>

<Modal zindex={500} label="Edit {colName}" onbackdropclick={handleBackdropClick}>
  <div bind:this={modalEl} class="modal-dialog" role="presentation" onkeydown={handleKeydown}>
    <header class="modal-header">
      <span class="modal-title">{colName}</span>
      <span class="modal-type">{dataType}</span>
      {#if inputType !== 'boolean'}
        <span class="modal-hint">Ctrl+Enter to confirm · Escape to cancel</span>
      {/if}
    </header>

    <div class="modal-body">
      {#if inputType === 'boolean'}
        <BooleanPicker
          value={boolState}
          displayFormat={settings.booleanDisplay ?? 'tick-cross'}
          {nullable}
          onselect={(v) => {
            boolState = v;
          }}
        />
      {:else if inputType === 'datetime-local'}
        <DateTimePicker
          value={textValue}
          onchange={(v) => {
            textValue = v;
          }}
        />
      {:else if inputType === 'date'}
        <DatePicker
          value={textValue}
          onchange={(v) => {
            textValue = v;
          }}
        />
      {:else if inputType === 'time'}
        <TimePicker
          value={textValue}
          onchange={(v) => {
            textValue = v;
          }}
        />
      {:else}
        <textarea
          bind:this={inputEl}
          class="modal-textarea"
          class:invalid-json={hasInvalidJson}
          bind:value={textValue}
          autocomplete="off"
          autocapitalize="off"
          spellcheck="false"
          aria-label="Edit cell value"
        ></textarea>
      {/if}
    </div>

    <footer class="modal-footer">
      <div class="modal-footer-left">
        {#if hasInvalidJson}
          <span class="modal-invalid-json">Invalid JSON</span>
        {/if}
      </div>
      <div class="modal-actions">
        {#if showFormatJson}
          <button class="modal-btn btn-format-json" onclick={formatJson}>Format JSON</button>
        {/if}
        {#if showNow}
          <button
            class="modal-btn btn-now"
            onclick={handleNow}
            title="Set to current {settings.nowTimeSource === 'database'
              ? 'database'
              : 'local'} time">NOW</button
          >
        {/if}
        {#if nullable}
          <button class="modal-btn btn-null" onclick={() => onConfirm(null)}>Set NULL</button>
        {/if}
        <button class="modal-btn btn-cancel" onclick={onCancel}>Cancel</button>
        {#if hasFormatted}
          <div class="split-btn">
            <button class="modal-btn btn-confirm btn-confirm-main" onclick={confirmAndMinify}
              >Confirm & Minify</button
            >
            <button
              class="modal-btn btn-confirm btn-confirm-toggle"
              onclick={() => (showConfirmDropdown = !showConfirmDropdown)}
              aria-label="More confirm options">▾</button
            >
            {#if showConfirmDropdown}
              <div
                class="split-btn-backdrop"
                role="presentation"
                onpointerdown={() => (showConfirmDropdown = false)}
              ></div>
              <div class="split-btn-dropdown">
                <button
                  class="split-btn-option"
                  onclick={() => {
                    showConfirmDropdown = false;
                    confirmEdit();
                  }}>Confirm</button
                >
              </div>
            {/if}
          </div>
        {:else}
          <button class="modal-btn btn-confirm" onclick={confirmEdit}>Confirm</button>
        {/if}
      </div>
    </footer>
  </div>
</Modal>

<style>
  .modal-dialog {
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    width: min(600px, calc(100vw - 48px));
    max-height: 90vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .modal-header {
    display: flex;
    align-items: baseline;
    gap: var(--spacing-2);
    padding: var(--spacing-3) var(--spacing-4);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .modal-title {
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
  }

  .modal-type {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-muted);
  }

  .modal-hint {
    margin-left: auto;
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  .modal-body {
    padding: var(--spacing-3) var(--spacing-4);
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    min-height: 0;
  }

  .modal-textarea {
    width: 100%;
    min-height: 160px;
    resize: none;
    overflow: hidden;
    padding: var(--spacing-2) var(--spacing-3);
    background: var(--color-bg-primary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    outline: none;
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    line-height: 1.5;
    box-sizing: border-box;
    transition: border-color var(--transition-fast);
  }

  .modal-textarea:focus {
    border-color: var(--color-accent);
  }

  .modal-textarea.invalid-json {
    border-color: var(--color-danger);
    background: var(--color-danger-subtle);
  }

  .modal-textarea.invalid-json:focus {
    border-color: var(--color-danger);
  }

  .modal-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--spacing-3);
    padding: var(--spacing-3) var(--spacing-4);
    border-top: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .modal-footer-left {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-1);
  }

  .modal-invalid-json {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-danger);
  }

  .modal-actions {
    display: flex;
    gap: var(--spacing-2);
  }

  .modal-btn {
    padding: var(--spacing-1) var(--spacing-3);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    cursor: pointer;
    border: 1px solid var(--color-border);
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
    background: transparent;
    color: var(--color-text-primary);
  }

  .modal-btn:hover {
    background: var(--color-bg-hover);
  }

  .btn-format-json {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
    color: var(--color-accent);
  }

  .btn-format-json:hover {
    background: var(--color-accent-subtle);
    border-color: var(--color-accent);
  }

  .btn-now {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
    color: var(--color-accent);
  }

  .btn-now:hover {
    background: var(--color-accent-subtle);
    border-color: var(--color-accent);
  }

  .btn-null {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
    color: var(--color-null);
  }

  .btn-null:hover {
    background: var(--color-accent-subtle);
    border-color: var(--color-accent);
  }

  .btn-cancel:hover {
    background: var(--color-danger-subtle);
    color: var(--color-danger);
    border-color: var(--color-danger);
  }

  .btn-confirm {
    background: var(--color-accent);
    color: #fff;
    border-color: var(--color-accent);
  }

  .btn-confirm:hover {
    background: color-mix(in srgb, var(--color-accent) 85%, black);
  }

  .split-btn {
    position: relative;
    display: flex;
  }

  .btn-confirm-main {
    border-radius: var(--radius-sm) 0 0 var(--radius-sm);
    border-right: none;
  }

  .btn-confirm-toggle {
    padding: var(--spacing-1) var(--spacing-2);
    border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
  }

  .split-btn-backdrop {
    position: fixed;
    inset: 0;
    z-index: 1;
  }

  .split-btn-dropdown {
    position: absolute;
    bottom: calc(100% + 4px);
    right: 0;
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-overlay);
    z-index: 2;
    min-width: 100%;
    white-space: nowrap;
  }

  .split-btn-option {
    display: block;
    width: 100%;
    padding: var(--spacing-1) var(--spacing-3);
    background: transparent;
    border: none;
    cursor: pointer;
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    color: var(--color-text-primary);
    text-align: left;
  }

  .split-btn-option:hover {
    background: var(--color-bg-hover);
  }
</style>
