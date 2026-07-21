<script lang="ts">
  interface Props {
    width?: number | string;
    height?: number | string;
    class?: string;
  }
  let { width = 13, height = 13, class: className = '' }: Props = $props();
</script>

<svg
  {width}
  {height}
  viewBox="0 0 24 24"
  fill="currentColor"
  stroke="none"
  aria-hidden="true"
  class={className}
>
  <polygon points="6 3 20 12 6 21 6 3" />
</svg>
