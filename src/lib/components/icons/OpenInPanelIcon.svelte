<script lang="ts">
  interface Props { width?: number | string; height?: number | string; class?: string; }
  let { width = 16, height = 16, class: className = '' }: Props = $props();
</script>
<svg {width} {height} viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class={className}>
  <path d="M10 3h3v3M13 3l-5.5 5.5M7 4H4a1 1 0 00-1 1v7a1 1 0 001 1h7a1 1 0 001-1V9" />
</svg>
