<script lang="ts">
  interface Props {
    direction?: 'up' | 'down';
    class?: string;
  }
  let { direction = 'down', class: className = '' }: Props = $props();
</script>

<svg class={className} width="10" height="6" viewBox="0 0 10 6" fill="none" aria-hidden="true">
  <path
    d={direction === 'down' ? 'M1 1l4 4 4-4' : 'M1 5l4-4 4 4'}
    stroke="currentColor"
    stroke-width="1.5"
    stroke-linecap="round"
    stroke-linejoin="round"
  />
</svg>
