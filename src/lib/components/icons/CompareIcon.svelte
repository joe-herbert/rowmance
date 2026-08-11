<script lang="ts">
  interface Props {
    size?: number;
    class?: string;
  }
  let { size = 14, class: className = '' }: Props = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.7"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  class={className}
>
  <circle cx="6" cy="6" r="2.75" />
  <circle cx="18" cy="18" r="2.75" />
  <path d="M13 6h3a2 2 0 0 1 2 2v7" />
  <path d="M11 18H8a2 2 0 0 1-2-2V9" />
</svg>
