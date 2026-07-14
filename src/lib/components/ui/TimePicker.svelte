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
  import SmallChevronIcon from '$lib/components/icons/SmallChevronIcon.svelte';

  const [initH, initM, initS] = untrack(() => parseTime(value));
  let h = $state(initH);
  let m = $state(initM);
  let s = $state(initS);

  function pad(n: number): string {
    return String(n).padStart(2, '0');
  }

  // Raw display strings — only padded on blur/arrow-key, not during typing
  let hRaw = $state(pad(initH));
  let mRaw = $state(pad(initM));
  let sRaw = $state(pad(initS));

  let activeField = $state<'h' | 'm' | 's' | null>(null);

  let hEl = $state<HTMLInputElement | null>(null);
  let mEl = $state<HTMLInputElement | null>(null);
  let sEl = $state<HTMLInputElement | null>(null);

  $effect(() => {
    const [ph, pm, ps] = parseTime(value);
    untrack(() => {
      h = ph;
      m = pm;
      s = ps;
      if (activeField !== 'h') hRaw = pad(ph);
      if (activeField !== 'm') mRaw = pad(pm);
      if (activeField !== 's') sRaw = pad(ps);
    });
  });

  function emit(): void {
    onchange(`${pad(h)}:${pad(m)}:${pad(s)}`);
  }

  function commitField(field: 'h' | 'm' | 's'): void {
    if (field === 'h') {
      const n = parseInt(hRaw, 10);
      h = isNaN(n) ? 0 : Math.min(23, Math.max(0, n));
      hRaw = pad(h);
    } else if (field === 'm') {
      const n = parseInt(mRaw, 10);
      m = isNaN(n) ? 0 : Math.min(59, Math.max(0, n));
      mRaw = pad(m);
    } else {
      const n = parseInt(sRaw, 10);
      s = isNaN(n) ? 0 : Math.min(59, Math.max(0, n));
      sRaw = pad(s);
    }
    emit();
  }

  function handleFocus(field: 'h' | 'm' | 's', e: FocusEvent): void {
    activeField = field;
    const input = e.target as HTMLInputElement;
    // Defer past mouseup so click doesn't deselect
    setTimeout(() => input.select(), 0);
  }

  function handleBlur(field: 'h' | 'm' | 's'): void {
    activeField = null;
    commitField(field);
  }

  function handleInput(field: 'h' | 'm' | 's', e: Event): void {
    const input = e.target as HTMLInputElement;
    const filtered = input.value.replace(/\D/g, '').slice(0, 2);
    if (field === 'h') hRaw = filtered;
    else if (field === 'm') mRaw = filtered;
    else sRaw = filtered;

    // Auto-commit and advance to next field when 2 digits are entered
    if (filtered.length === 2) {
      commitField(field);
      if (field === 'h') mEl?.focus();
      else if (field === 'm') sEl?.focus();
    }
  }

  function adjustH(delta: number): void {
    h = (((h + delta) % 24) + 24) % 24;
    hRaw = pad(h);
    emit();
  }
  function adjustM(delta: number): void {
    m = (((m + delta) % 60) + 60) % 60;
    mRaw = pad(m);
    emit();
  }
  function adjustS(delta: number): void {
    s = (((s + delta) % 60) + 60) % 60;
    sRaw = pad(s);
    emit();
  }

  function handleKeydown(field: 'h' | 'm' | 's', e: KeyboardEvent): void {
    if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (field === 'h') adjustH(1);
      else if (field === 'm') adjustM(1);
      else adjustS(1);
    }
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (field === 'h') adjustH(-1);
      else if (field === 'm') adjustM(-1);
      else adjustS(-1);
    }
    if (e.key === 'Tab' && !e.shiftKey) {
      if (field === 'h') {
        e.preventDefault();
        e.stopPropagation();
        commitField('h');
        mEl?.focus();
      } else if (field === 'm') {
        e.preventDefault();
        e.stopPropagation();
        commitField('m');
        sEl?.focus();
      }
      // s: let default tab behaviour move focus out
    }
    if (e.key === 'Tab' && e.shiftKey) {
      if (field === 's') {
        e.preventDefault();
        e.stopPropagation();
        commitField('s');
        mEl?.focus();
      } else if (field === 'm') {
        e.preventDefault();
        e.stopPropagation();
        commitField('m');
        hEl?.focus();
      }
      // h: let default shift-tab behaviour move focus out
    }
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      commitField(field);
      (e.target as HTMLInputElement).blur();
    }
  }
</script>

<div class="time-picker">
  <div class="tp-col">
    <button class="tp-arrow" onclick={() => adjustH(1)} aria-label="Increment hours" tabindex="-1">
      <SmallChevronIcon direction="up" />
    </button>
    <input
      class="tp-input"
      type="text"
      inputmode="numeric"
      bind:this={hEl}
      bind:value={hRaw}
      maxlength="2"
      oninput={(e) => handleInput('h', e)}
      onkeydown={(e) => handleKeydown('h', e)}
      onfocus={(e) => handleFocus('h', e)}
      onblur={() => handleBlur('h')}
      aria-label="Hours"
    />
    <button class="tp-arrow" onclick={() => adjustH(-1)} aria-label="Decrement hours" tabindex="-1">
      <SmallChevronIcon direction="down" />
    </button>
    <span class="tp-label">HH</span>
  </div>

  <span class="tp-colon">:</span>

  <div class="tp-col">
    <button
      class="tp-arrow"
      onclick={() => adjustM(1)}
      aria-label="Increment minutes"
      tabindex="-1"
    >
      <SmallChevronIcon direction="up" />
    </button>
    <input
      class="tp-input"
      type="text"
      inputmode="numeric"
      bind:this={mEl}
      bind:value={mRaw}
      maxlength="2"
      oninput={(e) => handleInput('m', e)}
      onkeydown={(e) => handleKeydown('m', e)}
      onfocus={(e) => handleFocus('m', e)}
      onblur={() => handleBlur('m')}
      aria-label="Minutes"
    />
    <button
      class="tp-arrow"
      onclick={() => adjustM(-1)}
      aria-label="Decrement minutes"
      tabindex="-1"
    >
      <SmallChevronIcon direction="down" />
    </button>
    <span class="tp-label">MM</span>
  </div>

  <span class="tp-colon">:</span>

  <div class="tp-col">
    <button
      class="tp-arrow"
      onclick={() => adjustS(1)}
      aria-label="Increment seconds"
      tabindex="-1"
    >
      <SmallChevronIcon direction="up" />
    </button>
    <input
      class="tp-input"
      type="text"
      inputmode="numeric"
      bind:this={sEl}
      bind:value={sRaw}
      maxlength="2"
      oninput={(e) => handleInput('s', e)}
      onkeydown={(e) => handleKeydown('s', e)}
      onfocus={(e) => handleFocus('s', e)}
      onblur={() => handleBlur('s')}
      aria-label="Seconds"
    />
    <button
      class="tp-arrow"
      onclick={() => adjustS(-1)}
      aria-label="Decrement seconds"
      tabindex="-1"
    >
      <SmallChevronIcon direction="down" />
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
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
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
    transition:
      border-color var(--transition-fast),
      box-shadow var(--transition-fast);
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
