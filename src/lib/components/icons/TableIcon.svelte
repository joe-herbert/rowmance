<script lang="ts">
  interface Props { system?: boolean; size?: number; }
  let { system = false, size = 14 }: Props = $props();
</script>

<span class="icon-wrap">
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6">
    <rect x="3" y="4" width="18" height="16" rx="2"></rect>
    <line x1="3" y1="9.5" x2="21" y2="9.5"></line>
    <line x1="9" y1="9.5" x2="9" y2="20"></line>
  </svg>
  {#if system}
    <svg class="badge" width="7" height="7" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
      <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/>
    </svg>
  {/if}
</span>

<style>
  .icon-wrap { position: relative; display: inline-flex; flex-shrink: 0; }
  .badge { position: absolute; bottom: -2px; right: -3px; }
</style>
