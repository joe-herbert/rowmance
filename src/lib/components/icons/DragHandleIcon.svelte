<script lang="ts">
  interface Props { class?: string; }
  let { class: className = '' }: Props = $props();
</script>
<svg width="8" height="12" viewBox="0 0 8 12" fill="currentColor" aria-hidden="true" class={className}>
  <circle cx="2" cy="2" r="1.2" /><circle cx="6" cy="2" r="1.2" />
  <circle cx="2" cy="6" r="1.2" /><circle cx="6" cy="6" r="1.2" />
  <circle cx="2" cy="10" r="1.2" /><circle cx="6" cy="10" r="1.2" />
</svg>
