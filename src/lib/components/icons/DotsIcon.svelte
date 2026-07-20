<script lang="ts">
  interface Props {
    width?: number | string;
    height?: number | string;
  }
  let { width = 14, height = 14 }: Props = $props();
</script>

<svg {width} {height} viewBox="0 0 14 14" fill="none" aria-hidden="true">
  <circle cx="2" cy="7" r="1.25" fill="currentColor" />
  <circle cx="7" cy="7" r="1.25" fill="currentColor" />
  <circle cx="12" cy="7" r="1.25" fill="currentColor" />
</svg>
