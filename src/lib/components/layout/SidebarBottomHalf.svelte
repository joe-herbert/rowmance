<!--
  SidebarBottomHalf — the connection + schema tree (schema browsing is now inline within ConnectionTree).
-->
<script lang="ts">
  import ConnectionTree from '$lib/components/connections/ConnectionTree.svelte';
</script>

<div class="bottom-half">
  <ConnectionTree />
</div>

<style>
  .bottom-half {
    display: flex;
    flex-direction: column;
    flex: 1;
    overflow: hidden;
    min-height: 0;
  }
</style>
