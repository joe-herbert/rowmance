<!--
  DateTimePicker — combines DatePicker and TimePicker for DATETIME/TIMESTAMP fields.
  Value format: "YYYY-MM-DD HH:MM:SS" (space-separated, matching DB storage format).
-->
<script lang="ts">
  import DatePicker from './DatePicker.svelte';
  import TimePicker from './TimePicker.svelte';

  interface Props {
    value: string;
    onchange: (_v: string) => void;
  }

  let { value = '', onchange }: Props = $props();

  function splitValue(v: string): { date: string; time: string } {
    const normalized = v.replace('T', ' ');
    const spaceIdx = normalized.indexOf(' ');
    if (spaceIdx === -1) return { date: normalized, time: '00:00:00' };
    return {
      date: normalized.slice(0, spaceIdx),
      time: normalized.slice(spaceIdx + 1).replace(/\.\d+$/, ''),
    };
  }

  const dateValue = $derived(splitValue(value).date);
  const timeValue = $derived(splitValue(value).time);

  function handleDateChange(d: string): void {
    onchange(`${d} ${timeValue || '00:00:00'}`);
  }

  function handleTimeChange(t: string): void {
    onchange(`${dateValue || ''} ${t}`);
  }
</script>

<div class="datetime-picker">
  <DatePicker value={dateValue} onchange={handleDateChange} />
  <div class="dtp-divider"></div>
  <TimePicker value={timeValue} onchange={handleTimeChange} />
</div>

<style>
  .datetime-picker {
    display: flex;
    flex-direction: column;
  }

  .dtp-divider {
    height: 1px;
    background: var(--color-border);
    margin: 0 var(--spacing-2);
  }
</style>
