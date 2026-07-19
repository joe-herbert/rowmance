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
  fill="none"
  stroke="currentColor"
  stroke-width="1.7"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  class={className}
>
  <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
  <polyline points="14 2 14 8 20 8" />
  <polyline points="9.5 13 8 15 9.5 17" />
  <polyline points="14.5 13 16 15 14.5 17" />
</svg>
