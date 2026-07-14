<script lang="ts">
  interface Props { direction?: 'up' | 'down' | 'left' | 'right'; width?: number | string; height?: number | string; strokeWidth?: number; class?: string; }
  let { direction = 'down', width = 14, height = 14, strokeWidth = 2, class: className = '' }: Props = $props();
  const points: Record<string, string> = {
    down: '6 9 12 15 18 9',
    up: '18 15 12 9 6 15',
    left: '15 18 9 12 15 6',
    right: '9 18 15 12 9 6',
  };
</script>
<svg {width} {height} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width={strokeWidth} stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class={className}>
  <polyline points={points[direction]} />
</svg>
