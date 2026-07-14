<script lang="ts">
  interface Props { width?: number | string; height?: number | string; class?: string; }
  let { width = 15, height = 15, class: className = '' }: Props = $props();
</script>
<svg {width} {height} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class={className}>
  <circle cx="11" cy="11" r="8" />
  <line x1="21" y1="21" x2="16.65" y2="16.65" />
  <line x1="8" y1="11" x2="14" y2="11" />
  <line x1="11" y1="8" x2="11" y2="14" />
</svg>
