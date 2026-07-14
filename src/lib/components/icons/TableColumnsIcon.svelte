<script lang="ts">
  interface Props { size?: number; class?: string; }
  let { size = 14, class: className = '' }: Props = $props();
</script>
<svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class={className}>
  <rect x="3" y="4" width="18" height="16" rx="2" />
  <line x1="9" y1="4" x2="9" y2="20" />
  <line x1="15" y1="4" x2="15" y2="20" />
</svg>
