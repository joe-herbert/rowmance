<script lang="ts">
  interface Props {
    direction?: 'left' | 'right';
    width?: number | string;
    height?: number | string;
    strokeWidth?: number;
  }
  let { direction = 'left', width = 13, height = 13, strokeWidth = 2 }: Props = $props();
</script>

<svg
  {width}
  {height}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={strokeWidth}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if direction === 'left'}
    <polyline points="11 18 5 12 11 6"></polyline>
    <polyline points="19 18 13 12 19 6"></polyline>
  {:else}
    <polyline points="13 18 19 12 13 6"></polyline>
    <polyline points="5 18 11 12 5 6"></polyline>
  {/if}
</svg>
