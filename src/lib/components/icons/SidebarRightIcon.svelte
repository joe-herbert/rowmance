<script lang="ts">
  interface Props { class?: string; }
  let { class: className = '' }: Props = $props();
</script>
<svg width="15" height="15" viewBox="0 0 15 15" fill="none" aria-hidden="true" class={className}>
  <rect x="1" y="1" width="13" height="13" rx="2" stroke="currentColor" stroke-width="1.2" />
  <line x1="10" y1="1" x2="10" y2="14" stroke="currentColor" stroke-width="1.2" />
  <rect x="10.6" y="1.6" width="2.8" height="11.8" rx="1" fill="currentColor" opacity="0.5" />
</svg>
