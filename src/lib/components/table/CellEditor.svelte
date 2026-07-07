<!--
  CellEditor — inline cell editor that mounts over a table cell.
  Portalled to document.body and positioned fixed in viewport coordinates.
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import { portal } from '$lib/actions/portal';
  import DatePicker from '$lib/components/ui/DatePicker.svelte';
  import TimePicker from '$lib/components/ui/TimePicker.svelte';
  import DateTimePicker from '$lib/components/ui/DateTimePicker.svelte';
  import BooleanPicker from '$lib/components/ui/BooleanPicker.svelte';
  import { executeQuery } from '$lib/tauri/query';

  type CellValue = string | number | boolean | null;

  interface Props {
    value: CellValue;
    originalValue: CellValue;
    dataType: string;
    nullable: boolean;
    initialViewportTop: number;
    initialViewportLeft: number;
    width: number;
    height: number;
    scrollEl?: HTMLElement | null;
    panelEl?: HTMLElement | null;
    onConfirm: (_newValue: CellValue) => void;
    onCancel: () => void;
    onTab?: (_shiftKey: boolean) => void;
    onTabConfirm?: (_newValue: CellValue, _shiftKey: boolean) => void;
    onOpenModal?: (_currentValue: CellValue) => void;
    connectionId?: string;
    database?: string | null;
  }

  let {
    value,
    originalValue,
    dataType,
    nullable,
    initialViewportTop,
    initialViewportLeft,
    width,
    height,
    scrollEl = null,
    panelEl = null,
    onConfirm,
    onCancel,
    onTab,
    onTabConfirm,
    onOpenModal,
    connectionId,
    database,
  }: Props = $props();

  const { settings } = useSettings();

  // ── Pure helpers (also exported for tests) ──────────────────────────────────

  export function isBooleanType(dt: string): boolean {
    const lower = dt.toLowerCase();
    return lower.includes('bool') || lower === 'tinyint(1)';
  }

  function isTinyInt1(dt: string): boolean {
    return dt.toLowerCase() === 'tinyint(1)';
  }

  export function isDateType(dt: string): boolean {
    return dt.toLowerCase().includes('date');
  }

  export function isDateTimeType(dt: string): boolean {
    const lower = dt.toLowerCase();
    return (lower.includes('date') && lower.includes('time')) || lower.includes('timestamp');
  }

  export function isTimeType(dt: string): boolean {
    const lower = dt.toLowerCase();
    return lower.includes('time') && !lower.includes('date') && !lower.includes('timestamp');
  }

  export function getInputType(
    dt: string,
  ): 'boolean' | 'datetime-local' | 'date' | 'time' | 'text' {
    if (isBooleanType(dt)) return 'boolean';
    if (isDateTimeType(dt)) return 'datetime-local';
    if (isDateType(dt)) return 'date';
    if (isTimeType(dt)) return 'time';
    return 'text';
  }

  // ── Local state ─────────────────────────────────────────────────────────────

  const inputType = $derived(getInputType(dataType));

  // For boolean: null → true → false → null; also accept 0/1 from tinyint(1)
  function toBoolState(v: typeof value): boolean | null {
    if (v === true || v === 1) return true;
    if (v === false || v === 0) return false;
    return null;
  }

  let boolState = $state<boolean | null>(untrack(() => toBoolState(value)));

  // For text/date: string representation
  let textValue = $state<string>(untrack(() => (value === null ? '' : String(value))));

  let inputEl = $state<HTMLInputElement | null>(null);
  let cellEditorEl = $state<HTMLDivElement | null>(null);
  let actionsEl = $state<HTMLDivElement | null>(null);
  let pickerEl = $state<HTMLDivElement | null>(null);

  const showPicker = $derived(
    inputType === 'date' ||
      inputType === 'datetime-local' ||
      inputType === 'time' ||
      inputType === 'boolean',
  );

  // Cell editor fixed viewport position, updated on scroll to follow the cell
  let currentTop = $state(untrack(() => initialViewportTop));
  let currentLeft = $state(untrack(() => initialViewportLeft));

  // Off-screen until first position calculation runs
  let pickerTop = $state(-9999);
  let pickerLeft = $state(-9999);
  let pickerOpenUp = $state(false);
  let actionsTopFixed = $state(-9999);
  let actionsLeftFixed = $state(-9999);

  // Fallback estimates used only to decide open-up vs open-down before first render
  const PICKER_HEIGHT_ESTIMATE: Record<string, number> = {
    date: 300,
    time: 150,
    'datetime-local': 450,
    boolean: 130,
  };

  // Height of the actions bar — used to decide above/below placement
  const ACTIONS_HEIGHT = 28;
  const ACTIONS_GAP = 3;

  $effect(() => {
    if (!cellEditorEl) return;

    // Snapshot scroll element and initial offsets as plain locals so the closure
    // below doesn't need to go through Svelte's reactive prop machinery.
    const snapScrollEl = scrollEl;
    const initScrollTop = snapScrollEl?.scrollTop ?? 0;
    const initScrollLeft = snapScrollEl?.scrollLeft ?? 0;

    function updatePositions(): void {
      if (!cellEditorEl) return;

      const rawTop = initialViewportTop - ((snapScrollEl?.scrollTop ?? 0) - initScrollTop);
      const rawLeft = initialViewportLeft - ((snapScrollEl?.scrollLeft ?? 0) - initScrollLeft);

      // Clamp the cell editor to the panel bounds so the group stays visible.
      const panel = panelEl?.getBoundingClientRect();
      const clampedTop = panel
        ? Math.max(panel.top, Math.min(panel.bottom - height, rawTop))
        : rawTop;
      const clampedLeft = panel
        ? Math.max(panel.left, Math.min(panel.right - width, rawLeft))
        : rawLeft;

      // Apply directly to the DOM so getBoundingClientRect() below is accurate.
      cellEditorEl.style.top = `${clampedTop}px`;
      cellEditorEl.style.left = `${clampedLeft}px`;
      currentTop = clampedTop;
      currentLeft = clampedLeft;

      const rect = cellEditorEl.getBoundingClientRect();
      const cellCenterX = rect.left + width / 2;

      const panelBottom = panel?.bottom ?? window.innerHeight;
      const panelLeft = panel?.left ?? 0;
      const panelRight = panel?.right ?? window.innerWidth;

      // Actions bar: above cell when no room below within the panel
      const actionsAbove = rect.bottom + ACTIONS_GAP + ACTIONS_HEIGHT > panelBottom - 4;
      actionsTopFixed = actionsAbove
        ? rect.top - ACTIONS_GAP - ACTIONS_HEIGHT
        : rect.bottom + ACTIONS_GAP;
      const actionsW = actionsEl?.offsetWidth ?? 100;
      actionsLeftFixed = Math.max(
        panelLeft + 4,
        Math.min(panelRight - actionsW - 4, cellCenterX - actionsW / 2),
      );

      // Picker
      if (showPicker) {
        const actualH = pickerEl
          ? pickerEl.offsetHeight
          : (PICKER_HEIGHT_ESTIMATE[inputType] ?? 300);
        const estimateH = PICKER_HEIGHT_ESTIMATE[inputType] ?? 300;
        const belowActionsY = actionsAbove
          ? rect.bottom + ACTIONS_GAP + 4
          : actionsTopFixed + ACTIONS_HEIGHT + 4;
        const aboveActionsY = actionsAbove ? actionsTopFixed - 4 : rect.top - 4;
        const spaceBelow = panelBottom - belowActionsY - 4;
        pickerOpenUp = spaceBelow < estimateH && aboveActionsY > estimateH;
        pickerTop = pickerOpenUp ? aboveActionsY - actualH : belowActionsY;
        const pickerW = pickerEl ? pickerEl.offsetWidth : 240;
        pickerLeft = Math.max(
          panelLeft + 4,
          Math.min(panelRight - pickerW - 4, cellCenterX - pickerW / 2),
        );
      }
    }

    // Two rAF passes: first positions roughly, second uses actual rendered dimensions
    requestAnimationFrame(() => {
      updatePositions();
      requestAnimationFrame(updatePositions);
    });
    // Listen directly on the table scroll element for table scroll, and on the
    // window (capture) for any other ancestor scroll or resize.
    snapScrollEl?.addEventListener('scroll', updatePositions);
    window.addEventListener('scroll', updatePositions, true);
    window.addEventListener('resize', updatePositions);
    return () => {
      snapScrollEl?.removeEventListener('scroll', updatePositions);
      window.removeEventListener('scroll', updatePositions, true);
      window.removeEventListener('resize', updatePositions);
    };
  });

  onMount(() => {
    if (inputEl) {
      inputEl.focus();
      inputEl.select();
    } else {
      requestAnimationFrame(() => actionsEl?.focus());
    }

    function handlePointerDown(e: PointerEvent): void {
      const target = e.target as Node;
      if (
        (cellEditorEl && cellEditorEl.contains(target)) ||
        (actionsEl && actionsEl.contains(target)) ||
        (pickerEl && pickerEl.contains(target))
      )
        return;
      if (settings.clickOutsideEdit === 'confirm') {
        confirmEdit();
      } else {
        onCancel();
      }
    }

    document.addEventListener('pointerdown', handlePointerDown);
    return () => document.removeEventListener('pointerdown', handlePointerDown);
  });

  function boolToDbValue(v: boolean | null): boolean | number | null {
    if (v === null) return null;
    return isTinyInt1(dataType) ? (v ? 1 : 0) : v;
  }

  function getConfirmedValue(): CellValue {
    if (inputType === 'boolean') return boolToDbValue(boolState);
    if (textValue === '') return '';
    if (inputType === 'text') {
      const asNum = Number(textValue);
      if (typeof originalValue === 'number' && !isNaN(asNum) && textValue.trim() !== '')
        return asNum;
      return textValue;
    }
    return textValue;
  }

  function confirmEdit(): void {
    onConfirm(getConfirmedValue());
  }

  const boolCycleOrder: (boolean | null)[] = [true, false, null];

  function cycleBool(dir: 1 | -1): void {
    const idx = boolCycleOrder.indexOf(boolState);
    boolState = boolCycleOrder[(idx + dir + boolCycleOrder.length) % boolCycleOrder.length];
  }

  export function cycle(dir: 1 | -1): void {
    if (inputType === 'boolean') cycleBool(dir);
  }

  function handleKeydown(e: KeyboardEvent): void {
    if (e.key === 'Enter') {
      e.preventDefault();
      confirmEdit();
    } else if (e.key === 'Tab') {
      e.preventDefault();
      if (onTabConfirm) {
        onTabConfirm(getConfirmedValue(), e.shiftKey);
      } else {
        confirmEdit();
        onTab?.(e.shiftKey);
      }
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onCancel();
    } else if (inputType === 'boolean' && e.key === 'ArrowDown') {
      e.preventDefault();
      e.stopPropagation();
      cycleBool(1);
    } else if (inputType === 'boolean' && e.key === 'ArrowUp') {
      e.preventDefault();
      e.stopPropagation();
      cycleBool(-1);
    }
  }

  function boolLabel(v: boolean | null): string {
    if (v === null) return 'NULL';
    const fmt = settings.booleanDisplay ?? 'tick-cross';
    if (fmt === 'true-false') return v ? 'True' : 'False';
    if (fmt === '1-0') return v ? '1' : '0';
    if (fmt === 'as-saved') return String(v);
    return v ? '✓' : '✗';
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

<svelte:document
  onkeydown={(e) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onCancel();
    }
    if (inputType === 'boolean') {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        cycleBool(1);
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        cycleBool(-1);
      }
    }
  }}
