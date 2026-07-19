<script lang="ts">
  interface Props {
    width?: number | string;
    height?: number | string;
    class?: string;
  }
  let { width = 13, height = 13, class: className = '' }: Props = $props();
</script>

<svg {width} {height} viewBox="0 0 16 16" fill="none" aria-hidden="true" class={className}>
  <path
    d="M8 1.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13ZM0 8a8 8 0 1 1 16 0A8 8 0 0 1 0 8Z"
    fill="currentColor"
  />
  <path
    d="M7.25 4.75a.75.75 0 0 1 1.5 0v3.5a.75.75 0 0 1-1.5 0v-3.5ZM8 11.5a.875.875 0 1 0 0-1.75.875.875 0 0 0 0 1.75Z"
    fill="currentColor"
  />
</svg>
