<script lang="ts">
  interface Props { width?: number | string; height?: number | string; strokeWidth?: number; }
  let { width = 14, height = 14, strokeWidth = 2 }: Props = $props();
</script>
<svg {width} {height} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width={strokeWidth} stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <polyline points="4,14 10,14 10,20" />
  <polyline points="20,10 14,10 14,4" />
  <line x1="14" y1="10" x2="21" y2="3" />
  <line x1="3" y1="21" x2="10" y2="14" />
</svg>
