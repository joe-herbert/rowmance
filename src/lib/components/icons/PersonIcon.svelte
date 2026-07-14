<script lang="ts">
  interface Props { size?: number; class?: string; }
  let { size = 14, class: className = '' }: Props = $props();
</script>
<svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class={className}>
  <circle cx="12" cy="8" r="4" />
  <path d="M4 20c0-4 3.6-7 8-7s8 3 8 7" />
</svg>
