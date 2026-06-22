<script lang="ts">
  import { portal } from '$lib/actions/portal';

  interface Props {
    zindex?: number;
    label?: string;
    onbackdropclick?: () => void;
    children: import('svelte').Snippet;
  }

  const { zindex = 300, label, onbackdropclick, children }: Props = $props();
</script>

<div
  class="modal-backdrop"
  style="z-index: {zindex}"
  role="dialog"
  aria-modal="true"
  aria-label={label}
  tabindex="-1"
  onclick={(e) => { if (e.target === e.currentTarget) onbackdropclick?.(); }}
  onkeydown={(e) => { if ((e.key === 'Enter' || e.key === ' ') && e.target === e.currentTarget) onbackdropclick?.(); }}
  use:portal
>
  {@render children()}
</div>

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
  }
</style>
