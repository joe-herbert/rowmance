<!--
  DatePicker — calendar grid for selecting a date (YYYY-MM-DD).
-->
<script lang="ts">
  interface Props {
    value: string;
    onchange: (_v: string) => void;
  }

  let { value = '', onchange }: Props = $props();

  function parseValue(v: string): { year: number; month: number; day: number } | null {
    const m = v.match(/^(\d{4})-(\d{2})-(\d{2})/);
    if (!m) return null;
    return { year: +m[1], month: +m[2] - 1, day: +m[3] };
  }

  import { untrack } from 'svelte';
  import Select from './Select.svelte';
  import ChevronIcon from '$lib/components/icons/ChevronIcon.svelte';

  const parsed = $derived(parseValue(value));
  const today = new Date();

  let viewYear = $state(untrack(() => parseValue(value)?.year ?? today.getFullYear()));
  let viewMonth = $state(untrack(() => parseValue(value)?.month ?? today.getMonth()));

  const MONTHS = [
    'January',
    'February',
    'March',
    'April',
    'May',
    'June',
    'July',
    'August',
    'September',
    'October',
    'November',
    'December',
  ];
  const DAY_HEADERS = ['Su', 'Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa'];

  const monthOptions = $derived(MONTHS.map((m, i) => ({ value: String(i), label: m })));
  const yearOptions = $derived.by(() => {
    const base = parseValue(value)?.year ?? today.getFullYear();
    const start = Math.min(base - 100, today.getFullYear() - 50);
    const end = Math.max(base + 100, today.getFullYear() + 50);
    return Array.from({ length: end - start + 1 }, (_, i) => {
      const y = start + i;
      return { value: String(y), label: String(y) };
    });
  });

  type Cell = { day: number; offset: -1 | 0 | 1 };

  const calendarDays = $derived.by((): Cell[] => {
    const firstWeekday = new Date(viewYear, viewMonth, 1).getDay();
    const daysInMonth = new Date(viewYear, viewMonth + 1, 0).getDate();
    const daysInPrev = new Date(viewYear, viewMonth, 0).getDate();
    const cells: Cell[] = [];

    for (let i = firstWeekday - 1; i >= 0; i--) {
      cells.push({ day: daysInPrev - i, offset: -1 });
    }
    for (let d = 1; d <= daysInMonth; d++) {
      cells.push({ day: d, offset: 0 });
    }
    let next = 1;
    while (cells.length < 42) {
      cells.push({ day: next++, offset: 1 });
    }
    return cells;
  });

  function prevMonth(): void {
    if (viewMonth === 0) {
      viewMonth = 11;
      viewYear--;
    } else viewMonth--;
  }

  function nextMonth(): void {
    if (viewMonth === 11) {
      viewMonth = 0;
      viewYear++;
    } else viewMonth++;
  }

  function selectCell(cell: Cell): void {
    let y = viewYear,
      mo = viewMonth + cell.offset;
    if (mo < 0) {
      mo = 11;
      y--;
    } else if (mo > 11) {
      mo = 0;
      y++;
    }
    const mm = String(mo + 1).padStart(2, '0');
    const dd = String(cell.day).padStart(2, '0');
    onchange(`${y}-${mm}-${dd}`);
  }

  function isSelected(cell: Cell): boolean {
    if (!parsed || cell.offset !== 0) return false;
    return parsed.year === viewYear && parsed.month === viewMonth && parsed.day === cell.day;
  }

  function isToday(cell: Cell): boolean {
    if (cell.offset !== 0) return false;
    return (
      today.getFullYear() === viewYear &&
      today.getMonth() === viewMonth &&
      today.getDate() === cell.day
    );
  }
</script>

<div class="date-picker">
  <div class="dp-header">
    <button class="dp-nav" onclick={prevMonth} aria-label="Previous month">
      <ChevronIcon direction="left" width={14} height={14} />
    </button>
    <div class="dp-dropdowns">
      <Select
        value={String(viewMonth)}
        options={monthOptions}
        size="xs"
        aria-label="Month"
        onchange={(v) => (viewMonth = +v)}
        searchable
      />
      <Select
        value={String(viewYear)}
        options={yearOptions}
        size="xs"
        aria-label="Year"
        onchange={(v) => (viewYear = +v)}
        searchable
      />
    </div>
    <button class="dp-nav" onclick={nextMonth} aria-label="Next month">
      <ChevronIcon direction="right" width={14} height={14} />
    </button>
  </div>

  <div class="dp-grid">
    {#each DAY_HEADERS as d}
      <span class="dp-weekday">{d}</span>
    {/each}
    {#each calendarDays as cell (cell.offset + '-' + cell.day + '-' + viewYear + '-' + viewMonth)}
      <button
        class="dp-day"
        class:dp-day--other={cell.offset !== 0}
        class:dp-day--today={isToday(cell)}
        class:dp-day--selected={isSelected(cell)}
        onclick={() => selectCell(cell)}
        tabindex={cell.offset === 0 ? 0 : -1}
        aria-label="{MONTHS[viewMonth]} {cell.day}, {viewYear}"
        aria-pressed={isSelected(cell)}
      >
        {cell.day}
      </button>
    {/each}
  </div>
</div>

<style>
  .date-picker {
    padding: var(--spacing-2);
    -webkit-user-select: none;
    user-select: none;
  }

  .dp-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: var(--spacing-2);
  }

  .dp-dropdowns {
    display: flex;
    gap: var(--spacing-1);
    flex: 1;
    justify-content: center;
  }

  .dp-nav {
    width: 24px;
    height: 24px;
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

  .dp-nav:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .dp-grid {
    display: grid;
    grid-template-columns: repeat(7, 30px);
    gap: 2px;
  }

  .dp-weekday {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    text-align: center;
    height: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: var(--font-weight-medium);
  }

  .dp-day {
    width: 30px;
    height: 28px;
    border: none;
    background: transparent;
    border-radius: var(--radius-sm);
    cursor: pointer;
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-primary);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .dp-day:hover:not(.dp-day--selected) {
    background: var(--color-accent-subtle);
  }

  .dp-day--other {
    color: var(--color-text-disabled);
  }

  .dp-day--today:not(.dp-day--selected) {
    color: var(--color-accent);
    font-weight: var(--font-weight-semibold);
  }

  .dp-day--selected {
    background: var(--color-accent);
    color: var(--color-text-on-accent);
    font-weight: var(--font-weight-semibold);
  }

  .dp-day--selected:hover {
    background: var(--color-accent-hover);
  }
</style>
