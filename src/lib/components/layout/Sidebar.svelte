<!--
  Sidebar — the left sidebar.
  Top section: open panels list (natural height, includes collapse button).
  Bottom section: connection + schema tree (fills remaining space).
-->
<script lang="ts">
  import SidebarTopHalf from './SidebarTopHalf.svelte';
  import SidebarBottomHalf from './SidebarBottomHalf.svelte';
</script>

<div class="sidebar">
  <SidebarTopHalf />
  <SidebarBottomHalf />
</div>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }
</style>
