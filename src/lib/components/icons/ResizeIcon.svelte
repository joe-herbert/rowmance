<script lang="ts">
  interface Props { width?: number | string; height?: number | string; }
  let { width = 10, height = 10 }: Props = $props();
</script>
<svg {width} {height} viewBox="0 0 10 10" fill="currentColor" aria-hidden="true">
  <line x1="3" y1="10" x2="10" y2="3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
  <line x1="6" y1="10" x2="10" y2="6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
  <line x1="9" y1="10" x2="10" y2="9" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
</svg>
