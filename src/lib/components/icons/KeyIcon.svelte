<script lang="ts">
  interface Props {
    width?: number | string;
    height?: number | string;
    strokeWidth?: number;
    class?: string;
  }
  let { width = 14, height = 14, strokeWidth = 2, class: className = '' }: Props = $props();
</script>

<svg
  {width}
  {height}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={strokeWidth}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  class={className}
>
  <circle cx="8" cy="9" r="4"></circle>
  <path d="M11 12l7 7"></path>
  <path d="M16 17l2-2"></path>
</svg>
