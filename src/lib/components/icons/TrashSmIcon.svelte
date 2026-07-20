<script lang="ts">
  interface Props {
    width?: number | string;
    height?: number | string;
    strokeWidth?: number;
  }
  let { width = 12, height = 12, strokeWidth = 1.5 }: Props = $props();
</script>

<svg
  {width}
  {height}
  viewBox="0 0 16 16"
  fill="none"
  stroke="currentColor"
  stroke-width={strokeWidth}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d="M6 2h4M2 5h12M4 5l1 9h6l1-9" />
</svg>
