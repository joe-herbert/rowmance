<script lang="ts">
  interface Props { size?: number; class?: string; }
  let { size = 14, class: className = '' }: Props = $props();
</script>
<svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class={className}>
  <polyline points="8 7 4 12 8 17" />
  <polyline points="16 7 20 12 16 17" />
</svg>
