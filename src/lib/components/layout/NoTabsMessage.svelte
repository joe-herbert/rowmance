<!--
  NoTabsMessage — minimal placeholder shown in a panel when there are no open tabs.
  Used instead of EmptyPanelPlaceholder when the "Empty Tab Behaviour" setting is
  'minimal', or as a transient fallback for 'dashboard' mode before the configured
  dashboard auto-opens (or if it no longer exists).
-->
<div class="empty-panel">
  <p class="empty-panel-title">No open tabs</p>
</div>

<style>
  .empty-panel {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
  }

  .empty-panel-title {
    font-size: var(--font-size-md);
    color: var(--color-text-muted);
    margin: 0;
  }
</style>
