<script lang="ts">
  interface Props { direction?: 'forward' | 'back'; width?: number | string; height?: number | string; class?: string; }
  let { direction = 'forward', width = 16, height = 16, class: className = '' }: Props = $props();
</script>
<svg {width} {height} viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class={className}>
  {#if direction === 'forward'}
    <path d="M3 8h10M9 4l4 4-4 4" />
  {:else}
    <path d="M13 8H3M7 4L3 8l4 4" />
  {/if}
</svg>
