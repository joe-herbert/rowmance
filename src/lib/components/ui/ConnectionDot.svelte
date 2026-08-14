<script lang="ts">
  interface Props {
    color: string;
    connected?: boolean;
    dim?: boolean;
    horizontalMargin?: boolean;
  }

  let { color, connected = true, dim = !connected, horizontalMargin = false }: Props = $props();
</script>

<span
  class="connection-dot"
  class:dim
  class:horizontal-margin={horizontalMargin}
  style="background:{color};{connected
    ? `box-shadow:0 0 0 3px color-mix(in srgb,${color} 18%,transparent)`
    : ''}"
  aria-hidden="true"
></span>

<style>
  .connection-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
    transition: opacity var(--transition-fast);
  }

  .connection-dot.dim {
    opacity: 0.4;
  }

  .connection-dot.horizontal-margin {
    margin: 0 2px 0 3px;
  }
</style>
