<script lang="ts">
  interface Props { width?: number | string; height?: number | string; strokeWidth?: number; class?: string; }
  let { width = 13, height = 13, strokeWidth = 2.5, class: className = '' }: Props = $props();
</script>
<svg {width} {height} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width={strokeWidth} stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class={className}>
  <polyline points="20 6 9 17 4 12" />
</svg>
