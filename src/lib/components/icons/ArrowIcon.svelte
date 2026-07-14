<script lang="ts">
  interface Props { direction?: 'left' | 'right' | 'up' | 'down'; width?: number | string; height?: number | string; class?: string; }
  let { direction = 'right', width = 13, height = 13, class: className = '' }: Props = $props();
</script>
<svg {width} {height} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true" class={className}>
  {#if direction === 'left'}
    <line x1="19" y1="12" x2="5" y2="12" /><polyline points="11 18 5 12 11 6" />
  {:else if direction === 'right'}
    <line x1="5" y1="12" x2="19" y2="12" /><polyline points="13 6 19 12 13 18" />
  {:else if direction === 'up'}
    <line x1="12" y1="19" x2="12" y2="5" /><polyline points="18 11 12 5 6 11" />
  {:else}
    <line x1="12" y1="5" x2="12" y2="19" /><polyline points="6 13 12 19 18 13" />
  {/if}
</svg>
