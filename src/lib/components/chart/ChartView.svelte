<!--
  ChartView — full-panel chart view opened as a tab.
  Reads live from the chartData store.
-->
<script lang="ts">
  import ChartPanel from './ChartPanel.svelte';
  import type { SerializedChartConfig } from './ChartPanel.svelte';

  interface Props {
    initialConfig?: SerializedChartConfig | null;
    itemId?: string;
    splitId?: string;
  }

  const { initialConfig = null }: Props = $props();
</script>

<div class="chart-view">
  <ChartPanel compact={false} {initialConfig} />
</div>

<style>
  .chart-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    overflow: hidden;
    background: var(--color-bg-primary);
  }
</style>
