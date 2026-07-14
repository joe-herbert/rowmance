<script lang="ts">
  interface Props { width?: number | string; height?: number | string; strokeWidth?: number; class?: string; }
  let { width = 15, height = 15, strokeWidth = 1.8, class: className = '' }: Props = $props();
</script>
<svg {width} {height} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width={strokeWidth} stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class={className}>
  <circle cx="12" cy="12" r="10" />
  <polyline points="12 6 12 12 16 14" />
</svg>
