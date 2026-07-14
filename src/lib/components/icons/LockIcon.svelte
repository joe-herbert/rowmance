<script lang="ts">
  interface Props { open?: boolean; width?: number | string; height?: number | string; class?: string; }
  let { open = false, width = 12, height = 12, class: className = '' }: Props = $props();
</script>
<svg {width} {height} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class={className}>
  <rect x="5" y="11" width="14" height="9" rx="2" />
  {#if open}
    <path d="M8 11V8a4 4 0 0 1 7.5-3.9" />
  {:else}
    <path d="M8 11V8a4 4 0 0 1 8 0v3" />
  {/if}
</svg>
