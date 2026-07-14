<script lang="ts">
  interface Props { width?: number | string; height?: number | string; strokeWidth?: number; class?: string; }
  let { width = 13, height = 13, strokeWidth = 2, class: className = '' }: Props = $props();
</script>
<svg {width} {height} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width={strokeWidth} stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class={className}>
  <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
  <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
</svg>
