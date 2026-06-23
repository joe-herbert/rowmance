<!--
  TimePicker — HH:MM:SS spinner for selecting a time value.
-->
<script lang="ts">
  interface Props {
    value: string;
    onchange: (_v: string) => void;
  }

  let { value = '00:00:00', onchange }: Props = $props();

  function parseTime(v: string): [number, number, number] {
    const m = v.match(/(\d{1,2}):(\d{2})(?::(\d{2}))?/);
    if (!m) return [0, 0, 0];
    return [+m[1], +m[2], m[3] !== undefined ? +m[3] : 0];
  }

  import { untrack } from 'svelte';

  const [initH, initM, initS] = untrack(() => parseTime(value));
  let h = $state(initH);
  let m = $state(initM);
  let s = $state(initS);

  $effect(() => {
    const [ph, pm, ps] = parseTime(value);
    untrack(() => { h = ph; m = pm; s = ps; });
  });

  function pad(n: number): string { return String(n).padStart(2, '0'); }

  function emit(): void {
    onchange(`${pad(h)}:${pad(m)}:${pad(s)}`);
  }

  function adjustH(delta: number): void { h = ((h + delta) % 24 + 24) % 24; emit(); }
  function adjustM(delta: number): void { m = ((m + delta) % 60 + 60) % 60; emit(); }
  function adjustS(delta: number): void { s = ((s + delta) % 60 + 60) % 60; emit(); }

  function handleInput(field: 'h' | 'm' | 's', raw: string): void {
    const n = parseInt(raw, 10);
    if (isNaN(n)) return;
    if (field === 'h') h = Math.min(23, Math.max(0, n));
    if (field === 'm') m = Math.min(59, Math.max(0, n));
    if (field === 's') s = Math.min(59, Math.max(0, n));
    emit();
  }

  function handleKeydown(field: 'h' | 'm' | 's', e: KeyboardEvent): void {
    if (e.key === 'ArrowUp') { e.preventDefault(); if (field === 'h') adjustH(1); else if (field === 'm') adjustM(1); else adjustS(1); }
    if (e.key === 'ArrowDown') { e.preventDefault(); if (field === 'h') adjustH(-1); else if (field === 'm') adjustM(-1); else adjustS(-1); }
  }
</script>

<div class="time-picker">
  <div class="tp-col">
    <button class="tp-arrow" onclick={() => adjustH(1)} aria-label="Increment hours" tabindex="-1">
      <svg width="10" height="6" viewBox="0 0 10 6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><polyline points="1 5 5 1 9 5"/></svg>
    </button>
    <input
      class="tp-input"
      type="text"
      inputmode="numeric"
      value={pad(h)}
      maxlength="2"
      oninput={(e) => handleInput('h', (e.target as HTMLInputElement).value)}
      onkeydown={(e) => handleKeydown('h', e)}
      onfocus={(e) => (e.target as HTMLInputElement).select()}
      aria-label="Hours"
    />
    <button class="tp-arrow" onclick={() => adjustH(-1)} aria-label="Decrement hours" tabindex="-1">
      <svg width="10" height="6" viewBox="0 0 10 6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><polyline points="1 1 5 5 9 1"/></svg>
    </button>
    <span class="tp-label">HH</span>
  </div>

  <span class="tp-colon">:</span>

  <div class="tp-col">
    <button class="tp-arrow" onclick={() => adjustM(1)} aria-label="Increment minutes" tabindex="-1">
      <svg width="10" height="6" viewBox="0 0 10 6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><polyline points="1 5 5 1 9 5"/></svg>
    </button>
    <input
      class="tp-input"
      type="text"
      inputmode="numeric"
      value={pad(m)}
      maxlength="2"
      oninput={(e) => handleInput('m', (e.target as HTMLInputElement).value)}
      onkeydown={(e) => handleKeydown('m', e)}
      onfocus={(e) => (e.target as HTMLInputElement).select()}
      aria-label="Minutes"
    />
    <button class="tp-arrow" onclick={() => adjustM(-1)} aria-label="Decrement minutes" tabindex="-1">
      <svg width="10" height="6" viewBox="0 0 10 6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><polyline points="1 1 5 5 9 1"/></svg>
    </button>
    <span class="tp-label">MM</span>
  </div>

  <span class="tp-colon">:</span>

  <div class="tp-col">
    <button class="tp-arrow" onclick={() => adjustS(1)} aria-label="Increment seconds" tabindex="-1">
      <svg width="10" height="6" viewBox="0 0 10 6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><polyline points="1 5 5 1 9 5"/></svg>
    </button>
    <input
      class="tp-input"
      type="text"
      inputmode="numeric"
      value={pad(s)}
      maxlength="2"
      oninput={(e) => handleInput('s', (e.target as HTMLInputElement).value)}
      onkeydown={(e) => handleKeydown('s', e)}
      onfocus={(e) => (e.target as HTMLInputElement).select()}
      aria-label="Seconds"
    />
    <button class="tp-arrow" onclick={() => adjustS(-1)} aria-label="Decrement seconds" tabindex="-1">
      <svg width="10" height="6" viewBox="0 0 10 6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><polyline points="1 1 5 5 9 1"/></svg>
    </button>
    <span class="tp-label">SS</span>
  </div>
</div>

<style>
  .time-picker {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: var(--spacing-3) var(--spacing-4);
  }

  .tp-col {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
  }

  .tp-arrow {
    width: 28px;
    height: 16px;
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    background: transparent;
    border-radius: var(--radius-sm);
    cursor: pointer;
    color: var(--color-text-muted);
    transition: background var(--transition-fast), color var(--transition-fast);
    padding: 0;
  }

  .tp-arrow:hover {
    background: var(--color-accent-subtle);
    color: var(--color-accent);
  }

  .tp-input {
    width: 36px;
    height: 32px;
    text-align: center;
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-primary);
    background: var(--color-bg-primary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    outline: none;
    transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
  }

  .tp-input:focus {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 2px var(--color-accent-subtle);
  }

  .tp-colon {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    margin-bottom: 20px;
    align-self: center;
  }

  .tp-label {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-family: var(--font-family-mono);
  }
</style>
