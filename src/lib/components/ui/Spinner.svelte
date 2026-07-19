<script lang="ts">
  interface Props {
    size?: number;
    label?: string;
    class?: string;
  }
  let { size = 10, label, class: className = '' }: Props = $props();
</script>

<span
  class="spinner {className}"
  style:width="{size}px"
  style:height="{size}px"
  aria-label={label}
  aria-hidden={label ? undefined : true}
></span>

<style>
  .spinner {
    display: block;
    border-radius: 50%;
    border: 2px solid color-mix(in srgb, var(--color-accent) 30%, transparent);
    border-top-color: var(--color-accent);
    animation: spin 0.7s linear infinite;
    flex-shrink: 0;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
