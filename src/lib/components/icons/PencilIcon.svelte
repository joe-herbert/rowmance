<script lang="ts">
  interface Props {
    width?: number | string;
    height?: number | string;
    strokeWidth?: number;
  }
  let { width = 14, height = 14, strokeWidth = 2 }: Props = $props();
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
>
  <path d="M12 20h9" />
  <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" />
</svg>
