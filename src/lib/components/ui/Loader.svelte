<script lang="ts">
  interface Props {
    class?: string;
  }

  let { class: className = '' }: Props = $props();
</script>

<div class="grid-loader {className}" role="status" aria-label="Loading">
  <div></div>
  <div></div>
  <div></div>
  <div></div>
  <div></div>
  <div></div>
  <div></div>
  <div></div>
  <div></div>
</div>

<style>
  .grid-loader {
    display: grid;
    grid-template-columns: repeat(3, 18px);
    grid-template-rows: repeat(3, 18px);
    gap: 6px;
    color: var(--color-accent);
  }

  .grid-loader div {
    width: 18px;
    height: 18px;
    border-radius: var(--radius-sm);
    background: currentColor;
    animation: grid-pulse 1.2s ease-in-out infinite;
  }

  .grid-loader div:nth-child(1) {
    animation-delay: 0s;
  }
  .grid-loader div:nth-child(2) {
    animation-delay: 0.1s;
  }
  .grid-loader div:nth-child(3) {
    animation-delay: 0.2s;
  }
  .grid-loader div:nth-child(4) {
    animation-delay: 0.1s;
  }
  .grid-loader div:nth-child(5) {
    animation-delay: 0.2s;
  }
  .grid-loader div:nth-child(6) {
    animation-delay: 0.3s;
  }
  .grid-loader div:nth-child(7) {
    animation-delay: 0.2s;
  }
  .grid-loader div:nth-child(8) {
    animation-delay: 0.3s;
  }
  .grid-loader div:nth-child(9) {
    animation-delay: 0.4s;
  }

  @keyframes grid-pulse {
    0%,
    70%,
    100% {
      opacity: 0.25;
      transform: scale(0.75);
    }
    35% {
      opacity: 1;
      transform: scale(1);
    }
  }
</style>
