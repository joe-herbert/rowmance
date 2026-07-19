<script lang="ts">
  interface Props {
    width?: number | string;
    height?: number | string;
    strokeWidth?: number;
    class?: string;
  }
  let { width = 14, height = 14, strokeWidth = 1.9, class: className = '' }: Props = $props();
</script>

<svg
  {width}
  {height}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={strokeWidth}
  stroke-linecap="round"
  aria-hidden="true"
  class={className}
>
  <line x1="12" y1="5" x2="12" y2="19" />
  <line x1="5" y1="12" x2="19" y2="12" />
</svg>
