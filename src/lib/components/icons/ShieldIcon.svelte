<script lang="ts">
  interface Props {
    /** Whether to show a checkmark (enabled) or a cross (disabled) inside the shield. */
    checked?: boolean;
    width?: number | string;
    height?: number | string;
    class?: string;
  }
  let { checked = true, width = 12, height = 12, class: className = '' }: Props = $props();
</script>

<svg
  {width}
  {height}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.8"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  class={className}
>
  <path d="M12 2l8 4v6c0 5-3.5 8.5-8 10-4.5-1.5-8-5-8-10V6l8-4z" />
  {#if checked}
    <path d="M9 12l2 2 4-4" />
  {:else}
    <path d="M9.5 9.5l5 5M14.5 9.5l-5 5" />
  {/if}
</svg>
