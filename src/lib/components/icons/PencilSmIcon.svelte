<script lang="ts">
  interface Props { width?: number | string; height?: number | string; strokeWidth?: number; }
  let { width = 12, height = 12, strokeWidth = 1.5 }: Props = $props();
</script>
<svg {width} {height} viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width={strokeWidth} stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <path d="M11 2.5a2.121 2.121 0 0 1 3 3L5 15H2v-3L11 2.5z" />
</svg>
