<script lang="ts">
  interface Props {
    size?: number;
    class?: string;
  }
  let { size = 8, class: className = '' }: Props = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 8 8"
  fill="currentColor"
  aria-hidden="true"
  class={className}
>
  <polygon points="1,1 7,4 1,7" />
</svg>
