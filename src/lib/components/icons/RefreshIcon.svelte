<script lang="ts">
  interface Props {
    width?: number;
    height?: number;
  }
  const { width = 14, height = 14 }: Props = $props();
</script>

<svg {width} {height} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <path d="M4 11a8 8 0 0 1 13.5-5.5L20 8"></path>
  <polyline points="20 3 20 8 15 8"></polyline>
  <path d="M20 13a8 8 0 0 1-13.5 5.5L4 16"></path>
  <polyline points="4 21 4 16 9 16"></polyline>
</svg>
