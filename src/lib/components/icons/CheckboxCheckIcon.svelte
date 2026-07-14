<script lang="ts">
  interface Props { class?: string; }
  let { class: className = '' }: Props = $props();
</script>
<svg class={className} viewBox="0 0 10 8" fill="none">
  <path d="M1 4l3 3 5-6" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" />
</svg>
