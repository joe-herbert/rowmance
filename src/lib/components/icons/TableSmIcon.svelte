<script lang="ts">
  interface Props { width?: number | string; height?: number | string; class?: string; }
  let { width = 16, height = 16, class: className = '' }: Props = $props();
</script>
<svg {width} {height} viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class={className}>
  <rect x="2" y="3" width="12" height="10" rx="1" />
  <path d="M2 7h12M6 7v6" />
</svg>