/>

<div
  bind:this={cellEditorEl}
  class="cell-editor"
  style="top: {currentTop}px; left: {currentLeft}px; width: {width}px; height: {height}px;"
  role="dialog"
  aria-label="Edit cell"
  tabindex="-1"
  onkeydown={handleKeydown}
  use:portal
>
  {#if inputType === 'boolean'}
    <span
      class="bool-toggle"
      class:bool-true={boolState === true}
      class:bool-false={boolState === false}
      class:bool-null={boolState === null}
      aria-label="Boolean value: {boolLabel(boolState)}"
    >
      {boolLabel(boolState)}
    </span>
  {:else if inputType === 'datetime-local'}
    <input
      bind:this={inputEl}
      class="editor-input"
      type="text"
      bind:value={textValue}
      autocomplete="off"
      autocorrect="off"
      autocapitalize="off"
      spellcheck="false"
      aria-label="Edit datetime value"
    />
  {:else if inputType === 'date'}
    <input
      bind:this={inputEl}
      class="editor-input"
      type="text"
      bind:value={textValue}
      autocomplete="off"
      autocorrect="off"
      autocapitalize="off"
      spellcheck="false"
      aria-label="Edit date value"
    />
  {:else if inputType === 'time'}
    <input
      bind:this={inputEl}
      class="editor-input"
      type="text"
      bind:value={textValue}
      autocomplete="off"
      autocorrect="off"
      autocapitalize="off"
      spellcheck="false"
      aria-label="Edit time value"
    />
  {:else}
    <input
      bind:this={inputEl}
      class="editor-input"
      type="text"
      bind:value={textValue}
      autocomplete="off"
      autocorrect="off"
      autocapitalize="off"
      spellcheck="false"
      aria-label="Edit cell value"
    />
  {/if}
</div>

<div
  bind:this={actionsEl}
  class="editor-actions"
  style="top:{actionsTopFixed}px; left:{actionsLeftFixed}px;"
  role="toolbar"
  tabindex="0"
  aria-label="Edit actions"
  onkeydown={handleKeydown}
  use:portal
>
  <button class="action-btn confirm-btn" onclick={confirmEdit} title="Confirm (Enter)"
    ><svg
      width="12"
      height="12"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2.5"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"><polyline points="20 6 9 17 4 12" /></svg
    ></button
  >
  {#if onOpenModal}
    <button
      class="action-btn open-modal-btn"
      onclick={() => onOpenModal!(getConfirmedValue())}
      title="Open in modal"
      aria-label="Open in modal"
      ><svg
        width="12"
        height="12"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
        ><polyline points="15 3 21 3 21 9" /><polyline points="9 21 3 21 3 15" /><line
          x1="21"
          y1="3"
          x2="14"
          y2="10"
        /><line x1="3" y1="21" x2="10" y2="14" /></svg
      ></button
    >
  {/if}
  {#if inputType !== 'boolean' && showPicker}
    <button
      class="action-btn now-btn"
      onclick={handleNow}
      title="Set to current {settings.nowTimeSource === 'database' ? 'database' : 'local'} time"
      aria-label="Set to now">NOW</button
    >
  {/if}
  {#if nullable}
    <button
      class="action-btn null-btn"
      onclick={() => onConfirm(null)}
      title="Set to NULL"
      aria-label="Set to NULL"
    >
      NULL
    </button>
  {/if}
  <button class="action-btn cancel-btn" onclick={onCancel} title="Cancel (Escape)"
    ><svg
      width="12"
      height="12"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2.5"
      stroke-linecap="round"
      aria-hidden="true"
      ><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg
    ></button
  >
</div>

{#if showPicker}
  <div
    bind:this={pickerEl}
    class="picker-popup"
    class:picker-popup--up={pickerOpenUp}
    style="top:{pickerTop}px; left:{pickerLeft}px;"
    role="dialog"
    tabindex="-1"
    aria-label="Pick {inputType === 'date'
      ? 'date'
      : inputType === 'time'
        ? 'time'
        : 'date and time'}"
    use:portal
    onkeydown={handleKeydown}
  >
    {#if inputType === 'boolean'}
      <BooleanPicker
        value={boolState}
        displayFormat={settings.booleanDisplay ?? 'tick-cross'}
        {nullable}
        onselect={(v) => {
          boolState = v;
          onConfirm(boolToDbValue(v));
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
    {:else if inputType === 'datetime-local'}
      <DateTimePicker
        value={textValue}
        onchange={(v) => {
          textValue = v;
        }}
      />
    {/if}
  </div>
{/if}

<style>
  :global(.cell-editor) {
    position: fixed;
    z-index: 9999;
    display: flex;
    align-items: stretch;
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-md);
    box-sizing: border-box;
    overflow: hidden;
  }

  .editor-input {
    flex: 1;
    padding: 0 var(--spacing-2);
    background: var(--color-bg-primary);
    border: none;
    outline: none;
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    min-width: 0;
    width: 100%;
    height: 100%;
    box-sizing: border-box;
  }

  .bool-toggle {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0 var(--spacing-3);
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    font-family: var(--font-family-mono);
    height: 100%;
    user-select: none;
  }

  .bool-toggle.bool-true {
    color: var(--color-success);
  }

  .bool-toggle.bool-false {
    color: var(--color-danger);
  }

  .bool-toggle.bool-null {
    color: var(--color-null);
    font-style: italic;
  }

  .editor-actions {
    position: fixed;
    z-index: 9999;
    display: flex;
    align-items: stretch;
    height: 28px;
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-md);
    overflow: hidden;
  }

  .action-btn {
    padding: 0 var(--spacing-2);
    background: transparent;
    border: none;
    border-left: 1px solid var(--color-border);
    font-size: var(--font-size-xs);
    cursor: pointer;
    transition: background var(--transition-fast);
    color: var(--color-text-secondary);
    font-family: var(--font-family-ui);
    height: 100%;
    white-space: nowrap;
  }

  .action-btn:first-child {
    border-left: none;
  }

  .action-btn:hover {
    background: var(--color-bg-hover);
  }

  .confirm-btn:hover {
    background: var(--color-success-subtle);
    color: var(--color-success);
  }

  .now-btn {
    font-size: 10px;
    font-family: var(--font-family-mono);
    color: var(--color-accent);
  }

  .now-btn:hover {
    background: var(--color-accent-subtle);
  }

  .null-btn {
    font-size: 10px;
    font-family: var(--font-family-mono);
    color: var(--color-null);
  }

  .null-btn:hover {
    background: var(--color-accent-subtle);
  }

  .cancel-btn:hover {
    background: var(--color-danger-subtle);
    color: var(--color-danger);
  }

  :global(.picker-popup) {
    position: fixed;
    z-index: 9999;
    background: var(--color-bg-overlay);
    -webkit-backdrop-filter: blur(20px) saturate(160%);
    backdrop-filter: blur(20px) saturate(160%);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-overlay);
    animation: picker-in var(--transition-md) both;
    transform-origin: top left;
    outline: none;
  }

  :global(.picker-popup--up) {
    animation: picker-in-up var(--transition-md) both;
    transform-origin: bottom left;
  }

  @keyframes picker-in {
    from {
      opacity: 0;
      transform: scaleY(0.94) translateY(-4px);
    }
    to {
      opacity: 1;
      transform: scaleY(1) translateY(0);
    }
  }

  @keyframes picker-in-up {
    from {
      opacity: 0;
      transform: scaleY(0.94) translateY(4px);
    }
    to {
      opacity: 1;
      transform: scaleY(1) translateY(0);
    }
  }
</style>
