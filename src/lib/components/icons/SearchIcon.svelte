<script lang="ts">
  interface Props { width?: number | string; height?: number | string; strokeWidth?: number; class?: string; }
  let { width = 14, height = 14, strokeWidth = 1.7, class: className = '' }: Props = $props();
</script>
<svg {width} {height} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width={strokeWidth} stroke-linecap="round" aria-hidden="true" class={className}>
  <circle cx="11" cy="11" r="7" />
  <line x1="21" y1="21" x2="16.5" y2="16.5" />
</svg>
