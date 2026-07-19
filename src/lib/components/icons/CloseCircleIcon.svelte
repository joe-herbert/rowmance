<script lang="ts">
  interface Props {
    width?: number | string;
    height?: number | string;
    strokeWidth?: number;
    class?: string;
  }
  let { width = 14, height = 14, strokeWidth = 1.8, class: className = '' }: Props = $props();
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
  <circle cx="12" cy="12" r="10"></circle>
  <line x1="15" y1="9" x2="9" y2="15"></line>
  <line x1="9" y1="9" x2="15" y2="15"></line>
</svg>
