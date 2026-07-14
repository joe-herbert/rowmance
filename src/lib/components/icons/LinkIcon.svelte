<script lang="ts">
  interface Props { width?: number | string; height?: number | string; strokeWidth?: number; class?: string; }
  let { width = 15, height = 15, strokeWidth = 1.8, class: className = '' }: Props = $props();
</script>
<svg {width} {height} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width={strokeWidth} stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class={className}>
  <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" />
  <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" />
</svg>
