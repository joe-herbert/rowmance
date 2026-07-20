<script lang="ts">
  interface Props {
    width?: number | string;
    height?: number | string;
    class?: string;
  }
  let { width = 11, height = 11, class: className = '' }: Props = $props();
</script>

<svg
  {width}
  {height}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  class={className}
>
  <polyline points="16 18 22 12 16 6" />
  <polyline points="8 6 2 12 8 18" />
  <line x1="14" y1="5" x2="10" y2="19" />
</svg>
