<!--
  SidebarBottomHalf — hosts the connection tree and schema browser.
  The connection tree is shown at the top; expanding a connection reveals its schema.
-->
<script lang="ts">
  import ConnectionTree from '$lib/components/connections/ConnectionTree.svelte';
  import SchemaTree from '$lib/components/schema/SchemaTree.svelte';
</script>

<div class="bottom-half">
  <ConnectionTree />
  <SchemaTree />
</div>

<style>
  .bottom-half {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }
</style>
