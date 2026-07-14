<script lang="ts">
  interface Props { width?: number | string; height?: number | string; strokeWidth?: number; }
  let { width = 14, height = 14, strokeWidth = 2 }: Props = $props();
</script>
<svg {width} {height} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width={strokeWidth} stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3" />
</svg>
