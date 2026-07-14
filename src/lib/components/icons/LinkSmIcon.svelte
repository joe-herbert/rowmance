<script lang="ts">
  interface Props { width?: number | string; height?: number | string; strokeWidth?: number; }
  let { width = 12, height = 12, strokeWidth = 1.5 }: Props = $props();
</script>
<svg {width} {height} viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width={strokeWidth} stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <path d="M6.5 9.5a3.5 3.5 0 0 0 4.95 0l2-2a3.5 3.5 0 0 0-4.95-4.95l-1 1" />
  <path d="M9.5 6.5a3.5 3.5 0 0 0-4.95 0l-2 2a3.5 3.5 0 0 0 4.95 4.95l1-1" />
</svg>
